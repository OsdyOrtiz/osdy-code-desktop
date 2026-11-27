//! The panel kinds (table, form, checklist, kanban, chart): one view with
//! the card chrome they share, the diagram card's frame (header with the
//! kind's icon, title, version and tools; the body; a footer with the
//! pending changes, a note, Discard and Send to agent) around the kind's
//! own state. Edits go through `gentle_core::genui::edits`, so what the
//! user does is exactly the change list sent to the agent.

use gentle_core::genui::chart::Chart;
use gentle_core::genui::checklist::Checklist;
use gentle_core::genui::edits::Editable as _;
use gentle_core::genui::form::Form;
use gentle_core::genui::kanban::{self, Board};
use gentle_core::genui::live::Live;
use gentle_core::genui::review::{Decision, Review};
use gentle_core::genui::table::Table;
use gentle_core::genui::{self, Outcome, Snapshot};
use gentle_core::genui::{erd, sequence, states, timeline};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::chart::ChartState;
use super::checklist::ChecklistState;
use super::diagram::{DiagramEvent, SendState};
use super::erd::ErdState;
use super::form::FormState;
use super::kanban::KanbanState;
use super::live::{AgentStatus, LiveState, SharedEnv};
use super::motion::Glides;
use super::review::ReviewState;
use super::sequence::SequenceState;
use super::states::MachineState;
use super::table::TableState;
use super::timeline::TimelineState;
use crate::shared::ui;

/// Header and footer heights (px at 100 %), for the card's height.
pub(super) const CHROME: f32 = 38. + 38.;
/// The tallest a panel gets in the conversation (px at 100 %); more
/// scrolls inside, or opens expanded.
pub(super) const MAX_HEIGHT: f32 = 480.;

/// Wire kind, default title, header icon.
const KINDS: [(&str, &str, IconName); 10] = [
    ("table", "Table", IconName::Table),
    ("form", "Form", IconName::ClipboardList),
    ("checklist", "Checklist", IconName::ListChecks),
    ("kanban", "Board", IconName::SquareKanban),
    ("chart", "Chart", IconName::ChartColumn),
    ("review", "Review", IconName::GitCompare),
    ("sequence", "Sequence", IconName::ArrowLeftRight),
    ("erd", "Data model", IconName::Database),
    ("state", "State machine", IconName::CircleDot),
    ("timeline", "Timeline", IconName::ChartGantt),
];

/// Whether `kind` is drawn by a [`PanelView`].
pub(super) fn handles(kind: &str) -> bool {
    KINDS.iter().any(|(k, ..)| *k == kind)
}

/// The title a surface of `kind` gets when the agent gives none.
pub(super) fn default_title(kind: &str) -> &'static str {
    crate::shared::i18n::t(
        KINDS
            .iter()
            .find(|(k, ..)| *k == kind)
            .map_or("Diagram", |(_, title, _)| title),
    )
}

fn icon(kind: &str) -> IconName {
    KINDS
        .iter()
        .find(|(k, ..)| *k == kind)
        .map_or(IconName::Workflow, |(.., icon)| *icon)
}

/// The kind's state: what the agent sent, the user's edits and the view's
/// own (sort, selection, drag…).
pub(super) enum Body {
    Table(TableState),
    Form(FormState),
    Checklist(ChecklistState),
    Kanban(KanbanState),
    Chart(ChartState),
    Review(ReviewState),
    Sequence(SequenceState),
    Erd(ErdState),
    State(MachineState),
    Timeline(TimelineState),
}

/// `$e` bound to the kind's edits (`mut`: mutably); charts have none.
macro_rules! edits {
    ($body:expr, $e:ident => $then:expr, $chart:expr) => {
        match $body {
            Body::Table(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Form(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Checklist(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Kanban(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Review(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Sequence(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Erd(s) => {
                let $e = &s.edits;
                $then
            }
            Body::State(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Timeline(s) => {
                let $e = &s.edits;
                $then
            }
            Body::Chart(_) => $chart,
        }
    };
    (mut $body:expr, $e:ident => $then:expr, $chart:expr) => {
        match $body {
            Body::Table(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Form(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Checklist(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Kanban(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Review(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Sequence(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Erd(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::State(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Timeline(s) => {
                let $e = &mut s.edits;
                $then
            }
            Body::Chart(_) => $chart,
        }
    };
}

impl Body {
    fn parse(kind: &str, spec: &Value) -> Result<Body, Vec<String>> {
        Ok(match kind {
            "table" => Body::Table(TableState::new(Table::parse(spec)?)),
            "form" => Body::Form(FormState::new(Form::parse(spec)?, spec)),
            "checklist" => Body::Checklist(ChecklistState::new(Checklist::parse(spec)?)),
            "kanban" => Body::Kanban(KanbanState::new(Board::parse(spec)?)),
            "chart" => Body::Chart(ChartState::new(Chart::parse(spec)?)),
            "review" => Body::Review(ReviewState::new(Review::parse(spec)?)),
            "sequence" => Body::Sequence(SequenceState::new(sequence::Sequence::parse(spec)?)),
            "erd" => Body::Erd(ErdState::new(erd::Erd::parse(spec)?)),
            "state" => Body::State(MachineState::new(states::Machine::parse(spec)?)),
            "timeline" => Body::Timeline(TimelineState::new(timeline::Timeline::parse(spec)?)),
            other => return Err(vec![format!("unknown kind \"{other}\"")]),
        })
    }

    /// A newer version from the agent (or live data); the unsent edits
    /// are replayed on it.
    pub(super) fn rebase(&mut self, spec: &Value) -> Result<(), Vec<String>> {
        match self {
            Body::Table(s) => s.rebase(Table::parse(spec)?),
            Body::Form(s) => s.rebase(Form::parse(spec)?, spec),
            Body::Checklist(s) => s.rebase(Checklist::parse(spec)?),
            Body::Kanban(s) => s.rebase(Board::parse(spec)?),
            Body::Chart(s) => s.rebase(Chart::parse(spec)?),
            Body::Review(s) => s.rebase(Review::parse(spec)?),
            Body::Sequence(s) => s.rebase(sequence::Sequence::parse(spec)?),
            Body::Erd(s) => s.rebase(erd::Erd::parse(spec)?),
            Body::State(s) => s.rebase(states::Machine::parse(spec)?),
            Body::Timeline(s) => s.rebase(timeline::Timeline::parse(spec)?),
        }
        Ok(())
    }

    /// Whether `spec` parses to what this already holds.
    pub(super) fn holds(&self, spec: &Value) -> bool {
        match self {
            Body::Table(s) => Table::parse(spec).is_ok_and(|t| &t == s.edits.base()),
            Body::Form(s) => Form::parse(spec).is_ok_and(|f| &f == s.edits.base()),
            Body::Checklist(s) => Checklist::parse(spec).is_ok_and(|c| &c == s.edits.base()),
            Body::Kanban(s) => Board::parse(spec).is_ok_and(|b| &b == s.edits.base()),
            Body::Chart(s) => Chart::parse(spec).is_ok_and(|c| c == s.chart),
            Body::Review(s) => Review::parse(spec).is_ok_and(|r| &r == s.edits.base()),
            Body::Sequence(s) => {
                sequence::Sequence::parse(spec).is_ok_and(|q| &q == s.edits.base())
            }
            Body::Erd(s) => erd::Erd::parse(spec).is_ok_and(|e| &e == s.edits.base()),
            Body::State(s) => states::Machine::parse(spec).is_ok_and(|m| &m == s.edits.base()),
            Body::Timeline(s) => {
                timeline::Timeline::parse(spec).is_ok_and(|t| &t == s.edits.base())
            }
        }
    }

    /// The agent's version as its wire JSON (what live data fills).
    pub(super) fn base_value(&self) -> Value {
        match self {
            Body::Table(s) => s.edits.base().to_value(),
            Body::Form(s) => s.edits.base().to_value(),
            Body::Checklist(s) => s.edits.base().to_value(),
            Body::Kanban(s) => s.edits.base().to_value(),
            Body::Chart(s) => s.chart.to_value(),
            Body::Review(s) => s.edits.base().to_value(),
            Body::Sequence(s) => s.edits.base().to_value(),
            Body::Erd(s) => s.edits.base().to_value(),
            Body::State(s) => s.edits.base().to_value(),
            Body::Timeline(s) => s.edits.base().to_value(),
        }
    }

    /// The spec's `live` (tables, charts and checklists).
    pub(super) fn live(&self) -> Option<&Live> {
        match self {
            Body::Table(s) => s.edits.base().live.as_deref(),
            Body::Checklist(s) => s.edits.base().live.as_deref(),
            Body::Chart(s) => s.chart.live.as_deref(),
            _ => None,
        }
    }

    pub(super) fn pending(&self) -> usize {
        edits!(self, e => e.pending(), 0)
    }

    fn can_undo(&self) -> bool {
        edits!(self, e => e.can_undo(), false)
    }

    fn undo(&mut self) -> bool {
        edits!(mut self, e => e.undo(), false)
    }

    fn discard(&mut self) {
        edits!(mut self, e => e.discard(), ())
    }

    fn mark_sent(&mut self) {
        edits!(mut self, e => e.mark_sent(), ())
    }

    fn accept_sent(&mut self) {
        edits!(mut self, e => e.accept_sent(), ())
    }

    /// The event for the pending edits; a board adds a `dispatch` change
    /// for each card the edits drop into a `starts` column.
    fn event_command(&self, id: &str, version: u64, note: Option<&str>) -> Option<String> {
        if let Body::Kanban(s) = self {
            let (base, working) = (s.edits.base(), s.edits.working());
            let (mut changes, mut summary) = s.edits.summarize();
            for card in Board::dispatches(base, working) {
                if let Some((change, line)) = kanban::dispatch(working, &card) {
                    changes.push(change);
                    summary.push(line);
                }
            }
            return Some(genui::event_command_json(
                id,
                version,
                &changes,
                &summary,
                &working.to_value(),
                note,
            ));
        }
        edits!(self, e => Some(e.event_command(id, version, note)), None)
    }

    /// The spec as the user sees it (unsent edits included), as JSON.
    fn spec(&self) -> Value {
        match self {
            Body::Table(s) => s.edits.working().to_value(),
            Body::Form(s) => s.edits.working().to_value(),
            Body::Checklist(s) => s.edits.working().to_value(),
            Body::Kanban(s) => s.edits.working().to_value(),
            Body::Chart(s) => s.chart.to_value(),
            Body::Review(s) => s.edits.working().to_value(),
            Body::Sequence(s) => s.edits.working().to_value(),
            Body::Erd(s) => s.edits.working().to_value(),
            Body::State(s) => s.edits.working().to_value(),
            Body::Timeline(s) => s.edits.working().to_value(),
        }
    }

    /// The header's counts.
    fn stats(&self) -> String {
        let language = crate::shared::i18n::language();
        match self {
            Body::Table(s) => {
                let t = s.edits.working();
                let mut text = format!(
                    "{} · {}",
                    super::count(t.rows.len(), "{count} row", "{count} rows"),
                    super::count(t.columns.len(), "{count} column", "{count} columns")
                );
                if !t.selection.is_empty() {
                    text.push_str(" · ");
                    text.push_str(&super::count(
                        t.selection.len(),
                        "{count} selected",
                        "{count} selected",
                    ));
                }
                text
            }
            Body::Form(s) => gentle_core::genui::form::field_count_in(s.edits.working(), language),
            Body::Checklist(s) => {
                let c = s.edits.working();
                crate::shared::i18n::t("{done} of {total} done")
                    .replace("{done}", &c.done().to_string())
                    .replace("{total}", &c.count().to_string())
            }
            Body::Kanban(s) => kanban::card_count_in(s.edits.working(), language),
            Body::Chart(s) => format!(
                "{} · {}",
                crate::shared::i18n::t(s.chart.kind.name()),
                super::count(s.chart.series.len(), "{count} series", "{count} series")
            ),
            Body::Review(s) => s.edits.working().stats_in(language),
            Body::Sequence(s) => sequence::counts_in(s.edits.working(), language),
            Body::Erd(s) => erd::counts_in(s.edits.working(), language),
            Body::State(s) => states::counts_in(s.edits.working(), language),
            Body::Timeline(s) => timeline::counts_in(s.edits.working(), language),
        }
    }

    /// The footer's hint while nothing is pending.
    fn hint(&self) -> &'static str {
        crate::shared::i18n::t(match self {
            Body::Table(s) if s.edits.working().editable => {
                "Click a header to sort · check rows to select · double-click a cell to edit"
            }
            Body::Table(_) => "Click a header to sort · filter · select rows to send them",
            Body::Form(_) => "Fill in and submit; the agent gets every answer at once",
            Body::Checklist(_) => {
                "Click to check · double-click to rename · drag ⋮⋮ to reorder · Tab to indent"
            }
            Body::Kanban(_) => "Drag cards between columns · click one for its details",
            Body::Chart(_) => "Hover for values · click one to ask about it",
            Body::Review(_) => {
                "j/k to move · a accept · r reject · c comment · Shift-A/R the whole file"
            }
            Body::Sequence(_) => {
                "Drag a lifeline to another to add a message · drag messages and participants to reorder · double-click to rename"
            }
            Body::Erd(_) => {
                "Drag tables to move · drag ● from a column to another to relate them · double-click to rename"
            }
            Body::State(_) => {
                "Drag states to move · drag ● to add a transition · double-click to rename"
            }
            Body::Timeline(_) => {
                "Drag a bar to move it, its ends to resize · drag ● to another task to link · double-click to rename"
            }
        })
    }

    /// The body's height (px at 100 %) for the conversation card.
    fn height(&self) -> f32 {
        match self {
            Body::Table(s) => s.height(),
            Body::Form(s) => s.height(),
            Body::Checklist(s) => s.height(),
            Body::Kanban(s) => s.height(),
            Body::Chart(_) => 300.,
            Body::Review(s) => s.height(),
            Body::Sequence(_) | Body::Erd(_) | Body::State(_) | Body::Timeline(_) => {
                super::canvas::CANVAS_HEIGHT
            }
        }
    }
}

pub struct PanelView {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) title: String,
    pub(super) version: u64,
    /// `None` when the first version couldn't be read (see `error`).
    pub(super) body: Option<Body>,
    /// Why the latest spec could not be drawn (the previous one stays).
    pub(super) error: Option<Vec<String>>,
    /// Why the last edit was refused.
    pub(super) refused: Option<crate::shared::i18n::Message>,
    pub(super) note: Option<Entity<InputState>>,
    note_language: gentle_core::i18n::Language,
    pub(super) send: SendState,
    pub(super) expanded: bool,
    pub(super) focus: FocusHandle,
    pub(super) glides: Glides,
    pub(super) env: super::share::Env,
    /// The spec's live source, its consent and latest data.
    pub(super) live: Option<LiveState>,
    /// The conversation's live data and allowed commands.
    pub(super) live_env: Option<SharedEnv>,
    /// The session's subagents, for kanban cards with an `agentId`.
    pub(super) agents: Vec<AgentStatus>,
    /// Shown by the app, not the agent (the todo list): nothing to edit or send.
    pub(super) read_only: bool,
}

impl EventEmitter<DiagramEvent> for PanelView {}

impl PanelView {
    pub fn new(snapshot: &Snapshot, cx: &mut Context<Self>) -> Self {
        let (body, error) = match Body::parse(&snapshot.kind, &snapshot.spec) {
            Ok(body) => (Some(body), None),
            Err(errors) => (None, Some(errors)),
        };
        Self {
            id: snapshot.id.clone(),
            kind: snapshot.kind.clone(),
            title: snapshot.title.clone(),
            version: snapshot.version,
            body,
            error,
            refused: None,
            note: None,
            note_language: crate::shared::i18n::language(),
            send: SendState::Idle,
            expanded: false,
            focus: cx.focus_handle(),
            glides: Glides::default(),
            env: Default::default(),
            live: None,
            live_env: None,
            agents: Vec::new(),
            read_only: false,
        }
    }

    /// The agent's todo list as a read-only checklist (no tool call: the
    /// app shows it from the `todo` data it already has).
    pub fn todo_list(id: &str, cx: &mut Context<Self>) -> Self {
        let snapshot = Snapshot {
            id: id.to_string(),
            version: 0,
            kind: "checklist".into(),
            title: "Todos".into(),
            spec: serde_json::json!({"items": [], "live": "todo"}),
            closed: false,
        };
        Self {
            read_only: true,
            ..Self::new(&snapshot, cx)
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn display_title(&self) -> &str {
        if self.read_only && self.id == super::TODO_SURFACE {
            crate::shared::i18n::t("Todos")
        } else if self.title.is_empty() {
            default_title(&self.kind)
        } else {
            &self.title
        }
    }

    /// What the panel shows, for the Export menu.
    pub(super) fn source(&self) -> super::share::Source {
        super::share::Source {
            id: self.id.clone(),
            kind: self.kind.clone(),
            title: self.title.clone(),
            spec: self.body.as_ref().map_or(Value::Null, Body::spec),
            root: self.env.root.clone(),
        }
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// A newer version from the transcript (see `DiagramView::set_snapshot`).
    pub fn set_snapshot(&mut self, snapshot: &Snapshot, replaced: bool, cx: &mut Context<Self>) {
        let same = snapshot.version == self.version && snapshot.title == self.title;
        if !replaced && (same || snapshot.version < self.version) {
            return;
        }
        if same && self.body.as_ref().is_some_and(|b| b.holds(&snapshot.spec)) {
            return;
        }
        self.version = snapshot.version;
        self.title = snapshot.title.clone();
        let result = match &mut self.body {
            Some(body) => body.rebase(&snapshot.spec),
            None => Body::parse(&self.kind, &snapshot.spec).map(|body| self.body = Some(body)),
        };
        self.error = result.err();
        self.sync_live(cx);
        cx.notify();
    }

    pub fn set_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        if self.expanded != expanded {
            self.expanded = expanded;
            cx.notify();
        }
    }

    /// Whether Gentle Shell ran the `/gentle-ui:event` command. The
    /// extension answers first ([`Self::outcome`]); `Ok` while still
    /// sending means an extension that doesn't answer, which took them.
    pub fn sent(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        if !matches!(self.send, SendState::Sending { .. }) {
            return;
        }
        self.send = match result {
            Ok(()) => {
                self.note = None;
                SendState::Sent
            }
            Err(error) => SendState::Failed(error),
        };
        self.apply_live(cx);
        cx.notify();
    }

    /// The extension's answer to the edits being sent: accepted ones
    /// become the base (edits made since stay pending); rejected ones stay
    /// pending with the reason, to retry or discard.
    pub fn outcome(&mut self, outcome: &Outcome, cx: &mut Context<Self>) {
        let SendState::Sending { version, .. } = self.send else {
            return;
        };
        if version != outcome.version {
            return;
        }
        match &outcome.result {
            Ok(current) => {
                if let Some(body) = &mut self.body {
                    body.accept_sent();
                }
                self.version = *current;
                self.note = None;
                self.send = SendState::Sent;
            }
            Err(error) => self.send = SendState::Failed(error.clone()),
        }
        self.apply_live(cx);
        cx.notify();
    }

    /// The card's height in the conversation (px at 100 %).
    pub fn card_height(&self) -> f32 {
        let body = self.body.as_ref().map_or(160., Body::height);
        (CHROME + body).min(MAX_HEIGHT)
    }

    // ── Edits ──────────────────────────────────────────────────────────────

    /// Apply one edit to the kind's spec through `apply` (which returns the
    /// edit's result); a refused one leaves everything as it was and says why.
    pub(super) fn edit_with(
        &mut self,
        cx: &mut Context<Self>,
        apply: impl FnOnce(&mut Body) -> Option<Result<(), String>>,
    ) -> bool {
        if self.locked() {
            return false;
        }
        let Some(result) = self.body.as_mut().and_then(apply) else {
            return false;
        };
        let applied = match result {
            Ok(()) => {
                self.refused = None;
                if matches!(self.send, SendState::Failed(_) | SendState::Sent) {
                    self.send = SendState::Idle;
                }
                true
            }
            Err(error) => {
                self.refused = Some(crate::shared::i18n::Message::raw(error));
                false
            }
        };
        cx.notify();
        applied
    }

    pub(super) fn undo(&mut self, cx: &mut Context<Self>) {
        if self.body.as_mut().is_some_and(Body::undo) {
            self.refused = None;
            cx.notify();
        }
    }

    pub(super) fn discard(&mut self, cx: &mut Context<Self>) {
        if matches!(self.send, SendState::Sending { .. }) {
            return;
        }
        if let Some(body) = &mut self.body {
            body.discard();
            self.refused = None;
            self.send = SendState::Idle;
            cx.notify();
        }
    }

    /// Nothing to edit: the app's own list (the todo list), or a checklist
    /// whose items come from live data.
    pub(super) fn locked(&self) -> bool {
        self.read_only
            || matches!(&self.body, Some(Body::Checklist(s)) if s.edits.base().live.is_some())
    }

    fn pending(&self) -> usize {
        self.body.as_ref().map_or(0, Body::pending)
    }

    /// Send the unsent edits (and the note, if any) to the agent, or again
    /// after a failure.
    pub(super) fn send(&mut self, cx: &mut Context<Self>) {
        if self.locked() || self.pending() == 0 || matches!(self.send, SendState::Sending { .. }) {
            return;
        }
        let note = self.note.as_ref().map(|n| n.read(cx).value().to_string());
        let Some(body) = &mut self.body else {
            return;
        };
        let Some(command) = body.event_command(&self.id, self.version, note.as_deref()) else {
            return;
        };
        body.mark_sent();
        self.send = SendState::Sending {
            version: self.version,
            spec: (),
        };
        cx.emit(DiagramEvent::Send(command));
        cx.notify();
    }

    /// "Ask about this": put a reference to `subject` in the composer.
    pub(super) fn ask(&mut self, subject: &str, cx: &mut Context<Self>) {
        let noun = match self.kind.as_str() {
            "kanban" => "board",
            "sequence" => "sequence diagram",
            "erd" => "data model",
            "state" => "state machine",
            other => other,
        };
        cx.emit(DiagramEvent::Ask(format!(
            "About {subject} in the “{}” {noun} ({}): ",
            self.title, self.id
        )));
    }

    fn note_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.note.is_none() {
            self.note_language = crate::shared::i18n::language();
            self.note = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(crate::shared::i18n::t(super::NOTE_PLACEHOLDER))
            }));
        }
    }

    /// Whether a text field inside the card has the keyboard.
    pub(super) fn typing(&self, window: &Window, cx: &App) -> bool {
        !self.focus.is_focused(window) && self.focus.contains_focused(window, cx)
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let handled = match &self.body {
            Some(Body::Checklist(_)) => self.checklist_key(keystroke, window, cx),
            Some(Body::Kanban(_)) => self.kanban_key(keystroke, window, cx),
            Some(Body::Table(_)) => self.table_key(keystroke, window, cx),
            Some(Body::Chart(_)) => self.chart_key(keystroke, cx),
            Some(Body::Review(_)) => self.review_key(keystroke, window, cx),
            Some(Body::Sequence(_)) => self.sequence_key(keystroke, window, cx),
            Some(Body::Erd(_)) => self.erd_key(keystroke, window, cx),
            Some(Body::State(_)) => self.state_key(keystroke, window, cx),
            Some(Body::Timeline(_)) => self.timeline_key(keystroke, window, cx),
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            return;
        }
        if self.typing(window, cx) {
            return;
        }
        match keystroke.key.as_str() {
            "z" if keystroke.modifiers.secondary(cx) && !keystroke.modifiers.shift => self.undo(cx),
            "escape" if self.expanded => cx.emit(DiagramEvent::Collapse),
            _ => return,
        }
        cx.stop_propagation();
    }

    // ── Chrome ─────────────────────────────────────────────────────────────

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = |id: &'static str, icon: IconName, tip: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .text_color(ui::muted())
                .icon(Icon::new(icon))
                .tooltip(crate::shared::i18n::t(tip))
        };
        let locked = self.locked();
        let editable = !locked && !matches!(self.body, Some(Body::Chart(_)) | None);
        let add = match &self.body {
            Some(Body::Checklist(_)) if !locked => Some("Add item"),
            Some(Body::Kanban(_)) => Some("Add card"),
            Some(Body::Sequence(_)) => Some("Add participant"),
            Some(Body::Erd(_)) => Some("Add table"),
            Some(Body::State(_)) => Some("Add state"),
            Some(Body::Timeline(_)) => Some("Add task"),
            _ => None,
        };
        // A checklist the agent wrote can become a plan the user runs.
        let run_mode = match &self.body {
            Some(Body::Checklist(s)) if !locked => Some(s.edits.working().run.is_some()),
            _ => None,
        };
        h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .min_w_0()
            .child(Icon::new(icon(&self.kind)).small().text_color(ui::info()))
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
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(12.))
                    .text_color(ui::muted())
                    .children(self.body.as_ref().map(Body::stats)),
            )
            .children(self.render_live_chip(cx))
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_0p5()
                    .when_some(run_mode, |d, on| {
                        d.child(
                            div().debug_selector(|| "genui-run-plan".into()).child(
                                tool(
                                    "genui-run-plan",
                                    IconName::ListChecks,
                                    if on {
                                        "Turn off Run plan"
                                    } else {
                                        "Run plan: drive the agent through these steps"
                                    },
                                )
                                .when(on, |b| b.text_color(ui::accent()))
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_run_mode(cx))),
                            ),
                        )
                    })
                    .when_some(add, |d, tip| {
                        d.child(
                            div().debug_selector(|| "genui-panel-add".into()).child(
                                tool("genui-panel-add", IconName::Plus, tip).on_click(
                                    cx.listener(|this, _, window, cx| this.add(window, cx)),
                                ),
                            ),
                        )
                    })
                    .when(matches!(self.body, Some(Body::Review(_))), |d| {
                        d.child(
                            div()
                                .debug_selector(|| "genui-review-accept-all".into())
                                .child(
                                    tool(
                                        "genui-review-accept-all",
                                        IconName::CheckCheck,
                                        "Accept all",
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.decide_all(None, Decision::Accepted, cx)
                                        },
                                    )),
                                ),
                        )
                        .child(
                            tool("genui-review-reject-all", IconName::X, "Reject all").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.decide_all(None, Decision::Rejected, cx)
                                }),
                            ),
                        )
                    })
                    .when(editable, |d| {
                        d.child(
                            tool("genui-panel-undo", IconName::Undo2, "Undo")
                                .disabled(!self.body.as_ref().is_some_and(Body::can_undo))
                                .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
                        )
                    })
                    // The app's own todo list isn't the agent's: nothing to
                    // pin, template or hand back.
                    .when(!self.read_only, |d| {
                        d.child(super::share::tools(
                            &self.env,
                            &self.kind,
                            PanelView::source,
                            cx,
                        ))
                    })
                    .child(
                        tool(
                            "genui-panel-expand",
                            if self.expanded {
                                IconName::Minimize2
                            } else {
                                IconName::Maximize2
                            },
                            if self.expanded {
                                "Back to the conversation"
                            } else {
                                "Expand"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.emit(if this.expanded {
                                DiagramEvent::Collapse
                            } else {
                                DiagramEvent::Expand
                            })
                        })),
                    ),
            )
    }

    /// The header's + : a new checklist item or kanban card.
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Some(Body::Checklist(_)) => self.start_add_item(window, cx),
            Some(Body::Kanban(_)) => self.start_add_card(None, window, cx),
            Some(Body::Sequence(_)) => self.add_participant(window, cx),
            Some(Body::Erd(_)) => self.add_table(window, cx),
            Some(Body::State(_)) => self.add_state(window, cx),
            Some(Body::Timeline(_)) => self.add_task(window, cx),
            _ => {}
        }
    }

    fn render_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match &self.body {
            Some(Body::Table(_)) => self.render_table(window, cx),
            Some(Body::Form(_)) => self.render_form(window, cx),
            Some(Body::Checklist(_)) => self.render_checklist(window, cx),
            Some(Body::Kanban(_)) => self.render_kanban(window, cx),
            Some(Body::Chart(_)) => self.render_chart(window, cx),
            Some(Body::Review(_)) => self.render_review(window, cx),
            Some(Body::Sequence(_)) => self.render_sequence(window, cx),
            Some(Body::Erd(_)) => self.render_erd(window, cx),
            Some(Body::State(_)) => self.render_states(window, cx),
            Some(Body::Timeline(_)) => self.render_timeline(window, cx),
            None => div().into_any_element(),
        }
    }

    fn render_error(&self) -> Option<AnyElement> {
        let errors = self.error.as_ref()?;
        let error = super::diagnostic::render_all(errors, "\n");
        let title = if self.body.is_some() {
            crate::shared::i18n::t("The agent's latest version can't be drawn")
        } else {
            crate::shared::i18n::t("This can't be drawn")
        };
        Some(
            div()
                .absolute()
                .left_2()
                .right_2()
                .top_2()
                .child(
                    ui::card()
                        .px_3()
                        .py_2()
                        .gap_1()
                        .bg(ui::popover_bg())
                        .border_color(ui::danger().opacity(0.35))
                        .child(
                            h_flex()
                                .gap_2()
                                .text_size(rems(12. / 16.))
                                .text_color(ui::danger())
                                .child(Icon::new(IconName::CircleAlert).xsmall())
                                .child(title),
                        )
                        .child(
                            div()
                                .font_family(ui::mono())
                                .text_size(ui::code_rems(11.))
                                .text_color(ui::muted())
                                .whitespace_normal()
                                .child(error),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let footer = h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .min_h(rems(38. / 16.))
            .border_t_1()
            .border_color(ui::border())
            .text_size(rems(12. / 16.));
        if let Some(Body::Form(form)) = &self.body {
            return self.form_footer(form, footer, cx).into_any_element();
        }
        if let Some(refused) = &self.refused {
            return footer
                .text_color(ui::danger())
                .child(Icon::new(IconName::CircleAlert).xsmall())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(super::diagnostic::render(&refused.render())),
                )
                .into_any_element();
        }
        let pending = self.pending();
        if pending > 0 {
            let (count, send) = match &self.body {
                Some(Body::Review(s)) => (
                    s.edits
                        .working()
                        .progress_in(crate::shared::i18n::language()),
                    "Send decisions",
                ),
                _ => (
                    super::count(pending, "{count} change", "{count} changes"),
                    "Send to agent",
                ),
            };
            return self
                .pending_footer(footer, count, send, cx)
                .into_any_element();
        }
        match &self.send {
            SendState::Sent => footer
                .text_color(ui::success())
                .child(Icon::new(IconName::CircleCheck).xsmall())
                .child(crate::shared::i18n::t("Sent to the agent"))
                .into_any_element(),
            _ => footer
                .text_color(ui::faint())
                .child(div().flex_1().min_w_0().truncate().child(self.hint()))
                .into_any_element(),
        }
    }

    /// The pending edits: a count, the note, the send error, Discard and
    /// the primary action (`send_label`, Retry after a failure).
    pub(super) fn pending_footer(
        &self,
        footer: Div,
        count: String,
        send_label: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let sending = matches!(self.send, SendState::Sending { .. });
        let retry = matches!(self.send, SendState::Failed(_));
        footer
            .child(ui::chip(count, ui::accent()))
            .child(match &self.note {
                Some(note) => ui::field()
                    .flex_1()
                    .min_w_0()
                    .h(rems(26. / 16.))
                    .child(Input::new(note).appearance(false).xsmall())
                    .into_any_element(),
                None => div()
                    .id("genui-panel-note")
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .cursor_pointer()
                    .text_color(ui::faint())
                    .hover(|d| d.text_color(ui::muted()))
                    .child(crate::shared::i18n::t("Add a note…"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.note_input(window, cx);
                        if let Some(note) = &this.note {
                            note.update(cx, |state, cx| state.focus(window, cx));
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            })
            .when_some(
                match &self.send {
                    SendState::Failed(error) => Some(error.clone()),
                    _ => None,
                },
                |d, error| {
                    d.child(
                        div()
                            .debug_selector(|| "genui-panel-send-error".into())
                            .max_w(rems(220. / 16.))
                            .truncate()
                            .text_color(ui::danger())
                            .child(error),
                    )
                },
            )
            .child(
                Button::new("genui-panel-discard")
                    .ghost()
                    .xsmall()
                    .label(crate::shared::i18n::t("Discard"))
                    .disabled(sending)
                    .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
            )
            .child(
                div().debug_selector(|| "genui-panel-send".into()).child(
                    Button::new("genui-panel-send")
                        .primary()
                        .xsmall()
                        .icon(Icon::new(if retry {
                            IconName::RefreshCw
                        } else {
                            IconName::Send
                        }))
                        .label(crate::shared::i18n::t(if retry {
                            "Retry"
                        } else {
                            send_label
                        }))
                        .loading(sending)
                        .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
                ),
            )
    }

    /// The footer's hint while nothing is pending.
    fn hint(&self) -> &'static str {
        match &self.body {
            Some(Body::Checklist(_)) if self.read_only => crate::shared::i18n::t(
                "The agent's todo list, live · read-only · Ask about an item from its row",
            ),
            Some(Body::Checklist(_)) if self.locked() => {
                crate::shared::i18n::t("Live items · read-only")
            }
            Some(Body::Checklist(s)) if s.edits.working().run.is_some() => crate::shared::i18n::t(
                "Skip or reorder steps, then Run (all) or Run next · the agent reports each step",
            ),
            Some(Body::Kanban(s)) if s.edits.working().columns.iter().any(|c| c.starts) => {
                crate::shared::i18n::t(
                    "Drag a card into a column marked with a bot: the agent starts a subagent on it",
                )
            }
            Some(body) => body.hint(),
            None => "",
        }
    }
}

impl Render for PanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(note) = &self.note {
            crate::shared::i18n::sync_placeholder(
                note,
                super::NOTE_PLACEHOLDER,
                &mut self.note_language,
                window,
                cx,
            );
        }
        self.glides.tick(window, cx);
        self.live_drawn(window, cx);
        let body = self.render_body(window, cx);
        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .size_full()
            .rounded_xl()
            .bg(ui::surface())
            .border_1()
            .border_color(ui::border())
            .overflow_hidden()
            .child(self.render_header(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(body)
                    .children(self.render_error())
                    .children(self.render_consent(cx)),
            )
            .child(self.render_footer(cx))
    }
}
