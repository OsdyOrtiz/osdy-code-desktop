//! Gentle Tower in a tab. Gentle Shell's Tower is a local web app that draws
//! every open session as a floor of its project's building (tasks, talk,
//! artifacts, Engram, cost, permissions). The tab runs `gentle-shell tower
//! --no-open --json` while it exists and shows the page it serves in the
//! app's web view; the app's own agents already publish their floors.
//! `/gentle:tower` opens this tab instead of the system browser.
//!
//! `--json` (Gentle Shell 5.0.0 on) prints one `gentle-shell.tower/v1` line
//! with the URL. Support is detected by trying it: an older Shell refuses the
//! flag as an unknown argument before starting anything, and the tab then
//! runs it again without the flag and reads the `Gentle Tower: <url>` text,
//! as before. The refusal is remembered until the app quits, so an older
//! Shell costs that one extra start once.

use std::io::{BufRead as _, BufReader, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use gentle_core::tower::{ready_url, refused_json_flag};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::{browser::WebSurface, setup::Detail};
use crate::shared::{
    i18n::{language, t},
    ui,
};

/// A Shell refused `tower --json` (older than 5.0.0): later starts in this
/// run go straight to the text output.
static JSON_REFUSED: AtomicBool = AtomicBool::new(false);

enum State {
    Starting,
    Ready(Entity<WebSurface>),
    /// Building the web app Gentle Shell shipped without; the latest line.
    Building(Option<String>),
    Failed(Detail),
}

/// The running `gentle-shell tower`. Closing its input stops it: the app
/// quitting or crashing closes it too, so no server outlives the app.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stdin.take();
        if cfg!(not(unix)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

/// `sh` that runs its arguments in the background, stops them once its
/// stdin closes (a watcher off the output pipes, so they close when the
/// server ends), and exits with them.
const TIED: &str = concat!(
    r#"exec 3<&0; "$@" <&- & child=$!; "#,
    r#"{ read -r _ <&3; kill "$child"; } >/dev/null 2>&1 & "#,
    r#"wait "$child""#,
);

/// `command` run so that it ends when its input closes. Unix wraps it in
/// [`TIED`]; Windows ends it on drop.
fn tied_to_stdin(command: Command) -> Command {
    if cfg!(not(unix)) {
        return command;
    }
    let mut tied = Command::new("/bin/sh");
    tied.arg("-c")
        .arg(TIED)
        .arg("sh")
        .arg(command.get_program())
        .args(command.get_args());
    if let Some(dir) = command.get_current_dir() {
        tied.current_dir(dir);
    }
    for (key, value) in command.get_envs() {
        match value {
            Some(value) => tied.env(key, value),
            None => tied.env_remove(key),
        };
    }
    tied
}

/// Reads `pipe` to its end on a thread of its own, so the server never
/// blocks writing to it.
fn drain(pipe: impl std::io::Read + Send + 'static) {
    std::thread::spawn(move || std::io::copy(&mut { pipe }, &mut std::io::sink()));
}

/// Starts the server for `cwd` and waits for its URL (blocking: run off the
/// main thread): with `--json` unless a Shell refused it before, and again
/// without it when this one refuses it.
fn start(cwd: PathBuf) -> Result<(Server, String), Detail> {
    if !JSON_REFUSED.load(Ordering::Relaxed) {
        match serve(&cwd, true) {
            Err(Detail::Raw(why)) if refused_json_flag(&why) => {
                JSON_REFUSED.store(true, Ordering::Relaxed)
            }
            served => return served,
        }
    }
    serve(&cwd, false)
}

/// Runs `gentle-shell tower --no-open` (with `--json` when `json`) until it
/// prints its URL. Where the folder's agents run in a backend, the server
/// runs there, tied to the same input inside it (stopping the local
/// `wsl.exe` or `docker` wouldn't stop it); its page is reached through
/// the backend's forwarded localhost.
fn serve(cwd: &Path, json: bool) -> Result<(Server, String), Detail> {
    let args: &[&str] = if json {
        &["tower", "--no-open", "--json"]
    } else {
        &["tower", "--no-open"]
    };
    let in_backend: Vec<&str> = ["sh", "-c", TIED, "sh", "gentle-shell"]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
    let mut command = match crate::features::backend::in_folder(cwd, &in_backend) {
        Some(command) => command,
        None => {
            let spec = pi_rpc::resolve_backend(cwd).map_err(|e| format!("{e:#}"))?;
            tied_to_stdin(spec.launcher_command(args.iter().copied()))
        }
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            Detail::template(
                "Couldn't start gentle-shell tower: {error}",
                [("{error}", e.to_string())],
            )
        })?;
    let stdin = child.stdin.take();
    let mut stdout = BufReader::new(child.stdout.take().ok_or("no output")?);
    let mut stderr = child.stderr.take().ok_or("no error output")?;
    let mut line = String::new();
    while matches!(stdout.read_line(&mut line), Ok(n) if n > 0) {
        if let Some(url) = ready_url(&line) {
            drain(stdout);
            drain(stderr);
            return Ok((Server { child, stdin }, url));
        }
        line.clear();
    }
    // It ended without serving: say why, from its error output.
    let mut why = String::new();
    let _ = stderr.read_to_string(&mut why);
    drop(stdin);
    let _ = child.wait();
    Err(match why.trim() {
        "" => "gentle-shell tower ended without serving. Is your Gentle Shell up to date?".into(),
        why => why.to_string().into(),
    })
}

pub struct TowerView {
    cwd: PathBuf,
    state: State,
    server: Option<Server>,
    /// Its tab is the one on screen.
    shown: bool,
    /// A screen, dialog or popover of the window is over it.
    covered: bool,
    focus: FocusHandle,
    _start: Task<()>,
    _quit: Subscription,
}

impl Focusable for TowerView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TowerView {
    pub fn new(cwd: PathBuf, cx: &mut Context<Self>) -> Self {
        let quit = cx.on_app_quit(|this, _| {
            this.server.take();
            async {}
        });
        let mut this = Self {
            cwd,
            state: State::Starting,
            server: None,
            // The workspace shows it once its tab is on screen.
            shown: false,
            covered: false,
            focus: cx.focus_handle(),
            _start: Task::ready(()),
            _quit: quit,
        };
        this.start(cx);
        this
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        self.state = State::Starting;
        self.server = None;
        let cwd = self.cwd.clone();
        self._start = cx.spawn(async move |this, cx| {
            let started = cx
                .background_executor()
                .spawn(async move { start(cwd) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match started {
                    Ok((server, url)) => {
                        this.server = Some(server);
                        let (shown, covered) = (this.shown, this.covered);
                        let surface = cx.new(|cx| {
                            let mut surface = WebSurface::new(url, None);
                            surface.set_covered(covered, cx);
                            if !shown {
                                surface.hide(cx);
                            }
                            surface
                        });
                        this.state = State::Ready(surface);
                    }
                    Err(why) => this.state = State::Failed(why),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    /// Builds the missing web app (`setup::tower`), then starts again.
    fn build(&mut self, cx: &mut Context<Self>) {
        self.state = State::Building(None);
        let (tx, rx) = async_channel::unbounded::<String>();
        self._start = cx.spawn(async move |this, cx| {
            let done = cx.background_executor().spawn(async move {
                crate::features::setup::tower::install(&move |line: String| {
                    let _ = tx.send_blocking(line);
                })
            });
            while let Ok(line) = rx.recv().await {
                let alive = this.update(cx, |this, cx| {
                    this.state = State::Building(Some(line));
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            let result = done.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => this.start(cx),
                Err(why) => {
                    this.state = State::Failed(why.into());
                    cx.notify();
                }
            });
        });
        cx.notify();
    }

    pub fn title(&self) -> SharedString {
        "Tower".into()
    }

    /// The web view floats over everything GPUI draws: a hidden tab drops it
    /// (and the page), anything over it only conceals it, so the page keeps
    /// its place. Called every frame; only changes reach the page.
    pub fn set_shown(&mut self, shown: bool, covered: bool, cx: &mut Context<Self>) {
        if (shown, covered) == (self.shown, self.covered) {
            return;
        }
        let was_shown = self.shown;
        self.shown = shown;
        self.covered = covered;
        if let State::Ready(surface) = &self.state {
            surface.update(cx, |surface, cx| {
                surface.set_covered(covered, cx);
                match (was_shown, shown) {
                    (false, true) => surface.show(cx),
                    (true, false) => surface.hide(cx),
                    _ => {}
                }
            });
        }
    }
}

impl Render for TowerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::shared::diagnostics::render("TowerView");
        let body = match &self.state {
            State::Ready(surface) => {
                return div()
                    .track_focus(&self.focus)
                    .size_full()
                    .child(surface.clone())
                    .into_any_element();
            }
            State::Starting => v_flex()
                .gap_3()
                .items_center()
                .child(ui::spinner(gpui_kit::component::Size::Medium, ui::muted()))
                .child(
                    div()
                        .text_sm()
                        .text_color(ui::muted())
                        .child(t("Starting Gentle Tower…")),
                ),
            State::Building(line) => v_flex()
                .gap_3()
                .items_center()
                .max_w(rems(36.))
                .child(ui::spinner(gpui_kit::component::Size::Medium, ui::muted()))
                .child(
                    div()
                        .text_sm()
                        .text_color(ui::fg())
                        .child(t("Building Gentle Tower (a few minutes, once)…")),
                )
                .child(
                    div()
                        .text_size(ui::code_rems(12.))
                        .font_family(ui::mono())
                        .text_color(ui::muted())
                        .child(line.clone().unwrap_or_else(|| t("Starting…").into())),
                ),
            State::Failed(why) => {
                // Gentle Shell 4.9.0 shipped without Tower's web app.
                let missing_web =
                    matches!(why, Detail::Raw(raw) if raw.contains("web app is not built"));
                v_flex()
                    .gap_3()
                    .items_center()
                    .max_w(rems(36.))
                    .child(
                        div()
                            .text_sm()
                            .text_color(ui::fg())
                            .child(t(if missing_web {
                                "This Gentle Shell came without Tower's web app"
                            } else {
                                "Gentle Tower didn't start"
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(ui::muted())
                            .when(missing_web, |d| {
                                d.child(t(
                                    "Enable builds it from the same version's source on GitHub \
                                       and adds it to your install. It takes a few minutes, once.",
                                ))
                            })
                            .when(!missing_web, |d| {
                                d.font_family(ui::mono())
                                    .text_size(ui::code_rems(12.))
                                    .child(why.text(language()))
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .when(missing_web, |d| {
                                d.child(
                                    Button::new("tower-enable")
                                        .small()
                                        .primary()
                                        .label(t("Enable Gentle Tower"))
                                        .on_click(cx.listener(|this, _, _, cx| this.build(cx))),
                                )
                            })
                            .child(
                                Button::new("tower-retry")
                                    .small()
                                    .outline()
                                    .label(t("Try again"))
                                    .on_click(cx.listener(|this, _, _, cx| this.start(cx))),
                            ),
                    )
            }
        };
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(body)
            .into_any_element()
    }
}
