//! Generative UI: surfaces the agent shows in the conversation with the
//! `gentle_ui` tool (from the Pi extension the app ships, `gentle-ui.ts`),
//! drawn natively and edited by the user; the edits go back to the agent
//! through the extension's `/gentle-ui:event` command, which answers each
//! one ([`GenUi::outcome`]). Kinds: the diagram ([`DiagramView`]), the
//! panels ([`PanelView`]: table, form, checklist, kanban, chart, review) and `html`
//! ([`HtmlView`]), an agent-written page run in an isolated web view that
//! speaks MCP Apps.
//!
//! Nothing is stored apart from the conversation: every version is a tool
//! result's or a `gentle-ui.event` message's `details`, so [`GenUi::sync`]
//! rebuilds the surfaces from the transcript, live or replayed. Around them
//! the app keeps only the user's own things: which surfaces are pinned to
//! the conversation's side panel ([`GenUi::pin`]) and saved templates
//! (`store.rs`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gentle_core::genui::live::Source;
use gentle_core::genui::{self, Snapshot};
use gentle_core::transcript::{Item, ToolStatus};
use gpui_kit::*;

mod canvas;
mod chart;
mod checklist;
mod details;
mod diagnostic;
mod diagram;
mod erd;
mod extension;
mod form;
mod html;
mod kanban;
mod live;
mod look;
pub mod mode;
mod motion;
mod paint;
mod panel;
mod pinned;
mod pointer;
mod refs;
mod review;
mod sequence;
mod share;
mod states;
mod store;
mod table;
mod templates;
#[cfg(test)]
mod tests;
mod timeline;
mod view;

pub use diagram::{DiagramEvent, DiagramView};
pub use extension::install;
pub(crate) use extension::write_if_changed;
pub use html::HtmlView;
use live::SharedEnv;
pub use live::todo_records;
pub use panel::PanelView;
pub use templates::{TemplatesEvent, TemplatesView};

const NOTE_PLACEHOLDER: &str = "Add a note for the agent (optional)";

fn sync_placeholders<'a>(
    fields: impl IntoIterator<
        Item = (
            &'a Entity<gpui_kit::component::input::InputState>,
            &'static str,
        ),
    >,
    seen: &mut gentle_core::i18n::Language,
    window: &mut Window,
    cx: &mut App,
) {
    let before = *seen;
    for (input, key) in fields {
        let mut previous = before;
        crate::shared::i18n::sync_placeholder(input, key, &mut previous, window, cx);
        *seen = previous;
    }
}

fn count(count: usize, one: &'static str, many: &'static str) -> String {
    gentle_core::genui::edits::count_in(crate::shared::i18n::language(), count, one, many)
}

/// The id of the app's own todo checklist (never a `gentle_ui` id: those
/// are slugs without ":").
pub const TODO_SURFACE: &str = "app:todo";

/// One surface's live view, of any kind. Every kind emits [`DiagramEvent`].
#[derive(Clone)]
pub enum Surface {
    Diagram(Entity<DiagramView>),
    /// Table, form, checklist, kanban and chart.
    Panel(Entity<PanelView>),
    Html(Entity<HtmlView>),
}

impl Surface {
    fn new(snapshot: &Snapshot, cx: &mut App) -> Self {
        match snapshot.kind.as_str() {
            "html" => Self::Html(cx.new(|cx| HtmlView::new(snapshot, cx))),
            kind if panel::handles(kind) => Self::Panel(cx.new(|cx| PanelView::new(snapshot, cx))),
            _ => Self::Diagram(cx.new(|cx| DiagramView::new(snapshot, cx))),
        }
    }

    /// Whether this view draws `kind` (a reloaded transcript may reuse an id;
    /// an id shown again as another kind gets a new surface).
    fn fits(&self, kind: &str, cx: &App) -> bool {
        match self {
            Self::Diagram(_) => kind != "html" && !panel::handles(kind),
            Self::Panel(view) => view.read(cx).kind() == kind,
            Self::Html(_) => kind == "html",
        }
    }

    pub fn entity_id(&self) -> EntityId {
        match self {
            Self::Diagram(view) => view.entity_id(),
            Self::Panel(view) => view.entity_id(),
            Self::Html(view) => view.entity_id(),
        }
    }

    pub fn id(&self, cx: &App) -> String {
        match self {
            Self::Diagram(view) => view.read(cx).id().to_string(),
            Self::Panel(view) => view.read(cx).id().to_string(),
            Self::Html(view) => view.read(cx).id().to_string(),
        }
    }

    fn set_snapshot(&self, snapshot: &Snapshot, reset: bool, cx: &mut App) {
        match self {
            Self::Diagram(view) => {
                view.update(cx, |view, cx| view.set_snapshot(snapshot, reset, cx))
            }
            Self::Panel(view) => view.update(cx, |view, cx| view.set_snapshot(snapshot, reset, cx)),
            Self::Html(view) => view.update(cx, |view, cx| view.set_snapshot(snapshot, reset, cx)),
        }
    }

    fn set_expanded(&self, expanded: bool, cx: &mut App) {
        match self {
            Self::Diagram(view) => view.update(cx, |view, cx| view.set_expanded(expanded, cx)),
            Self::Panel(view) => view.update(cx, |view, cx| view.set_expanded(expanded, cx)),
            Self::Html(view) => view.update(cx, |view, cx| view.set_expanded(expanded, cx)),
        }
    }

    /// Whether the keyboard is inside the surface: a diagram or panel then
    /// handles Esc itself (clearing a selection first). An HTML page never
    /// is, for GPUI: its keys go to the web view.
    fn has_focus(&self, window: &Window, cx: &App) -> bool {
        match self {
            Self::Diagram(view) => view.read(cx).focus.contains_focused(window, cx),
            Self::Panel(view) => view.read(cx).focus.contains_focused(window, cx),
            Self::Html(_) => false,
        }
    }

    /// Whether Gentle Shell ran the surface's last [`DiagramEvent::Send`].
    pub fn sent(&self, result: Result<(), String>, cx: &mut App) {
        match self {
            Self::Diagram(view) => view.update(cx, |view, cx| view.sent(result, cx)),
            Self::Panel(view) => view.update(cx, |view, cx| view.sent(result, cx)),
            Self::Html(view) => view.update(cx, |view, cx| view.sent(result, cx)),
        }
    }

    /// The extension's answer to the surface's last [`DiagramEvent::Send`].
    fn outcome(&self, outcome: &genui::Outcome, cx: &mut App) {
        match self {
            Self::Diagram(view) => view.update(cx, |view, cx| view.outcome(outcome, cx)),
            Self::Panel(view) => view.update(cx, |view, cx| view.outcome(outcome, cx)),
            Self::Html(view) => view.update(cx, |view, cx| view.outcome(outcome, cx)),
        }
    }

    /// Something is drawn over the surface or its conversation is off
    /// screen: only a native web view (html) needs to know.
    pub fn set_obscured(&self, obscured: bool, cx: &mut App) {
        if let Self::Html(view) = self {
            view.update(cx, |view, cx| view.set_obscured(obscured, cx));
        }
    }

    pub fn emit(&self, event: DiagramEvent, cx: &mut App) {
        match self {
            Self::Diagram(view) => view.update(cx, |_, cx| cx.emit(event)),
            Self::Panel(view) => view.update(cx, |_, cx| cx.emit(event)),
            Self::Html(view) => view.update(cx, |_, cx| cx.emit(event)),
        }
    }

    /// The card's height in the conversation (px at 100 %).
    fn card_height(&self, cx: &App) -> f32 {
        match self {
            Self::Diagram(_) => view::CARD_HEIGHT,
            Self::Panel(view) => view.read(cx).card_height(),
            Self::Html(_) => html::CARD_HEIGHT,
        }
    }

    pub fn view(&self) -> AnyView {
        match self {
            Self::Diagram(view) => view.clone().into(),
            Self::Panel(view) => view.clone().into(),
            Self::Html(view) => view.clone().into(),
        }
    }

    /// Where the surface is shown from: the conversation's folder (code
    /// refs resolve against it) and whether it is pinned to the side panel.
    fn set_env(&self, root: Option<&Path>, pinned: bool, cx: &mut App) {
        let apply = |env: &mut share::Env| {
            let changed = env.pinned != pinned || env.root.as_deref() != root;
            env.pinned = pinned;
            env.root = root.map(Path::to_path_buf);
            changed
        };
        match self {
            Self::Diagram(view) => view.update(cx, |view, cx| {
                if apply(&mut view.env) {
                    cx.notify();
                }
            }),
            Self::Panel(view) => view.update(cx, |view, cx| {
                if apply(&mut view.env) {
                    cx.notify();
                }
            }),
            Self::Html(view) => view.update(cx, |view, cx| {
                if apply(&mut view.env) {
                    cx.notify();
                }
            }),
        }
    }

    fn title(&self, cx: &App) -> String {
        match self {
            Self::Diagram(view) => view.read(cx).title.clone(),
            Self::Panel(view) => view.read(cx).title.clone(),
            Self::Html(view) => view.read(cx).display_title().to_string(),
        }
    }
}

/// The surfaces of one conversation.
#[derive(Default)]
pub struct GenUi {
    surfaces: HashMap<String, Surface>,
    /// The item each surface's live card is drawn at: its latest
    /// `gentle_ui` tool item. Earlier items of the same id are superseded.
    homes: HashMap<usize, String>,
    /// Surfaces the agent closed, by the item that closed them.
    closed: HashSet<usize>,
    expanded: Option<String>,
    /// Something is drawn over the conversation, or it isn't on screen.
    obscured: bool,
    /// Created since the conversation last subscribed to them.
    fresh: Vec<Surface>,
    subscriptions: HashMap<EntityId, Subscription>,
    /// The conversation's folder, for code refs and saved files.
    root: Option<PathBuf>,
    /// Pinned to the side panel, in pin order; the one shown there.
    pins: Vec<String>,
    pinned_shown: Option<String>,
    /// The session file the pins were restored for.
    pins_for: Option<PathBuf>,
    /// Templates being inserted, by the id asked for: an error outcome for
    /// one is reported, as no surface waits for it.
    inserting: HashSet<String>,
    /// The templates manager, while open.
    templates: Option<(Entity<TemplatesView>, Subscription)>,
    /// The live data, workspace and allowed commands every live panel shares.
    live: SharedEnv,
    /// The agent's todo list as a checklist ("Show as checklist"), the
    /// app's own surface: no tool call, never in the transcript.
    todo: Option<Surface>,
}

/// Whether `item` carries a surface version (or will).
fn is_surface(item: &Item) -> bool {
    match item {
        Item::Tool { name, .. } => name == genui::TOOL,
        Item::Custom { kind, .. } => kind == genui::EVENT_MESSAGE,
        _ => false,
    }
}

fn tool_title(args: &serde_json::Value, snapshot: Option<&Snapshot>) -> String {
    snapshot
        .map(|s| s.title.as_str())
        .or(args["title"].as_str())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| match args["kind"].as_str().unwrap_or_default() {
            "html" => "HTML",
            kind => panel::default_title(kind),
        })
        .to_string()
}

impl GenUi {
    /// Follow the transcript after items `from..` changed (`reset`: it was
    /// replaced). Cheap when none of them is a surface.
    pub fn sync(&mut self, items: &[Item], from: usize, reset: bool, cx: &mut App) {
        if !reset && !items.get(from..).unwrap_or_default().iter().any(is_surface) {
            return;
        }
        // Each id's latest version and live card, in conversation order.
        let mut latest: HashMap<String, Snapshot> = HashMap::new();
        let mut homes: HashMap<String, usize> = HashMap::new();
        self.closed.clear();
        for (ix, item) in items.iter().enumerate() {
            let (snapshot, args, inserted) = match item {
                Item::Tool {
                    name,
                    details,
                    args,
                    ..
                } if name == genui::TOOL => (genui::snapshot(details), Some(args), false),
                // A template the user inserted lives at its message.
                Item::Custom { kind, details, .. } if kind == genui::EVENT_MESSAGE => {
                    (genui::snapshot(details), None, details["inserted"] == true)
                }
                _ => continue,
            };
            if let Some(snapshot) = snapshot.as_ref().filter(|_| inserted) {
                homes.insert(snapshot.id.clone(), ix);
            }
            if let Some(args) = args {
                let id = snapshot.as_ref().map(|s| s.id.clone()).or_else(|| {
                    args["id"]
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .map(str::to_string)
                });
                if let Some(id) = id {
                    homes.insert(id, ix);
                }
            }
            if let Some(snapshot) = snapshot {
                if snapshot.closed {
                    self.closed.insert(ix);
                }
                latest.insert(snapshot.id.clone(), snapshot);
            }
        }
        self.homes = homes.into_iter().map(|(id, ix)| (ix, id)).collect();
        let gone: Vec<String> = self
            .surfaces
            .keys()
            .filter(|id| latest.get(*id).is_none_or(|s| s.closed))
            .cloned()
            .collect();
        for id in gone {
            if let Some(view) = self.surfaces.remove(&id) {
                self.subscriptions.remove(&view.entity_id());
            }
        }
        for (id, snapshot) in latest {
            if snapshot.closed {
                continue;
            }
            match self.surfaces.get(&id) {
                Some(surface) if surface.fits(&snapshot.kind, cx) => {
                    surface.set_snapshot(&snapshot, reset, cx)
                }
                _ => {
                    if let Some(old) = self.surfaces.remove(&id) {
                        self.subscriptions.remove(&old.entity_id());
                    }
                    let surface = Surface::new(&snapshot, cx);
                    surface.set_env(self.root.as_deref(), self.pins.contains(&id), cx);
                    if let Surface::Panel(view) = &surface {
                        let env = self.live.clone();
                        view.update(cx, |view, cx| view.attach(env, cx));
                    }
                    self.fresh.push(surface.clone());
                    self.surfaces.insert(id, surface);
                }
            }
        }
        if self
            .expanded
            .as_ref()
            .is_some_and(|id| !self.surfaces.contains_key(id) && id != TODO_SURFACE)
        {
            self.expanded = None;
        }
    }

    fn panels(&self) -> impl Iterator<Item = &Entity<PanelView>> {
        self.surfaces
            .values()
            .chain(&self.todo)
            .filter_map(|surface| match surface {
                Surface::Panel(view) => Some(view),
                _ => None,
            })
    }

    /// Whether a live panel (or a kanban card's subagent) shows `source`:
    /// the conversation builds its data only then.
    pub fn wants(&self, source: Source, cx: &App) -> bool {
        self.panels().any(|view| view.read(cx).wants(source))
    }

    /// `source`'s latest data; the panels showing it follow when it changed.
    pub fn feed(&mut self, source: Source, data: serde_json::Value, cx: &mut App) {
        {
            let mut env = self.live.borrow_mut();
            if env.feeds.get(&source) == Some(&data) {
                return;
            }
            env.feeds.insert(source, data);
        }
        let views: Vec<Entity<PanelView>> = self.panels().cloned().collect();
        for view in views {
            view.update(cx, |view, cx| {
                if view.wants(source) {
                    view.feed(source, cx);
                }
            });
        }
    }

    /// The Todos island's "Show as checklist": the agent's todo list,
    /// live and read-only, in place of the conversation.
    pub fn show_todo(&mut self, cx: &mut App) {
        if self.todo.is_none() {
            let view = cx.new(|cx| PanelView::todo_list(TODO_SURFACE, cx));
            let env = self.live.clone();
            view.update(cx, |view, cx| view.attach(env, cx));
            let surface = Surface::Panel(view);
            self.fresh.push(surface.clone());
            self.todo = Some(surface);
        }
        self.set_expanded(Some(TODO_SURFACE.to_string()), cx);
    }

    /// The extension's answer to a surface's sent edits (its
    /// [`genui::OUTCOME_WIDGET`] line). A template that couldn't be
    /// inserted has no surface to tell: its error comes back.
    pub fn outcome(&mut self, line: &str, cx: &mut App) -> Option<String> {
        let outcome = genui::outcome(line)?;
        if self.inserting.remove(&outcome.id) {
            return outcome.result.err();
        }
        if let Some(surface) = self.surfaces.get(&outcome.id) {
            surface.outcome(&outcome, cx);
        }
        None
    }

    /// The conversation's folder (it can change when the tab moves to a
    /// worktree): code refs resolve, files save, and live git sources and
    /// commands run against it.
    pub fn set_root(&mut self, root: &Path, cx: &mut App) {
        if self.root.as_deref() == Some(root) {
            return;
        }
        self.root = Some(root.to_path_buf());
        self.live.borrow_mut().workspace = Some(root.to_path_buf());
        for (id, surface) in &self.surfaces {
            surface.set_env(self.root.as_deref(), self.pins.contains(id), cx);
        }
    }

    /// Pin `id` to the side panel and show it there (`false`: unpin), and
    /// remember it for the conversation saved at `session`.
    pub fn pin(&mut self, id: &str, on: bool, session: Option<&Path>, cx: &mut App) {
        let had = self.pins.iter().any(|p| p == id);
        if on {
            if !had {
                self.pins.push(id.to_string());
            }
            self.pinned_shown = Some(id.to_string());
        } else {
            self.pins.retain(|p| p != id);
            if self.pinned_shown.as_deref() == Some(id) {
                self.pinned_shown = self.pins.last().cloned();
            }
        }
        if let Some(surface) = self.surfaces.get(id) {
            surface.set_env(self.root.as_deref(), on, cx);
        }
        if had != on {
            self.save_pins(session, cx);
        }
    }

    fn save_pins(&mut self, session: Option<&Path>, cx: &App) {
        let (Some(dir), Some(session)) = (store::dir(cx), session) else {
            return;
        };
        self.pins_for = Some(session.to_path_buf());
        if let Err(error) = store::set_pins(&dir, session, &self.pins) {
            eprintln!("gentle-shell-gpui: could not save pinned surfaces: {error}");
        }
    }

    /// Pin again what was pinned in the conversation saved at `session`,
    /// once per session file. Pins made before the conversation had a file
    /// are kept (and saved with it); another conversation's are dropped.
    pub fn restore_pins(&mut self, session: Option<&Path>, cx: &mut App) {
        let Some(session) = session else { return };
        if self.pins_for.as_deref() == Some(session) {
            return;
        }
        let unsaved = self.pins_for.is_none();
        self.pins_for = Some(session.to_path_buf());
        let Some(dir) = store::dir(cx) else { return };
        let before = std::mem::replace(&mut self.pins, store::pins(&dir, session));
        if unsaved && before.iter().any(|id| !self.pins.contains(id)) {
            for id in before {
                if !self.pins.contains(&id) {
                    self.pins.push(id);
                }
            }
            self.save_pins(Some(session), cx);
        }
        self.pinned_shown = self.pins.last().cloned();
        for (id, surface) in &self.surfaces {
            surface.set_env(self.root.as_deref(), self.pins.contains(id), cx);
        }
    }

    /// The ids pinned to the side panel, in pin order.
    #[cfg(test)]
    pub fn pins(&self) -> &[String] {
        &self.pins
    }

    /// The side panel with the pinned surfaces (tabs when there are
    /// several); `None` when nothing pinned is open.
    pub fn render_pinned(&self, cx: &App) -> Option<AnyElement> {
        let pinned: Vec<(String, Surface, String)> = self
            .pins
            .iter()
            .filter_map(|id| {
                let surface = self.surfaces.get(id)?;
                Some((id.clone(), surface.clone(), surface.title(cx)))
            })
            .collect();
        if pinned.is_empty() {
            return None;
        }
        let shown = self
            .pinned_shown
            .as_ref()
            .filter(|id| pinned.iter().any(|(p, ..)| p == *id))
            .or_else(|| pinned.last().map(|(id, ..)| id))
            .cloned()
            .unwrap_or_default();
        Some(pinned::panel(&pinned, &shown, self.expanded.as_deref()))
    }

    /// Open the templates manager (`view`, whose events `subscription`
    /// follows); `None` closes it.
    pub fn set_templates(&mut self, manager: Option<(Entity<TemplatesView>, Subscription)>) {
        self.templates = manager;
    }

    /// The templates manager over the top of the conversation, while open.
    /// `cover` goes in its island (see `LiveSlots::cover`).
    pub fn render_templates(&self, cover: impl IntoElement, cx: &App) -> Option<AnyElement> {
        let view = self.templates()?.clone();
        Some(
            div()
                .absolute()
                .top_4()
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    crate::shared::ui::island(cx)
                        .w(rems(440. / 16.))
                        .p_3()
                        .child(view)
                        .child(cover),
                )
                .into_any_element(),
        )
    }

    pub fn templates(&self) -> Option<&Entity<TemplatesView>> {
        self.templates.as_ref().map(|(view, _)| view)
    }

    /// `/gentle-ui:event {"op":"insert", …}` for `template`; its outcome is
    /// awaited by [`Self::outcome`].
    pub fn insert_command(&mut self, template: &genui::template::Template) -> String {
        self.inserting.insert(genui::template::stem(&template.name));
        template.insert_command()
    }

    /// Surfaces created since the last call, for the conversation to
    /// subscribe to (and hand back with [`Self::keep`]).
    pub fn take_fresh(&mut self) -> Vec<Surface> {
        std::mem::take(&mut self.fresh)
    }

    pub fn keep(&mut self, view: EntityId, subscription: Subscription) {
        self.subscriptions.insert(view, subscription);
    }

    /// Show `id` in place of the conversation (`None`: back to it).
    pub fn set_expanded(&mut self, id: Option<String>, cx: &mut App) {
        for (key, surface) in &self.surfaces {
            let on = id.as_ref() == Some(key);
            surface.set_obscured(self.obscured, cx);
            surface.set_expanded(on, cx);
        }
        if let Some(todo) = &self.todo {
            todo.set_obscured(self.obscured, cx);
            todo.set_expanded(id.as_deref() == Some(TODO_SURFACE), cx);
        }
        self.expanded = id.filter(|id| {
            self.surfaces.contains_key(id) || (id == TODO_SURFACE && self.todo.is_some())
        });
    }

    pub fn expanded(&self) -> Option<Surface> {
        let id = self.expanded.as_ref()?;
        match self.surfaces.get(id) {
            Some(surface) => Some(surface.clone()),
            None if id == TODO_SURFACE => self.todo.clone(),
            None => None,
        }
    }

    /// Esc in the session: back to the conversation when a surface is
    /// expanded and doesn't have the keyboard (one that has it collapses on
    /// its own Esc). Whether it collapsed one.
    pub fn collapse_on_escape(&mut self, window: &Window, cx: &mut App) -> bool {
        match self.expanded() {
            Some(surface) if !surface.has_focus(window, cx) => {
                self.set_expanded(None, cx);
                true
            }
            _ => false,
        }
    }

    /// Something is drawn over the conversation (a menu over the expanded
    /// surface) or it isn't on screen: native web views hide.
    pub fn set_obscured(&mut self, obscured: bool, cx: &mut App) {
        if self.obscured != obscured {
            self.obscured = obscured;
            if let Some(surface) = self.expanded() {
                surface.set_obscured(obscured, cx);
            }
        }
    }

    /// What a `gentle_ui` tool item shows: the live surface, or a row for
    /// a version still arriving, failed, closed or superseded.
    pub fn render_tool(&self, ix: usize, item: &Item, cx: &App) -> AnyElement {
        let Item::Tool {
            args,
            status,
            output,
            details,
            ..
        } = item
        else {
            return div().into_any_element();
        };
        let snapshot = genui::snapshot(details);
        let title = tool_title(args, snapshot.as_ref());
        match status {
            // A review that waits for the user's decisions shows while the
            // call runs (its partial result carries the surface).
            ToolStatus::Running if snapshot.is_none() => return view::drawing_card(&title),
            ToolStatus::Running => {}
            ToolStatus::Failed => return view::failed_card(&title, output),
            ToolStatus::Done => {}
        }
        if self.closed.contains(&ix) {
            return view::superseded_row(&title, 0, true);
        }
        match self.render_home(ix, &title, cx) {
            Some(live) => live,
            None => view::superseded_row(&title, snapshot.map_or(0, |s| s.version), false),
        }
    }

    /// The live surface whose card is drawn at item `ix`, or where it went
    /// (expanded, pinned).
    fn render_home(&self, ix: usize, title: &str, cx: &App) -> Option<AnyElement> {
        let id = self.homes.get(&ix)?;
        let surface = self.surfaces.get(id)?;
        if self.expanded.as_ref() == Some(id) {
            return Some(view::expanded_row(surface.clone(), title));
        }
        if self.pins.contains(id) {
            return Some(view::pinned_row(surface.clone(), title));
        }
        Some(
            div()
                .w_full()
                .h(rems(surface.card_height(cx) / 16.))
                .child(
                    surface
                        .view()
                        .cached(StyleRefinement::default().size_full()),
                )
                .into_any_element(),
        )
    }

    /// A `gentle-ui.event` message: the user's sent edits, a docs request,
    /// or a template they inserted (whose live card is drawn here).
    pub fn render_event(&self, ix: usize, details: &serde_json::Value, cx: &App) -> AnyElement {
        let title = details["title"].as_str().unwrap_or_default();
        self.render_home(ix, title, cx)
            .unwrap_or_else(|| view::event_card(details))
    }
}
