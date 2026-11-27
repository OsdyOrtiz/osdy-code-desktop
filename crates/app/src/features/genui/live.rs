//! Live panels: a table, chart or checklist whose spec has `live` (see
//! `gentle_core::genui::live`) shows data the app reads itself. Jobs,
//! subagents and the todo list are handed over by the conversation when
//! they change ([`LiveEnv::feeds`], [`super::GenUi::feed`]); git sources and
//! commands are read every interval, only while the card is drawn in the
//! active window and not paused, and a command never runs before the user
//! allows it on the card ([`Gate`]).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gentle_core::genui::live::{
    self, COMMAND_TIMEOUT, Consent, Decision, Gate, Live, OUTPUT_CAP, Source,
};
use gentle_core::tools::TodoTask;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::diagram::SendState;
use super::panel::{Body, PanelView};
use crate::features::git;
use crate::shared::ui;

/// How often a live card looks at the clock: it reads a source that is
/// due and redraws ("updated … ago"; a redraw also tells it is on screen).
const TICK: Duration = Duration::from_secs(5);

/// What the live cards of one conversation share.
#[derive(Default)]
pub struct LiveEnv {
    /// Where git sources and commands run.
    pub workspace: Option<PathBuf>,
    /// Commands the user allowed for the session ("Always for this session").
    pub allowed: HashSet<String>,
    /// The latest data of each pushed source (jobs, subagents, todo).
    pub feeds: HashMap<Source, Value>,
}

pub type SharedEnv = Rc<RefCell<LiveEnv>>;

/// One live card's source, consent and latest data.
pub(super) struct LiveState {
    pub spec: Live,
    /// Only for a command source.
    pub gate: Option<Gate>,
    pub data: Option<Value>,
    pub updated: Option<Instant>,
    pub error: Option<crate::shared::i18n::Message>,
    pub paused: bool,
    pub reading: bool,
    /// Reads started so far (git reads and command runs).
    pub reads: usize,
    /// Drawn since the last tick: the card is on screen.
    pub seen: bool,
    pub window: Option<AnyWindowHandle>,
    last_read: Option<Instant>,
    ticker: Option<Task<()>>,
}

impl LiveState {
    fn new(spec: Live, allowed: &HashSet<String>) -> Self {
        let gate = spec.command.as_ref().map(|c| {
            let key = c.key();
            let session = allowed.contains(&key);
            Gate::new(key, session)
        });
        Self {
            spec,
            gate,
            data: None,
            updated: None,
            error: None,
            paused: false,
            reading: false,
            reads: 0,
            seen: false,
            window: None,
            last_read: None,
            ticker: None,
        }
    }

    fn due(&self) -> bool {
        let interval = Duration::from_secs(self.spec.interval.into());
        self.last_read.is_none_or(|at| at.elapsed() >= interval)
    }

    fn may_read(&self) -> bool {
        !self.spec.source.pushed()
            && !self.paused
            && !self.reading
            && self.gate.as_ref().is_none_or(Gate::may_run)
    }

    /// The header's "Live · …".
    pub fn label(&self) -> String {
        use crate::shared::i18n::{language, t};
        let state = match self.gate.as_ref().map(Gate::consent) {
            Some(Consent::Ask) => t("waiting for your OK").to_string(),
            Some(Consent::Denied) => t("not allowed").to_string(),
            _ if self.paused => t("paused").to_string(),
            _ if self.error.is_some() => t("failed").to_string(),
            _ => match self.updated {
                Some(at) => t("updated {age}")
                    .replace("{age}", &live::ago_in(at.elapsed().as_secs(), language())),
                None if self.reading => t("reading…").to_string(),
                None => t("waiting for data").to_string(),
            },
        };
        t("Live · {state}").replace("{state}", &state)
    }
}

/// A subagent as a kanban card shows it (from the `subagents` records).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AgentStatus {
    pub id: String,
    pub agent: String,
    pub status: String,
    pub step: Option<String>,
    pub cost: Option<f64>,
    pub active: bool,
}

/// The `subagents` records as card statuses.
pub(super) fn agent_statuses(records: Option<&Value>) -> Vec<AgentStatus> {
    let text = |r: &Value, key: &str| {
        r[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    records
        .and_then(Value::as_array)
        .map(|records| {
            records
                .iter()
                .filter_map(|r| {
                    Some(AgentStatus {
                        id: text(r, "id")?,
                        agent: text(r, "agent").unwrap_or_default(),
                        status: text(r, "status").unwrap_or_default(),
                        step: text(r, "step"),
                        cost: r["cost"].as_f64(),
                        active: r["active"].as_bool() == Some(true),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The agent's todo list as `todo` records: `{title, status, note?}`.
pub fn todo_records(tasks: &[TodoTask]) -> Value {
    Value::Array(
        tasks
            .iter()
            .map(|t| {
                let mut record = json!({"title": t.title, "status": t.status});
                if let Some(note) = &t.note {
                    record["note"] = json!(note);
                }
                record
            })
            .collect(),
    )
}

/// One read of a read source, off the UI thread.
fn read(
    source: Source,
    live: &Live,
    workspace: &Path,
) -> Result<Value, crate::shared::i18n::Message> {
    use crate::shared::i18n::Message;
    match source {
        Source::GitStatus => {
            let snapshot =
                git::snapshot(workspace).ok_or_else(|| Message::new("not a git repository"))?;
            Ok(Value::Array(
                snapshot
                    .files
                    .iter()
                    .map(|f| {
                        json!({
                            "path": f.path,
                            "status": change_word(f.status),
                            "code": f.status.to_string(),
                            "added": f.added,
                            "removed": f.removed,
                        })
                    })
                    .collect(),
            ))
        }
        Source::GitLog => {
            let top =
                git::toplevel(workspace).ok_or_else(|| Message::new("not a git repository"))?;
            let history = git::history(&top, &git::Scope::Head, 50)
                .ok_or_else(|| Message::new("git log failed"))?;
            Ok(Value::Array(
                history
                    .commits
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.short(),
                            "hash": c.hash,
                            "short": c.short(),
                            "author": c.author,
                            "time": c.time,
                            "subject": c.subject,
                            "files": c.stat.as_ref().map(|s| s.files),
                            "insertions": c.stat.as_ref().map(|s| s.insertions),
                            "deletions": c.stat.as_ref().map(|s| s.deletions),
                        })
                    })
                    .collect(),
            ))
        }
        Source::Command => {
            let command = live
                .command
                .as_ref()
                .ok_or_else(|| Message::new("no command"))?;
            let dir = match &command.cwd {
                Some(cwd) => workspace.join(cwd),
                None => workspace.to_path_buf(),
            };
            let output = run_command(&command.cmd, &dir)?;
            live::parse_output_with_details(command.parse, &output).map_err(|error| {
                Message::with(
                    "the output isn't JSON ({error}): {output}",
                    [
                        ("{error}", error.error.to_string()),
                        ("{output}", error.preview),
                    ],
                )
            })
        }
        Source::Jobs | Source::Subagents | Source::Todo => {
            Err(Message::new("handed over, not read"))
        }
    }
}

/// A porcelain status letter as a word.
fn change_word(code: char) -> &'static str {
    match code {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        '?' => "untracked",
        'U' => "conflicted",
        _ => "changed",
    }
}

/// `cmd` run by the user's shell in `dir` with the app's PATH: its stdout
/// (any exit code: test runners exit 1 with a report), stopped after
/// [`COMMAND_TIMEOUT`] or past [`OUTPUT_CAP`] bytes.
fn run_command(cmd: &str, dir: &Path) -> Result<String, crate::shared::i18n::Message> {
    use crate::shared::i18n::Message;
    let output_limit = || {
        Message::with(
            "its output is over {limit} KiB",
            [("{limit}", (OUTPUT_CAP / 1024).to_string())],
        )
    };
    if !dir.is_dir() {
        return Err(Message::with(
            "{path} is not a folder",
            [("{path}", dir.display().to_string())],
        ));
    }
    #[cfg(unix)]
    let mut command = {
        use std::os::unix::process::CommandExt as _;
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty() && Path::new(s).exists())
            .unwrap_or_else(|| "/bin/sh".to_string());
        let mut command = pi_rpc::command(shell);
        command.arg("-c").arg(cmd);
        // Its own process group, so a timeout stops what it started too.
        command.process_group(0);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = pi_rpc::command("cmd");
        command.arg("/C").arg(cmd);
        command
    };
    command
        .current_dir(dir)
        .env("PATH", pi_rpc::child_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| Message::with_error("couldn't start: {error}", e))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stdout) = stdout {
            let _ = stdout.take(OUTPUT_CAP as u64 + 1).read_to_end(&mut bytes);
        }
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(stderr) = stderr {
            let _ = stderr.take(64 * 1024).read_to_string(&mut text);
        }
        text
    });
    let deadline = Instant::now() + Duration::from_secs(COMMAND_TIMEOUT);
    let mut out = Some(out);
    let mut bytes = None;
    let status = loop {
        // Output past the cap stops the command rather than waiting on it.
        if out.as_ref().is_some_and(|reader| reader.is_finished()) {
            let read = out.take().and_then(|r| r.join().ok()).unwrap_or_default();
            if read.len() > OUTPUT_CAP {
                pi_rpc::kill_tree(&mut child);
                return Err(output_limit());
            }
            bytes = Some(read);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                pi_rpc::kill_tree(&mut child);
                return Err(Message::with(
                    "stopped after {seconds} s",
                    [("{seconds}", COMMAND_TIMEOUT.to_string())],
                ));
            }
        }
    };
    let bytes = match bytes {
        Some(bytes) => bytes,
        None => out.and_then(|r| r.join().ok()).unwrap_or_default(),
    };
    if bytes.len() > OUTPUT_CAP {
        return Err(output_limit());
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if text.trim().is_empty() && !status.success() {
        let stderr = err.join().unwrap_or_default();
        let last = stderr.lines().rev().find(|l| !l.trim().is_empty());
        return Err(Message::raw(match last {
            Some(line) => format!("{status}: {}", line.trim()),
            None => status.to_string(),
        }));
    }
    Ok(text)
}

impl PanelView {
    /// Join the conversation's live data (and the session's allowed
    /// commands): a pushed source gets its latest data at once.
    pub fn attach(&mut self, env: SharedEnv, cx: &mut Context<Self>) {
        self.live_env = Some(env);
        self.sync_live(cx);
        self.agents = agent_statuses(self.feed_data(Source::Subagents).as_ref());
        cx.notify();
    }

    fn feed_data(&self, source: Source) -> Option<Value> {
        self.live_env.as_ref()?.borrow().feeds.get(&source).cloned()
    }

    /// Whether this card shows `source` (kanban cards follow subagents).
    pub fn wants(&self, source: Source) -> bool {
        self.live.as_ref().is_some_and(|l| l.spec.source == source)
            || (source == Source::Subagents
                && matches!(&self.body, Some(Body::Kanban(s))
                    if s.edits.working().cards.iter().any(|c| c.agent_id.is_some())))
    }

    /// `source`'s data changed (see [`LiveEnv::feeds`]).
    pub fn feed(&mut self, source: Source, cx: &mut Context<Self>) {
        let data = self.feed_data(source);
        if source == Source::Subagents {
            self.agents = agent_statuses(data.as_ref());
            cx.notify();
        }
        if let (Some(state), Some(data)) = (&mut self.live, data)
            && state.spec.source == source
        {
            state.data = Some(data);
            state.updated = Some(Instant::now());
            state.error = None;
            self.apply_live(cx);
        }
    }

    /// Follow the spec's `live` (first version, or the agent changed it):
    /// a new source starts over, a new command is asked about again.
    pub(super) fn sync_live(&mut self, cx: &mut Context<Self>) {
        let wanted = self.body.as_ref().and_then(Body::live).cloned();
        let allowed = self
            .live_env
            .as_ref()
            .map(|e| e.borrow().allowed.clone())
            .unwrap_or_default();
        match (&mut self.live, wanted) {
            (live, None) => *live = None,
            (Some(state), Some(spec)) if state.spec.source == spec.source => {
                if let (Some(gate), Some(command)) = (&mut state.gate, &spec.command) {
                    let key = command.key();
                    let session = allowed.contains(&key);
                    gate.retarget(&key, session);
                }
                state.spec = spec;
            }
            (live, Some(spec)) => *live = Some(LiveState::new(spec, &allowed)),
        }
        let feed = self
            .live
            .as_ref()
            .filter(|l| l.spec.source.pushed())
            .and_then(|l| self.feed_data(l.spec.source));
        let ticker = match &self.live {
            Some(state) if state.ticker.is_none() => Some(self.start_ticker(cx)),
            _ => None,
        };
        if let Some(state) = &mut self.live {
            if let Some(data) = feed
                && state.data.as_ref() != Some(&data)
            {
                state.data = Some(data);
                state.updated = Some(Instant::now());
            }
            if ticker.is_some() {
                state.ticker = ticker;
            }
        }
        self.apply_live(cx);
    }

    /// Put the latest data in the spec (the user's unsent edits replayed on
    /// it); waits while edits are being sent, and while paused.
    pub(super) fn apply_live(&mut self, cx: &mut Context<Self>) {
        if matches!(self.send, SendState::Sending { .. }) {
            return;
        }
        let Some(state) = &mut self.live else { return };
        let (Some(data), false) = (&state.data, state.paused) else {
            return;
        };
        let Some(body) = &mut self.body else { return };
        let filled = live::fill(&self.kind, &body.base_value(), &state.spec, data);
        if !body.holds(&filled)
            && let Err(errors) = body.rebase(&filled)
        {
            state.error = Some(crate::shared::i18n::Message::raw(errors.join("; ")));
        }
        cx.notify();
    }

    /// Read a read source now, off the UI thread (the caller redraws).
    pub(super) fn read_live(&mut self, cx: &mut Context<Self>) {
        let workspace = self
            .live_env
            .as_ref()
            .and_then(|e| e.borrow().workspace.clone());
        let Some(state) = &mut self.live else { return };
        if !state.may_read() {
            return;
        }
        let Some(workspace) = workspace else {
            state.error = Some(crate::shared::i18n::Message::new(
                "no workspace folder to read from",
            ));
            return;
        };
        let (source, spec) = (state.spec.source, state.spec.clone());
        state.reading = true;
        state.reads += 1;
        state.last_read = Some(Instant::now());
        let task = cx
            .background_executor()
            .spawn(async move { read(source, &spec, &workspace) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.finish_read(result, cx));
        })
        .detach();
    }

    fn finish_read(
        &mut self,
        result: Result<Value, crate::shared::i18n::Message>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.live else { return };
        state.reading = false;
        match result {
            Ok(data) => {
                state.data = Some(data);
                state.updated = Some(Instant::now());
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
        self.apply_live(cx);
        cx.notify();
    }

    fn start_ticker(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                crate::shared::diagnostics::tick();
                let Ok(window) = this.read_with(cx, |this, _| this.live.as_ref()?.window) else {
                    break;
                };
                let active = window
                    .and_then(|w| {
                        cx.update_window(w, |_, window, _| window.is_window_active())
                            .ok()
                    })
                    .unwrap_or(false);
                if this
                    .update(cx, |this, cx| this.live_tick(active, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
    }

    /// Every [`TICK`]: nothing while the window is in the background; a
    /// due source is read only if the card was drawn since the last tick.
    fn live_tick(&mut self, active: bool, cx: &mut Context<Self>) {
        let allowed = self.live_env.as_ref().is_some_and(|env| {
            self.live_key()
                .is_some_and(|k| env.borrow().allowed.contains(&k))
        });
        let Some(state) = &mut self.live else { return };
        let seen = std::mem::take(&mut state.seen);
        if !active {
            return;
        }
        if allowed && let Some(gate) = &mut state.gate {
            gate.session_allowed();
        }
        if seen && state.due() {
            self.read_live(cx);
        }
        cx.notify();
    }

    fn live_key(&self) -> Option<String> {
        Some(self.live.as_ref()?.gate.as_ref()?.key().to_string())
    }

    /// Called while drawing: the card is on screen.
    pub(super) fn live_drawn(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.live else { return };
        state.seen = true;
        state.window = Some(window.window_handle());
        // The first read waits until the card is first drawn.
        if state.last_read.is_none() && state.may_read() {
            self.read_live(cx);
        }
    }

    /// The user's answer about a command source.
    pub(super) fn decide(&mut self, decision: Decision, cx: &mut Context<Self>) {
        let Some(gate) = self.live.as_mut().and_then(|l| l.gate.as_mut()) else {
            return;
        };
        gate.decide(decision);
        let key = gate.key().to_string();
        let run = gate.may_run();
        if decision == Decision::Session
            && let Some(env) = &self.live_env
        {
            env.borrow_mut().allowed.insert(key);
        }
        if run {
            self.read_live(cx);
        }
        cx.notify();
    }

    fn ask_again(&mut self, cx: &mut Context<Self>) {
        if let Some(gate) = self.live.as_mut().and_then(|l| l.gate.as_mut()) {
            gate.ask_again();
            cx.notify();
        }
    }

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &mut self.live else { return };
        state.paused = !state.paused;
        if state.paused {
            cx.notify();
            return;
        }
        // Resumed: what arrived meanwhile, and a read if one is due.
        self.apply_live(cx);
        if self.live.as_ref().is_some_and(LiveState::due) {
            self.read_live(cx);
        }
        cx.notify();
    }

    // ── Drawing ────────────────────────────────────────────────────────────

    /// The header's "Live · updated 5 s ago" and pause/resume.
    pub(super) fn render_live_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.live.as_ref()?;
        let color = if state.error.is_some() {
            ui::danger()
        } else if state.paused || state.gate.as_ref().is_some_and(|g| !g.may_run()) {
            ui::faint()
        } else {
            ui::success()
        };
        let tip = state
            .error
            .as_ref()
            .map(crate::shared::i18n::Message::render)
            .unwrap_or_else(|| {
                crate::shared::i18n::t("From {source}")
                    .replace("{source}", &source_text(&state.spec))
            });
        let consented = state.gate.as_ref().is_none_or(Gate::may_run);
        Some(
            h_flex()
                .flex_shrink_0()
                .gap_1()
                .child(
                    h_flex()
                        .id("genui-live")
                        .debug_selector(|| "genui-live".into())
                        .gap_1()
                        .px_1p5()
                        .rounded_md()
                        .bg(color.opacity(0.12))
                        .text_size(rems(11. / 16.))
                        .text_color(color)
                        .child(Icon::new(IconName::Activity).xsmall())
                        .child(state.label())
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tip.clone())
                                .build(window, cx)
                        }),
                )
                .when(consented, |d| {
                    d.child(
                        div().debug_selector(|| "genui-live-pause".into()).child(
                            Button::new("genui-live-pause")
                                .ghost()
                                .xsmall()
                                .text_color(ui::muted())
                                .icon(Icon::new(if state.paused {
                                    IconName::Play
                                } else {
                                    IconName::Pause
                                }))
                                .tooltip(crate::shared::i18n::t(if state.paused {
                                    "Resume"
                                } else {
                                    "Pause"
                                }))
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_pause(cx))),
                        ),
                    )
                })
                .into_any_element(),
        )
    }

    /// A command source's question, over the top of the body, until the
    /// user answers; a denial leaves a strip to ask again.
    pub(super) fn render_consent(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.live.as_ref()?;
        let gate = state.gate.as_ref()?;
        let command = state.spec.command.as_ref()?;
        let code = |text: String| {
            div()
                .font_family(ui::mono())
                .text_size(ui::code_rems(12.))
                .text_color(ui::fg())
                .child(text)
        };
        let schedule =
            crate::shared::i18n::t("every {seconds} s in {path}, while this card is on screen")
                .replace("{seconds}", &state.spec.interval.to_string())
                .replace(
                    "{path}",
                    command
                        .cwd
                        .as_deref()
                        .unwrap_or_else(|| crate::shared::i18n::t("the workspace")),
                );
        let panel = div().absolute().left_2().right_2().top_2();
        match gate.consent() {
            Consent::Ask => Some(
                panel
                    .child(
                        ui::card()
                            .debug_selector(|| "genui-live-consent".into())
                            .px_3()
                            .py_2()
                            .gap_2()
                            .bg(ui::popover_bg())
                            .border_color(ui::warning().opacity(0.4))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .text_size(rems(12. / 16.))
                                    .text_color(ui::warning())
                                    .child(Icon::new(IconName::Terminal).xsmall())
                                    .child(crate::shared::i18n::t(
                                        "Allow this UI to run a command?",
                                    )),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .text_size(rems(12. / 16.))
                                    .text_color(ui::muted())
                                    .child(code(command.cmd.clone()).whitespace_normal())
                                    .child(schedule),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(div().flex_1())
                                    .child(consent_button(
                                        "genui-live-deny",
                                        "Deny",
                                        false,
                                        cx.listener(|this, _, _, cx| {
                                            this.decide(Decision::Deny, cx)
                                        }),
                                    ))
                                    .child(consent_button(
                                        "genui-live-allow-session",
                                        "Always for this session",
                                        false,
                                        cx.listener(|this, _, _, cx| {
                                            this.decide(Decision::Session, cx)
                                        }),
                                    ))
                                    .child(consent_button(
                                        "genui-live-allow-once",
                                        "Allow once",
                                        true,
                                        cx.listener(|this, _, _, cx| {
                                            this.decide(Decision::Once, cx)
                                        }),
                                    )),
                            ),
                    )
                    .into_any_element(),
            ),
            Consent::Denied => Some(
                panel
                    .child(
                        h_flex()
                            .px_3()
                            .py_1()
                            .gap_2()
                            .rounded_lg()
                            .bg(ui::popover_bg())
                            .border_1()
                            .border_color(ui::border())
                            .text_size(rems(12. / 16.))
                            .text_color(ui::muted())
                            .child(Icon::new(IconName::Ban).xsmall())
                            .child(
                                div().flex_1().min_w_0().truncate().child(
                                    crate::shared::i18n::t("Not running {command}")
                                        .replace("{command}", &command.cmd),
                                ),
                            )
                            .child(consent_button(
                                "genui-live-ask-again",
                                "Ask again",
                                false,
                                cx.listener(|this, _, _, cx| this.ask_again(cx)),
                            )),
                    )
                    .into_any_element(),
            ),
            Consent::Once | Consent::Session => None,
        }
    }
}

fn consent_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let button = Button::new(id)
        .xsmall()
        .label(crate::shared::i18n::t(label))
        .on_click(on_click);
    div().debug_selector(move || id.into()).child(if primary {
        button.primary()
    } else {
        button.ghost()
    })
}

/// "the background jobs", "`npm test` every 30 s"… for the chip's tooltip.
fn source_text(live: &Live) -> String {
    use crate::shared::i18n::t;
    let command = match (live.source, &live.command) {
        (Source::Jobs, _) => return t("the session's background jobs").into(),
        (Source::Subagents, _) => return t("the session's subagents").into(),
        (Source::Todo, _) => return t("the agent's todo list").into(),
        (Source::GitStatus, _) => "git status",
        (Source::GitLog, _) => "git log",
        (Source::Command, Some(c)) => &c.cmd,
        (Source::Command, None) => return t("a command").into(),
    };
    t("{command}, every {seconds} s")
        .replace("{seconds}", &live.interval.to_string())
        .replace("{command}", command)
}
