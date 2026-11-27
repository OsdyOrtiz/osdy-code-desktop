//! One `html` surface: a page the agent wrote, run in an isolated web view
//! that speaks MCP Apps (`gentle_core::genui::apps`), so the same page works
//! in any MCP Apps host.
//!
//! In the conversation it is a compact card; the live page only runs in the
//! expanded view. A native web view is drawn above everything GPUI draws and
//! isn't clipped by it, so one inside a scrolling list would cover the
//! composer, menus and dialogs and stick out of the list's edges; the
//! expanded view has a fixed place, hides the page whenever something is
//! drawn over it (see [`HtmlView::set_obscured`]) and keeps one web process
//! per conversation at most.
//!
//! The page is served from the app's own scheme inside a relay document
//! (`html-host.html`) as a sandboxed iframe: opaque origin, scripts only,
//! the policy from its spec (no network by default). What it asks goes
//! through the bridge: `ui/message` and `ui/update-model-context` become
//! `/gentle-ui:event` commands, `ui/open-link` asks the user first and
//! opens the in-app browser, `ui/notifications/size-changed` sizes the page.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gentle_core::genui::apps::{self, Ask, Bridge};
use gentle_core::genui::html::{self as spec, HtmlSpec};
use gentle_core::genui::{self, Outcome, Snapshot};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, Size, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::diagram::{DiagramEvent, SendState};
use crate::features::browser::{Sandbox, SandboxPage, WebSurface, sandbox_url};
use crate::shared::ui;

/// The relay document the page runs in.
const HOST_PAGE: &str = include_str!("html-host.html");
/// Model context is sent once the page stops changing it for this long.
const CONTEXT_QUIET: Duration = Duration::from_millis(800);
/// How long a closing page may take to answer `ui/resource-teardown`.
const TEARDOWN_WAIT: Duration = Duration::from_millis(500);
/// The card's height in the conversation (px at 100 %).
pub(super) const CARD_HEIGHT: f32 = 104.;

/// What the scheme serves for one surface.
#[derive(Default)]
struct Pages {
    host: String,
    host_csp: String,
    view: String,
    view_csp: String,
}

/// A `ui/message` on its way to the agent: the page's JSON-RPC request id
/// (answered once the extension took it) and the event command.
struct PageMessage {
    /// `None` once the page that asked is gone.
    id: Option<Value>,
    version: u64,
    command: String,
}

/// The send in flight (one at a time), until Gentle Shell ran its command.
enum Sending {
    Message(PageMessage),
    Context {
        version: u64,
    },
    /// The extension already answered it ([`HtmlView::outcome`]).
    Answered,
}

pub struct HtmlView {
    id: String,
    title: String,
    version: u64,
    /// The spec as the agent sent it (echoed in the events).
    spec_value: Value,
    spec: Option<HtmlSpec>,
    /// Why the latest spec can't be shown.
    error: Option<String>,
    expanded: bool,
    /// Something is drawn over the page, or its conversation isn't shown.
    obscured: bool,
    /// The page, while expanded (and while it closes).
    live: Option<Entity<WebSurface>>,
    /// Waiting for the page's answer to `ui/resource-teardown`.
    closing: bool,
    pages: Arc<Mutex<Pages>>,
    /// Loads so far, so each reload gets a fresh URL.
    loads: u64,
    ipc: async_channel::Sender<String>,
    bridge: Bridge,
    /// The last `ui/message`'s fate, for the footer.
    send: SendState,
    in_flight: Option<Sending>,
    /// A message the agent didn't get, kept for Retry (the page waits for
    /// its answer until then, or until Discard).
    unsent: Option<PageMessage>,
    /// The page's latest model context, not sent yet.
    context: Option<(Vec<Value>, Option<Value>)>,
    /// Bumped by each context update, so only the last one's timer sends.
    context_generation: u64,
    /// A link the page wants opened, waiting for the user.
    link: Option<(Value, String)>,
    /// The page's own height (`size-changed`), px.
    content_height: Option<f32>,
    /// The space the page may take, written while painting.
    area: Rc<Cell<Bounds<Pixels>>>,
    _ipc: Task<()>,
    pub(super) env: super::share::Env,
}

impl EventEmitter<DiagramEvent> for HtmlView {}

impl HtmlView {
    pub fn new(snapshot: &Snapshot, cx: &mut Context<Self>) -> Self {
        let (ipc, inbox) = async_channel::unbounded::<String>();
        let receiver = cx.spawn(async move |this, cx| {
            while let Ok(message) = inbox.recv().await {
                if this
                    .update(cx, |view, cx| view.receive(&message, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut view = Self {
            id: snapshot.id.clone(),
            title: snapshot.title.clone(),
            version: snapshot.version,
            spec_value: Value::Null,
            spec: None,
            error: None,
            expanded: false,
            obscured: false,
            live: None,
            closing: false,
            pages: Arc::default(),
            loads: 0,
            ipc,
            bridge: Bridge::new(
                "gentle-shell-gpui",
                env!("CARGO_PKG_VERSION"),
                apps::capabilities(json!({})),
                json!({}),
            ),
            send: SendState::Idle,
            in_flight: None,
            unsent: None,
            context: None,
            context_generation: 0,
            link: None,
            content_height: None,
            area: Rc::default(),
            _ipc: receiver,
            env: Default::default(),
        };
        view.read_spec(&snapshot.spec);
        view
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The page as the agent sent it, for the Export menu.
    fn source(&self) -> super::share::Source {
        super::share::Source {
            id: self.id.clone(),
            kind: "html".into(),
            title: self.display_title().to_string(),
            spec: self.spec_value.clone(),
            root: self.env.root.clone(),
        }
    }

    fn read_spec(&mut self, value: &Value) {
        self.spec_value = value.clone();
        match HtmlSpec::parse(value) {
            Ok(spec) => {
                self.error = None;
                self.spec = Some(spec);
            }
            Err(errors) => self.error = Some(errors.join("\n")),
        }
    }

    /// A newer version from the transcript: a new page or policy reloads it,
    /// new data reaches the running page as a tool result.
    pub fn set_snapshot(&mut self, snapshot: &Snapshot, replaced: bool, cx: &mut Context<Self>) {
        let same = snapshot.version == self.version && snapshot.title == self.title;
        if !replaced && (same || snapshot.version < self.version) {
            return;
        }
        if same && snapshot.spec == self.spec_value {
            return;
        }
        let before = self.spec.clone();
        self.version = snapshot.version;
        self.title = snapshot.title.clone();
        self.read_spec(&snapshot.spec);
        let same_page = matches!((&before, &self.spec), (Some(old), Some(new)) if old.html == new.html && old.csp == new.csp);
        let new_data = before.as_ref().map(|s| &s.data) != self.spec.as_ref().map(|s| &s.data);
        if same_page {
            if new_data {
                self.deliver_result(cx);
            }
        } else if self.live.is_some() && !self.closing {
            self.content_height = None;
            self.load(cx);
        }
        cx.notify();
    }

    pub fn set_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        if expanded {
            if self.closing {
                self.drop_live();
            }
            self.load(cx);
        } else {
            self.close(cx);
        }
        cx.notify();
    }

    /// Hide the page while something is drawn over it or its conversation
    /// isn't on screen.
    pub fn set_obscured(&mut self, obscured: bool, cx: &mut Context<Self>) {
        if self.obscured != obscured {
            self.obscured = obscured;
            self.sync_cover(cx);
        }
    }

    /// Whether Gentle Shell ran the last [`DiagramEvent::Send`]'s command.
    /// The extension answers first ([`Self::outcome`]); an answer only here
    /// means an extension that doesn't send outcomes.
    pub fn sent(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        if let Some(sending) = self.in_flight.take() {
            self.settle(sending, result, cx);
        }
        // The next send waits for this one's command to finish.
        if self.context.is_some() {
            self.send_context(cx);
        }
        cx.notify();
    }

    /// The extension's answer to the event in flight: the page hears
    /// whether its message reached the agent, the footer says so.
    pub fn outcome(&mut self, outcome: &Outcome, cx: &mut Context<Self>) {
        let version = match &self.in_flight {
            Some(Sending::Message(PageMessage { version, .. }) | Sending::Context { version }) => {
                *version
            }
            _ => return,
        };
        if version != outcome.version {
            return;
        }
        if let Some(sending) = self.in_flight.replace(Sending::Answered) {
            let result = outcome.result.as_ref().map(|_| ()).map_err(String::clone);
            self.settle(sending, result, cx);
        }
        if let Ok(current) = &outcome.result {
            // A message is a new version of the page's surface (same spec).
            self.version = self.version.max(*current);
        }
        cx.notify();
    }

    fn settle(&mut self, sending: Sending, result: Result<(), String>, cx: &mut Context<Self>) {
        match sending {
            Sending::Message(message) => match result {
                Ok(()) => {
                    if let Some(id) = &message.id {
                        self.post(&apps::reply(id, json!({})), cx);
                    }
                    self.send = SendState::Sent;
                }
                // The page keeps waiting: Retry sends it again, Discard
                // tells the page it didn't go.
                Err(error) => {
                    self.unsent = Some(message);
                    self.send = SendState::Failed(error);
                }
            },
            Sending::Context { .. } => {
                if let Err(error) = result {
                    self.send = SendState::Failed(error);
                }
            }
            Sending::Answered => {}
        }
    }

    /// Send the message the agent didn't get again.
    fn retry(&mut self, cx: &mut Context<Self>) {
        if self.in_flight.is_some() {
            return;
        }
        let Some(message) = self.unsent.take() else {
            return;
        };
        let command = message.command.clone();
        self.send = SendState::Sending {
            version: message.version,
            spec: (),
        };
        self.in_flight = Some(Sending::Message(message));
        cx.emit(DiagramEvent::Send(command));
        cx.notify();
    }

    /// Give up on the message the agent didn't get: the page hears so.
    fn discard(&mut self, cx: &mut Context<Self>) {
        let Some(message) = self.unsent.take() else {
            return;
        };
        let error = match &self.send {
            SendState::Failed(error) => error.clone(),
            _ => "Not sent".to_string(),
        };
        if let Some(id) = &message.id {
            self.post(&apps::error(id, apps::DENIED, &error), cx);
        }
        self.send = SendState::Idle;
        cx.notify();
    }

    // ── The page ──────────────────────────────────────────────────────────

    /// (Re)load the page: serve the current spec and point a sandboxed web
    /// view at it (a new one when none runs).
    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(spec) = self.spec.clone() else {
            return;
        };
        self.loads += 1;
        let view_url = sandbox_url(&format!("view?v={}", self.loads));
        let host_url = sandbox_url(&format!("host?v={}", self.loads));
        let origin = sandbox_url("");
        {
            let mut pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
            pages.host = HOST_PAGE
                .replace("{{BACKGROUND}}", &css(ui::well()))
                .replace("{{VIEW}}", &view_url);
            pages.host_csp = format!(
                "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
                 frame-src {}; base-uri 'none'; form-action 'none'",
                origin.trim_end_matches('/')
            );
            pages.view = spec.document(&base_css(cx));
            pages.view_csp = spec.csp.policy();
        }
        self.forget_page();
        self.bridge
            .set_capabilities(apps::capabilities(spec.csp.to_value()));
        self.link = None;
        match &self.live {
            Some(live) => live.update(cx, |surface, cx| surface.navigate(host_url, cx)),
            None => {
                let pages = self.pages.clone();
                let csp = spec.csp.clone();
                let sandbox = Sandbox {
                    serve: Arc::new(move |path| {
                        let pages = pages.lock().unwrap_or_else(|e| e.into_inner());
                        match path {
                            "host" => Some(SandboxPage {
                                html: pages.host.clone(),
                                csp: pages.host_csp.clone(),
                            }),
                            "view" => Some(SandboxPage {
                                html: pages.view.clone(),
                                csp: pages.view_csp.clone(),
                            }),
                            _ => None,
                        }
                    }),
                    ipc: self.ipc.clone(),
                    frames: Rc::new(move |url| csp.allows_frame(url)),
                };
                self.live = Some(cx.new(|_| WebSurface::sandboxed(host_url, sandbox)));
            }
        }
        self.sync_cover(cx);
    }

    /// Collapse: warn the page (`ui/resource-teardown`) and drop it once it
    /// answers, or after [`TEARDOWN_WAIT`].
    fn close(&mut self, cx: &mut Context<Self>) {
        if let Some((id, _)) = self.link.take() {
            self.post(&apps::error(&id, apps::DENIED, "The page was closed"), cx);
        }
        let Some(request) = self
            .live
            .as_ref()
            .and_then(|_| self.bridge.teardown("closed"))
        else {
            self.drop_live();
            return;
        };
        self.post(&request, cx);
        self.closing = true;
        self.sync_cover(cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TEARDOWN_WAIT).await;
            let _ = this.update(cx, |view, cx| {
                if view.closing {
                    view.drop_live();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn drop_live(&mut self) {
        self.live = None;
        self.closing = false;
        self.link = None;
        self.forget_page();
    }

    /// The page instance is gone (closed or reloaded): its requests can't
    /// be answered any more, and a message it wasn't told about is dropped.
    fn forget_page(&mut self) {
        self.bridge.reset();
        self.unsent = None;
        if let Some(Sending::Message(message)) = &mut self.in_flight {
            message.id = None;
        }
    }

    /// Show the native view only when nothing may be drawn over it.
    fn sync_cover(&self, cx: &mut Context<Self>) {
        let covered = !self.expanded || self.obscured || self.closing || self.link.is_some();
        if let Some(live) = &self.live {
            live.update(cx, |surface, cx| surface.set_covered(covered, cx));
        }
    }

    /// Post a JSON-RPC message to the page (through the relay document).
    fn post(&self, message: &Value, cx: &App) {
        if let Some(live) = &self.live {
            live.read(cx)
                .evaluate(&format!("window.__gentleDeliver({message})"));
        }
    }

    /// The page's data as a tool result (after the handshake only: the
    /// handshake sends it anyway).
    fn deliver_result(&mut self, cx: &mut Context<Self>) {
        let Some(spec) = &self.spec else { return };
        if !self.bridge.ready() {
            return;
        }
        let result = apps::tool_result(&self.result_text(), &spec.data);
        if let Some(message) = self.bridge.notify("ui/notifications/tool-result", result) {
            self.post(&message, cx);
        }
    }

    fn result_text(&self) -> String {
        format!(
            "HTML UI \"{}\" ({}, v{}) shown by the agent's gentle_ui tool.",
            self.display_title(),
            self.id,
            self.version
        )
    }

    /// One message the page posted.
    fn receive(&mut self, raw: &str, cx: &mut Context<Self>) {
        if self.live.is_none() {
            return;
        }
        // Esc pressed inside the page: the web view kept the keyboard.
        if spec::is_escape(raw) {
            if self.expanded {
                cx.emit(DiagramEvent::Collapse);
            }
            return;
        }
        let step = self.bridge.receive(raw);
        for message in &step.post {
            self.post(message, cx);
        }
        match step.ask {
            Some(Ask::Ready) => {
                let data = self
                    .spec
                    .as_ref()
                    .map(|s| s.data.clone())
                    .unwrap_or_default();
                if let Some(message) = self
                    .bridge
                    .notify("ui/notifications/tool-input", apps::tool_input(&data))
                {
                    self.post(&message, cx);
                }
                self.deliver_result(cx);
            }
            Some(Ask::Message { id, text }) => {
                if self.in_flight.is_some() || self.unsent.is_some() {
                    let busy = "Another message to the agent is still being sent";
                    self.post(&apps::error(&id, apps::DENIED, busy), cx);
                    return;
                }
                let shown: String = text.chars().take(200).collect();
                let more = if shown.len() < text.len() { "…" } else { "" };
                let command = genui::event_command_json(
                    &self.id,
                    self.version,
                    &[json!({ "op": "message", "text": text })],
                    &[format!("said: “{shown}{more}”")],
                    &self.spec_value,
                    None,
                );
                self.send = SendState::Sending {
                    version: self.version,
                    spec: (),
                };
                self.in_flight = Some(Sending::Message(PageMessage {
                    id: Some(id),
                    version: self.version,
                    command: command.clone(),
                }));
                cx.emit(DiagramEvent::Send(command));
                cx.notify();
            }
            Some(Ask::Context {
                id,
                content,
                structured,
            }) => {
                // Stored by the host (this view) now; the agent gets the
                // latest one when it stops changing.
                self.post(&apps::reply(&id, json!({})), cx);
                self.context = Some((content, structured));
                self.context_generation += 1;
                let generation = self.context_generation;
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(CONTEXT_QUIET).await;
                    let _ = this.update(cx, |view, cx| {
                        if view.context_generation == generation {
                            view.send_context(cx);
                        }
                    });
                })
                .detach();
            }
            Some(Ask::OpenLink { id, url }) => {
                if self.link.is_some() {
                    let busy = "Another link is waiting for the user";
                    self.post(&apps::error(&id, apps::DENIED, busy), cx);
                    return;
                }
                self.link = Some((id, url));
                self.sync_cover(cx);
                cx.notify();
            }
            Some(Ask::Resize { height, .. }) => {
                let height = height.map(|h| h.clamp(spec::MIN_HEIGHT, spec::MAX_HEIGHT) as f32);
                if height.is_some() && height != self.content_height {
                    self.content_height = height;
                    cx.notify();
                }
            }
            Some(Ask::Close) => cx.emit(DiagramEvent::Collapse),
            Some(Ask::TornDown) => {
                if self.closing {
                    self.drop_live();
                    cx.notify();
                }
            }
            Some(Ask::Log { .. }) | None => {}
        }
    }

    /// Send the page's latest model context (unless a send is in flight:
    /// [`Self::sent`] sends it next).
    fn send_context(&mut self, cx: &mut Context<Self>) {
        if self.in_flight.is_some() {
            return;
        }
        let Some((content, structured)) = self.context.take() else {
            return;
        };
        let mut change = json!({ "op": "context", "content": content });
        if let Some(structured) = structured {
            change["structuredContent"] = structured;
        }
        let command = genui::event_command_json(
            &self.id,
            self.version,
            &[change],
            &["updated what the agent should know".to_string()],
            &self.spec_value,
            None,
        );
        self.in_flight = Some(Sending::Context {
            version: self.version,
        });
        cx.emit(DiagramEvent::Send(command));
    }

    /// The user answered the page's link request.
    fn answer_link(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some((id, url)) = self.link.take() else {
            return;
        };
        if open {
            self.post(&apps::reply(&id, json!({})), cx);
            cx.emit(DiagramEvent::OpenWeb(url));
        } else {
            let denied = "Link opening denied by user";
            self.post(&apps::error(&id, apps::DENIED, denied), cx);
        }
        self.sync_cover(cx);
        cx.notify();
    }

    /// The host context for the page in `area`, sent when it changed.
    fn sync_context(&mut self, window: &Window, cx: &mut Context<Self>) {
        let area = self.area.get();
        if self.live.is_none() || area.size.width <= px(1.) {
            return;
        }
        let colors = [
            ui::well(),
            ui::raised(),
            ui::panel(),
            ui::fg(),
            ui::muted(),
            ui::faint(),
            ui::border(),
            ui::border_strong(),
            ui::accent(),
            ui::info(),
            ui::success(),
            ui::danger(),
            ui::warning(),
        ]
        .map(css);
        let [
            background,
            raised,
            well,
            fg,
            muted,
            faint,
            border,
            border_strong,
            accent,
            info,
            success,
            danger,
            warning,
        ] = &colors;
        let font_sans = format!("\"{}\", system-ui, sans-serif", cx.theme().font_family);
        let font_mono = format!("\"{}\", ui-monospace, monospace", ui::mono());
        let palette = apps::Palette {
            dark: cx.theme().is_dark(),
            background,
            raised,
            well,
            fg,
            muted,
            faint,
            border,
            border_strong,
            accent,
            info,
            success,
            danger,
            warning,
            font_sans: &font_sans,
            font_mono: &font_mono,
            rem: f64::from(f32::from(window.rem_size())),
        };
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
            .and_then(|posix| apps::locale_tag(&posix));
        let context = apps::host_context(
            &palette,
            f64::from(f32::from(area.size.width)),
            f64::from(f32::from(area.size.height)),
            locale.as_deref(),
            concat!("gentle-shell-gpui/", env!("CARGO_PKG_VERSION")),
        );
        if let Some(message) = self.bridge.set_context(context) {
            self.post(&message, cx);
        }
    }

    pub(super) fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "HTML"
        } else {
            &self.title
        }
    }

    /// "12 KiB · isolated" or the origins the page may reach.
    fn reach(&self) -> (IconName, Hsla, String) {
        let Some(spec) = &self.spec else {
            return (
                IconName::CircleAlert,
                ui::danger(),
                crate::shared::i18n::t("can't be shown").into(),
            );
        };
        let mut origins: Vec<&str> = spec
            .csp
            .connect
            .iter()
            .chain(&spec.csp.resources)
            .chain(&spec.csp.frames)
            .map(|o| o.split_once("://").map_or(o.as_str(), |(_, rest)| rest))
            .collect();
        origins.sort_unstable();
        origins.dedup();
        if origins.is_empty() {
            (
                IconName::ShieldCheck,
                ui::success(),
                crate::shared::i18n::t("isolated · no network").into(),
            )
        } else {
            (
                IconName::Globe,
                ui::warning(),
                crate::shared::i18n::t("may reach {origins}")
                    .replace("{origins}", &origins.join(", ")),
            )
        }
    }
}

/// A color as CSS.
fn css(color: Hsla) -> String {
    let c = Rgba::from(color);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    if c.a >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
    } else {
        format!(
            "rgba({}, {}, {}, {:.3})",
            byte(c.r),
            byte(c.g),
            byte(c.b),
            c.a
        )
    }
}

/// The host's defaults under the page's own styles: the app's text color,
/// font and color scheme on a transparent background (the relay document
/// paints the card's body color).
fn base_css(cx: &App) -> String {
    format!(
        ":root{{color-scheme:{};color:{};font-family:\"{}\",system-ui,sans-serif;background:transparent}}",
        if cx.theme().is_dark() {
            "dark"
        } else {
            "light"
        },
        css(ui::fg()),
        cx.theme().font_family
    )
}

impl Render for HtmlView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.expanded {
            self.sync_context(window, cx);
        }
        let shell = v_flex()
            .size_full()
            .rounded_xl()
            .bg(ui::surface())
            .border_1()
            .border_color(ui::border())
            .overflow_hidden();
        if !self.expanded {
            return shell.child(self.render_card(cx)).into_any_element();
        }
        shell
            .child(self.render_header(cx))
            .child(self.render_page(cx))
            .child(self.render_footer(cx))
            .into_any_element()
    }
}

impl HtmlView {
    fn title_row(&self) -> Div {
        let (icon, color, reach) = self.reach();
        h_flex()
            .gap_2()
            .min_w_0()
            .child(Icon::new(IconName::CodeXml).small().text_color(ui::info()))
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(rems(13. / 16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui::champagne())
                    .child(self.display_title().to_string()),
            )
            .child(ui::chip(format!("v{}", self.version), ui::info()).flex_shrink_0())
            .child(
                h_flex()
                    .min_w_0()
                    .flex_shrink(1.)
                    .gap_1()
                    .text_size(rems(12. / 16.))
                    .text_color(ui::muted())
                    .child(Icon::new(icon).xsmall().text_color(color))
                    .child(div().min_w_0().truncate().child(reach)),
            )
    }

    /// The card in the conversation: what the page is and a way in.
    fn render_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let note = match (&self.error, &self.send) {
            (Some(error), _) => (ui::danger(), error.lines().next().unwrap_or_default().to_string()),
            (None, SendState::Sent) => (ui::success(), crate::shared::i18n::t("Your message from the page reached the agent").into()),
            (None, SendState::Failed(error)) => (ui::danger(), error.clone()),
            _ => (
                ui::faint(),
                crate::shared::i18n::t("An interactive page from the agent. Open it to use it; what you do there can reach the agent.")
                    .into(),
            ),
        };
        v_flex()
            .id("genui-html-card")
            .size_full()
            .px_3()
            .py_2()
            .gap_1p5()
            .justify_center()
            .cursor_pointer()
            .hover(|d| d.bg(ui::surface_hover()))
            .on_click(cx.listener(|this, _, _, cx| {
                if this.spec.is_some() {
                    cx.emit(DiagramEvent::Expand);
                }
            }))
            .child(
                h_flex()
                    .gap_2()
                    .child(self.title_row().flex_1())
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(super::share::tools(&self.env, "html", Self::source, cx)),
                    )
                    .when(self.spec.is_some(), |row| {
                        row.child(
                            Button::new("genui-html-open")
                                .primary()
                                .xsmall()
                                .icon(Icon::new(IconName::Maximize2))
                                .label(crate::shared::i18n::t("Open"))
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(DiagramEvent::Expand);
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(rems(12. / 16.))
                    .text_color(note.0)
                    .child(note.1),
            )
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .min_w_0()
            .child(self.title_row().flex_1())
            .child(super::share::tools(&self.env, "html", Self::source, cx))
            .child(
                Button::new("genui-html-collapse")
                    .ghost()
                    .xsmall()
                    .text_color(ui::muted())
                    .icon(Icon::new(IconName::Minimize2))
                    .tooltip(crate::shared::i18n::t("Back to the conversation"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DiagramEvent::Collapse))),
            )
    }

    /// The page at its preferred height (what it reports, else the spec's,
    /// else all the room), or the link question in its place.
    fn render_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let area = self.area.clone();
        let height = self
            .content_height
            .or(self.spec.as_ref().and_then(|s| s.height.map(|h| h as f32)));
        let page = div()
            .w_full()
            .rounded_lg()
            .bg(ui::well())
            .overflow_hidden()
            .map(|d| match height {
                Some(h) => d.h(px(h)).max_h_full(),
                None => d.h_full(),
            });
        let page = match (&self.link, &self.error, &self.live) {
            (Some((_, url)), _, _) => page.child(self.render_link(url, cx)),
            (None, Some(error), _) => page.child(
                div()
                    .p_3()
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(12.))
                    .text_color(ui::danger())
                    .whitespace_normal()
                    .child(error.clone()),
            ),
            (None, None, Some(live)) => page.child(live.clone()),
            (None, None, None) => page,
        };
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .px_2()
            .child(
                canvas(
                    move |bounds, window, _| {
                        if area.get() != bounds {
                            area.set(bounds);
                            // Lay the page out for this size on the next
                            // frame (notifying from prepaint draws it stale).
                            ui::request_animation_frame(window);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(page)
    }

    fn render_link(&self, url: &str, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .p_4()
            .child(Icon::new(IconName::ExternalLink).text_color(ui::accent()))
            .child(
                div()
                    .text_size(rems(13. / 16.))
                    .text_color(ui::fg())
                    .child(crate::shared::i18n::t("The page wants to open a link")),
            )
            .child(
                div()
                    .max_w_full()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(ui::raised())
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(12.))
                    .text_color(ui::muted())
                    .truncate()
                    .child(url.to_string()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("genui-html-link-cancel")
                            .ghost()
                            .small()
                            .label(crate::shared::i18n::t("Cancel"))
                            .on_click(cx.listener(|this, _, _, cx| this.answer_link(false, cx))),
                    )
                    .child(
                        Button::new("genui-html-link-open")
                            .primary()
                            .small()
                            .icon(Icon::new(IconName::Globe))
                            .label(crate::shared::i18n::t("Open in the browser card"))
                            .on_click(cx.listener(|this, _, _, cx| this.answer_link(true, cx))),
                    ),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let footer = h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .min_h(rems(34. / 16.))
            .text_size(rems(12. / 16.));
        match &self.send {
            SendState::Sending { .. } => footer
                .text_color(ui::muted())
                .child(ui::spinner(Size::XSmall, ui::info()))
                .child(crate::shared::i18n::t("Sending the page's message to the agent…")),
            SendState::Sent => footer
                .text_color(ui::success())
                .child(Icon::new(IconName::CircleCheck).xsmall())
                .child(crate::shared::i18n::t("Sent to the agent")),
            SendState::Failed(error) => footer
                .text_color(ui::danger())
                .child(Icon::new(IconName::CircleAlert).xsmall())
                .child(div().flex_1().min_w_0().truncate().child(error.clone()))
                .when(self.unsent.is_some(), |footer| {
                    footer
                        .child(
                            Button::new("genui-html-discard")
                                .ghost()
                                .xsmall()
                                .label(crate::shared::i18n::t("Discard"))
                                .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
                        )
                        .child(
                            Button::new("genui-html-retry")
                                .primary()
                                .xsmall()
                                .icon(Icon::new(IconName::RefreshCw))
                                .label(crate::shared::i18n::t("Retry"))
                                .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
                        )
                }),
            SendState::Idle => footer.text_color(ui::faint()).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(crate::shared::i18n::t("Runs isolated from your files and the app · messages from the page go to the agent")),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gentle_core::genui::{Outcome, Snapshot};
    // Not a glob: GPUI exports a `test` attribute that would shadow `#[test]`.
    use gpui_kit::{AppContext as _, Entity, TestAppContext};
    use serde_json::json;

    use super::{DiagramEvent, HtmlView, PageMessage, SendState, Sending};

    const COMMAND: &str = "/gentle-ui:event {\"id\":\"page\",\"version\":1}";

    fn answer(view: &Entity<HtmlView>, result: Result<u64, String>, cx: &mut TestAppContext) {
        let outcome = Outcome {
            id: "page".into(),
            version: 1,
            result,
        };
        view.update(cx, |view, cx| view.outcome(&outcome, cx));
    }

    /// A page's message waits for the extension's answer; a rejection
    /// keeps it for Retry, an acceptance adopts the new version.
    #[gpui::test]
    fn a_page_message_waits_for_the_answer_and_can_be_retried(cx: &mut TestAppContext) {
        let snapshot = Snapshot {
            id: "page".into(),
            version: 1,
            kind: "html".into(),
            title: "Page".into(),
            spec: json!({"html": "<p>hi</p>"}),
            closed: false,
        };
        let view = cx.update(|cx| cx.new(|cx| HtmlView::new(&snapshot, cx)));
        let sends = Rc::new(RefCell::new(Vec::new()));
        let log = sends.clone();
        cx.update(|cx| {
            cx.subscribe(&view, move |_, event: &DiagramEvent, _| {
                if let DiagramEvent::Send(command) = event {
                    log.borrow_mut().push(command.clone());
                }
            })
            .detach()
        });
        // As `receive` leaves it for a `ui/message` (no live page here).
        view.update(cx, |view, _| {
            view.in_flight = Some(Sending::Message(PageMessage {
                id: Some(json!(7)),
                version: 1,
                command: COMMAND.into(),
            }));
            view.send = SendState::Sending {
                version: 1,
                spec: (),
            };
        });

        answer(&view, Err("No agent is running".into()), cx);
        // The command's own completion comes after and changes nothing.
        view.update(cx, |view, cx| view.sent(Ok(()), cx));
        view.read_with(cx, |view, _| {
            assert!(matches!(&view.send, SendState::Failed(e) if e == "No agent is running"));
            assert!(view.unsent.is_some() && view.in_flight.is_none());
        });

        view.update(cx, |view, cx| view.retry(cx));
        assert_eq!(*sends.borrow(), vec![COMMAND.to_string()]);
        // An answer about another version is ignored.
        view.update(cx, |view, cx| {
            let other = Outcome {
                id: "page".into(),
                version: 4,
                result: Ok(5),
            };
            view.outcome(&other, cx)
        });
        view.read_with(cx, |view, _| {
            assert!(matches!(view.send, SendState::Sending { .. }));
        });

        answer(&view, Ok(2), cx);
        view.update(cx, |view, cx| view.sent(Ok(()), cx));
        view.read_with(cx, |view, _| {
            assert!(matches!(view.send, SendState::Sent));
            assert_eq!(view.version, 2);
            assert!(view.unsent.is_none() && view.in_flight.is_none());
        });
    }
}
