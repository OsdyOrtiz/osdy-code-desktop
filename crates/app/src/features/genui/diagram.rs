//! One diagram surface: the agent's latest spec, the user's local edits on
//! top of it, and sending them. Edits go through
//! `gentle_core::genui::changes::apply`, so what the user does is exactly
//! what the change list sent to the agent describes. Pointer, keyboard and
//! viewport handling live in `pointer.rs`, animation in `motion.rs`.

use std::cell::Cell;
use std::rc::Rc;

use gentle_core::genui::changes::{self, Change, NodePatch};
use gentle_core::genui::diagram::{Diagram, Direction, Edge, EdgeStyle, Group, Node, NodeKind};
use gentle_core::genui::layout::{self, Layout};
use gentle_core::genui::{self, Outcome, Snapshot};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;

use super::motion::{Arrival, ViewTween};

/// Zoom limits, as a factor of the interface size.
pub(super) const MIN_ZOOM: f32 = 0.2;
pub(super) const MAX_ZOOM: f32 = 2.5;
/// The automatic fit doesn't go below this: labels stay readable.
pub(super) const READABLE_ZOOM: f32 = 0.6;
/// Dragged and added nodes snap to this grid (diagram units); the
/// background's dots mark it.
pub(super) const GRID: f32 = 20.;
/// Undo keeps this many steps.
const UNDO_DEPTH: usize = 100;

/// What a surface asks of the conversation that shows it.
pub enum DiagramEvent {
    /// Run this `/gentle-ui:event …` command line; answer with
    /// [`DiagramView::sent`] (and the extension with [`DiagramView::outcome`]).
    Send(String),
    /// Put this reference in the composer.
    Ask(String),
    Expand,
    Collapse,
    /// Open this http(s) URL in the in-app browser (the user confirmed).
    OpenWeb(String),
    /// Pin the surface to the conversation's side panel (`false`: unpin).
    Pin(bool),
    /// Run this `/gentle-ui:event …` request (not an edit: no `sent` or
    /// outcome for the view), e.g. "add this to the docs".
    Request(String),
    /// Open this code reference in the file viewer.
    OpenRef(gentle_core::genui::refs::CodeRef),
    /// Tell the user how an export went (`Err`: it failed).
    Notice(Result<String, String>),
    /// Stop this subagent (`cancel_subagent`): a kanban card's Stop.
    CancelSubagent(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Selection {
    Node(String),
    /// Two or more nodes (box selection, shift-click).
    Nodes(Vec<String>),
    Edge(String),
    Group(String),
}

impl Selection {
    /// One node, several, or nothing.
    pub fn of_nodes(mut ids: Vec<String>) -> Option<Self> {
        match ids.len() {
            0 => None,
            1 => ids.pop().map(Selection::Node),
            _ => Some(Selection::Nodes(ids)),
        }
    }

    /// The selected nodes (none when an edge or a group is selected).
    pub fn nodes(&self) -> &[String] {
        match self {
            Selection::Node(id) => std::slice::from_ref(id),
            Selection::Nodes(ids) => ids,
            Selection::Edge(_) | Selection::Group(_) => &[],
        }
    }
}

pub(super) enum Drag {
    /// Moving the view: where the press started and the pan then.
    Pan {
        from: Point<Pixels>,
        pan: Point<f32>,
        moved: bool,
    },
    /// Moving the selected nodes: where each one was and the press point
    /// (diagram units).
    Nodes {
        starts: Vec<(String, layout::Point)>,
        /// The layout when the drag started: its group boxes decide
        /// whether a dropped node changes group.
        before: Rc<Layout>,
        grab: layout::Point,
        from: Point<Pixels>,
        moved: bool,
    },
    /// Drawing a connection from a node's handle to the pointer:
    /// `outgoing` from its source side, else into its target side.
    Connect {
        node: String,
        outgoing: bool,
        to: layout::Point,
    },
    /// Shift-dragging a selection box (diagram units).
    Select {
        from: layout::Point,
        to: layout::Point,
    },
    /// Navigating with the minimap.
    MiniMap,
}

/// Where a surface's last send stands; every kind uses it. `S` is what
/// went out (the diagram keeps its sent copy here; panels keep theirs in
/// their `Edits`, the html page has none).
pub(super) enum SendState<S = ()> {
    Idle,
    /// Waiting for the extension's answer about what was sent from `version`.
    Sending {
        version: u64,
        spec: S,
    },
    /// The extension took it; says so until the next edit.
    Sent,
    /// Not delivered, or rejected by the extension: the edits are kept.
    Failed(String),
}

pub(super) struct Rename {
    pub target: Selection,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

/// The details panel's text fields for the selected node or group.
pub(super) const TECH_PLACEHOLDER: &str = "Tech (e.g. Postgres)";

pub(super) struct Fields {
    pub language: gentle_core::i18n::Language,
    pub target: Selection,
    /// Nodes only.
    pub tech: Option<Entity<InputState>>,
    pub description: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

/// Pan (px, from the canvas' top-left) and zoom (× interface size).
pub(super) struct Viewport {
    pub pan: Point<f32>,
    pub zoom: f32,
    /// The canvas size it was last fitted to (`None`: fit at next frame).
    pub fitted_for: Option<Size<Pixels>>,
    /// The user panned or zoomed: a resize no longer refits.
    pub touched: bool,
}

pub struct DiagramView {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) version: u64,
    /// The agent's latest spec (tool result, or the user's edits the
    /// extension accepted).
    pub(super) base: Diagram,
    /// `base` plus the edits not sent yet.
    pub(super) working: Diagram,
    pub(super) layout: Rc<Layout>,
    /// Changes between `base` and `working`.
    pub(super) pending: usize,
    undo: Vec<Diagram>,
    redo: Vec<Diagram>,
    /// Why the latest spec could not be drawn (the previous one stays).
    pub(super) error: Option<String>,
    /// Why the last edit was refused.
    pub(super) refused: Option<String>,
    pub(super) viewport: Viewport,
    /// A fit or zoom button easing the viewport to a target.
    pub(super) view_tween: Option<ViewTween>,
    /// The agent's new version easing in over the previous one.
    pub(super) arrival: Option<Arrival>,
    pub(super) selection: Option<Selection>,
    pub(super) drag: Option<Drag>,
    /// The edge under the pointer (index into `working.edges`).
    pub(super) hover_edge: Option<usize>,
    pub(super) rename: Option<Rename>,
    pub(super) fields: Option<Fields>,
    pub(super) note: Option<Entity<InputState>>,
    pub(super) note_language: gentle_core::i18n::Language,
    pub(super) send: SendState<Diagram>,
    /// The canvas' and the minimap's bounds last frame, written while painting.
    pub(super) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) minimap: Rc<Cell<Bounds<Pixels>>>,
    /// Shown in place of the conversation instead of in its card.
    pub(super) expanded: bool,
    /// Viewing only: no moving, connecting or editing.
    pub(super) locked: bool,
    pub(super) focus: FocusHandle,
    pub(super) env: super::share::Env,
}

impl EventEmitter<DiagramEvent> for DiagramView {}

impl DiagramView {
    pub fn new(snapshot: &Snapshot, cx: &mut Context<Self>) -> Self {
        let (base, error) = match Diagram::parse(&snapshot.spec) {
            Ok(diagram) => (diagram, None),
            Err(errors) => (Diagram::default(), Some(errors.join("\n"))),
        };
        Self {
            id: snapshot.id.clone(),
            title: snapshot.title.clone(),
            version: snapshot.version,
            layout: Rc::new(layout::layout(&base)),
            working: base.clone(),
            base,
            pending: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            error,
            refused: None,
            viewport: Viewport {
                pan: Point::default(),
                zoom: 1.,
                fitted_for: None,
                touched: false,
            },
            view_tween: None,
            arrival: None,
            selection: None,
            drag: None,
            hover_edge: None,
            rename: None,
            fields: None,
            note: None,
            note_language: crate::shared::i18n::language(),
            send: SendState::Idle,
            bounds: Rc::new(Cell::new(Bounds::default())),
            minimap: Rc::new(Cell::new(Bounds::default())),
            expanded: false,
            locked: false,
            focus: cx.focus_handle(),
            env: Default::default(),
        }
    }

    /// A newer version from the transcript. Unsent edits are replayed on
    /// top of it (those that still apply) and the change eases in; the
    /// viewport stays. `replaced`: the conversation was reloaded or
    /// replaced, so the same id may now carry another (or an older)
    /// version; an identical one changes nothing.
    pub fn set_snapshot(&mut self, snapshot: &Snapshot, replaced: bool, cx: &mut Context<Self>) {
        let same = snapshot.version == self.version && snapshot.title == self.title;
        if !replaced && (same || snapshot.version < self.version) {
            return;
        }
        let parsed = Diagram::parse(&snapshot.spec);
        if same && parsed.as_ref().is_ok_and(|diagram| *diagram == self.base) {
            return;
        }
        self.version = snapshot.version;
        self.title = snapshot.title.clone();
        let before = (self.working.clone(), self.layout.clone());
        match parsed {
            Ok(diagram) => {
                self.error = None;
                // The extension's echo of edits it took without answering
                // (an extension from before outcomes): they were sent.
                if let SendState::Sending { spec, .. } = &self.send
                    && *spec == diagram
                {
                    self.send = SendState::Sent;
                    self.note = None;
                }
                let pending = changes::diff(&self.base, &self.working);
                let mut working = diagram.clone();
                for change in &pending {
                    // An edit the new version makes impossible is dropped.
                    let _ = changes::apply(&mut working, change);
                }
                self.base = diagram;
                self.working = working;
                self.undo.clear();
                self.redo.clear();
            }
            Err(errors) => self.error = Some(errors.join("\n")),
        }
        // A view the user hasn't moved follows the new content.
        if !self.viewport.touched {
            self.viewport.fitted_for = None;
        }
        self.forget_missing();
        self.relayout();
        if !cx.reduce_motion() && before.0 != self.working {
            self.arrival = Arrival::between(&before.0, &before.1, &self.working, &self.layout);
        }
        cx.notify();
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The diagram as the user sees it, for the Export menu.
    pub(super) fn source(&self) -> super::share::Source {
        super::share::Source {
            id: self.id.clone(),
            kind: "diagram".into(),
            title: self.title.clone(),
            spec: self.working.to_value(),
            root: self.env.root.clone(),
        }
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
        cx.notify();
    }

    /// The extension's answer to the edits being sent: accepted ones
    /// become the base (the edits made since stay pending); rejected ones
    /// stay pending with the reason, to retry or discard.
    pub fn outcome(&mut self, outcome: &Outcome, cx: &mut Context<Self>) {
        let SendState::Sending { version, spec } = &self.send else {
            return;
        };
        if *version != outcome.version {
            return;
        }
        match &outcome.result {
            Ok(current) => {
                self.base = spec.clone();
                self.version = *current;
                self.note = None;
                self.send = SendState::Sent;
                self.relayout();
            }
            Err(error) => self.send = SendState::Failed(error.clone()),
        }
        cx.notify();
    }

    // ── Edits ──────────────────────────────────────────────────────────────

    pub(super) fn relayout(&mut self) {
        self.layout = Rc::new(layout::layout(&self.working));
        self.pending = changes::diff(&self.base, &self.working).len();
        if self
            .hover_edge
            .is_some_and(|ix| ix >= self.working.edges.len())
        {
            self.hover_edge = None;
        }
    }

    /// Drop a selection, rename or details fields whose target no longer exists.
    fn forget_missing(&mut self) {
        let working = &self.working;
        let exists = |selection: &Selection| match selection {
            Selection::Node(id) => working.node(id).is_some(),
            Selection::Nodes(ids) => ids.iter().all(|id| working.node(id).is_some()),
            Selection::Edge(id) => working.edge(id).is_some(),
            Selection::Group(id) => working.group(id).is_some(),
        };
        if let Some(Selection::Nodes(ids)) = &self.selection
            && !exists(&Selection::Nodes(ids.clone()))
        {
            let kept = ids
                .iter()
                .filter(|id| working.node(id).is_some())
                .cloned()
                .collect();
            self.selection = Selection::of_nodes(kept);
        }
        if self.selection.as_ref().is_some_and(|s| !exists(s)) {
            self.selection = None;
        }
        if self.rename.as_ref().is_some_and(|r| !exists(&r.target)) {
            self.rename = None;
        }
        if self.fields.as_ref().is_some_and(|f| !exists(&f.target)) {
            self.fields = None;
        }
    }

    pub(super) fn remember(&mut self, before: Diagram) {
        self.undo.push(before);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// The working copy changed: a new edit clears a refusal and the last
    /// send's result.
    pub(super) fn edited(&mut self) {
        self.refused = None;
        if matches!(self.send, SendState::Failed(_) | SendState::Sent) {
            self.send = SendState::Idle;
        }
    }

    /// Apply one user edit; a refused one leaves everything as it was.
    pub(super) fn edit(&mut self, change: Change, cx: &mut Context<Self>) -> bool {
        self.edit_all(&[change], cx)
    }

    /// Apply edits as one undo step, all or none.
    pub(super) fn edit_all(&mut self, edits: &[Change], cx: &mut Context<Self>) -> bool {
        let mut next = self.working.clone();
        let result = edits
            .iter()
            .try_for_each(|change| changes::apply(&mut next, change));
        cx.notify();
        if let Err(error) = result {
            self.refused = Some(error);
            return false;
        }
        let before = std::mem::replace(&mut self.working, next);
        self.remember(before);
        self.edited();
        self.forget_missing();
        self.relayout();
        true
    }

    pub(super) fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(previous) = self.undo.pop() {
            let current = std::mem::replace(&mut self.working, previous);
            self.redo.push(current);
            self.after_history(cx);
        }
    }

    pub(super) fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(next) = self.redo.pop() {
            let current = std::mem::replace(&mut self.working, next);
            self.undo.push(current);
            self.after_history(cx);
        }
    }

    fn after_history(&mut self, cx: &mut Context<Self>) {
        self.rename = None;
        self.edited();
        self.forget_missing();
        self.relayout();
        cx.notify();
    }

    pub(super) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(super) fn discard(&mut self, cx: &mut Context<Self>) {
        if self.pending == 0 || matches!(self.send, SendState::Sending { .. }) {
            return;
        }
        let before = std::mem::replace(&mut self.working, self.base.clone());
        self.remember(before);
        self.rename = None;
        self.refused = None;
        self.send = SendState::Idle;
        self.forget_missing();
        self.relayout();
        cx.notify();
    }

    pub(super) fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if self.locked {
            return;
        }
        let edits: Vec<Change> = match self.selection.clone() {
            Some(Selection::Node(node)) => vec![Change::RemoveNode { node }],
            Some(Selection::Nodes(nodes)) => nodes
                .into_iter()
                .map(|node| Change::RemoveNode { node })
                .collect(),
            Some(Selection::Edge(edge)) => vec![Change::RemoveEdge { edge }],
            Some(Selection::Group(group)) => vec![Change::RemoveGroup { group }],
            None => return,
        };
        if self.edit_all(&edits, cx) {
            self.selection = None;
        }
    }

    /// A new node in the middle of the view (in the selected group, if
    /// one is), selected and being named.
    pub(super) fn add_node(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked {
            return;
        }
        let bounds = self.bounds.get();
        let center = self.to_world(
            point(bounds.size.width / 2., bounds.size.height / 2.),
            window.rem_size(),
        );
        let id = self.working.fresh_node_id("new-node");
        let mut node = Node::new(id.clone(), "New node", NodeKind::Service);
        node.x = Some(snap(center.x - layout::NODE_W / 2.));
        node.y = Some(snap(center.y - layout::NODE_H / 2.));
        if let Some(Selection::Group(group)) = &self.selection {
            node.group = Some(group.clone());
        }
        if self.edit(Change::AddNode { node }, cx) {
            self.selection = Some(Selection::Node(id.clone()));
            self.start_rename(Selection::Node(id), window, cx);
        }
    }

    /// A new group holding the selected nodes, selected and being named.
    pub(super) fn group_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let nodes = self
            .selection
            .as_ref()
            .map(|s| s.nodes().to_vec())
            .unwrap_or_default();
        if self.locked || nodes.is_empty() {
            return;
        }
        let id = self.working.fresh_group_id("group");
        let mut edits = vec![Change::AddGroup {
            group: Group {
                id: id.clone(),
                label: "New group".into(),
                description: None,
                color: None,
            },
        }];
        edits.extend(nodes.into_iter().map(|node| Change::UpdateNode {
            node,
            patch: NodePatch {
                group: Some(Some(id.clone())),
                ..NodePatch::default()
            },
        }));
        if self.edit_all(&edits, cx) {
            self.selection = Some(Selection::Group(id.clone()));
            self.start_rename(Selection::Group(id), window, cx);
        }
    }

    pub(super) fn toggle_direction(&mut self, cx: &mut Context<Self>) {
        if self.locked {
            return;
        }
        let direction = match self.working.direction {
            Direction::LeftRight => Direction::TopBottom,
            Direction::TopBottom => Direction::LeftRight,
        };
        if self.edit(Change::SetDirection { direction }, cx) && !self.viewport.touched {
            self.viewport.fitted_for = None;
        }
    }

    /// One field of a node, from the details panel.
    pub(super) fn patch_node(&mut self, node: &str, patch: NodePatch, cx: &mut Context<Self>) {
        if self.locked {
            return;
        }
        // An unchanged value is not an edit.
        let Some(current) = self.working.node(node) else {
            return;
        };
        let changed = patch.label.as_ref().is_some_and(|l| *l != current.label)
            || patch.kind.is_some_and(|k| k != current.kind)
            || patch.group.as_ref().is_some_and(|g| *g != current.group)
            || patch
                .description
                .as_ref()
                .is_some_and(|d| *d != current.description)
            || patch.tech.as_ref().is_some_and(|t| *t != current.tech)
            || patch.status.is_some_and(|s| s != current.status);
        if changed {
            self.edit(
                Change::UpdateNode {
                    node: node.to_string(),
                    patch,
                },
                cx,
            );
        }
    }

    pub(super) fn set_edge_style(&mut self, edge: &str, style: EdgeStyle, cx: &mut Context<Self>) {
        if self.locked || self.working.edge(edge).is_none_or(|e| e.style == style) {
            return;
        }
        self.edit(
            Change::UpdateEdge {
                edge: edge.to_string(),
                label: None,
                style: Some(style),
            },
            cx,
        );
    }

    /// The selected node into a new group named after it.
    pub(super) fn new_group_for(
        &mut self,
        node: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = Some(Selection::Node(node.to_string()));
        self.group_selection(window, cx);
    }

    /// `node` → `other` when `outgoing`, else `other` → `node`.
    pub(super) fn connect(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        if from == to || self.locked {
            return;
        }
        let id = self.working.fresh_edge_id(from, to);
        let edge = Edge {
            id: id.clone(),
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            style: EdgeStyle::default(),
            color: None,
        };
        if self.edit(Change::AddEdge { edge }, cx) {
            self.selection = Some(Selection::Edge(id));
        }
    }

    pub(super) fn start_rename(
        &mut self,
        target: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.locked {
            return;
        }
        let current = match &target {
            Selection::Node(id) => self.working.node(id).map(|n| n.label.clone()),
            Selection::Edge(id) => self
                .working
                .edge(id)
                .map(|e| e.label.clone().unwrap_or_default()),
            Selection::Group(id) => self.working.group(id).map(|g| g.label.clone()),
            Selection::Nodes(_) => None,
        };
        let Some(current) = current else { return };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.finish_rename(true, cx);
                    window.focus(&this.focus, cx);
                }
                InputEvent::Blur => this.finish_rename(true, cx),
                _ => {}
            },
        );
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        self.rename = Some(Rename {
            target,
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(super) fn finish_rename(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(rename) = self.rename.take() else {
            return;
        };
        cx.notify();
        if !keep {
            return;
        }
        let text = rename.input.read(cx).value().trim().to_string();
        let change = match rename.target {
            Selection::Node(node) => {
                let unchanged = self.working.node(&node).is_none_or(|n| n.label == text);
                if text.is_empty() || unchanged {
                    return;
                }
                Change::UpdateNode {
                    node,
                    patch: NodePatch {
                        label: Some(text),
                        ..NodePatch::default()
                    },
                }
            }
            Selection::Edge(edge) => {
                let label = (!text.is_empty()).then_some(text);
                if self.working.edge(&edge).is_none_or(|e| e.label == label) {
                    return;
                }
                Change::UpdateEdge {
                    edge,
                    label: Some(label),
                    style: None,
                }
            }
            Selection::Group(group) => {
                let unchanged = self.working.group(&group).is_none_or(|g| g.label == text);
                if text.is_empty() || unchanged {
                    return;
                }
                Change::UpdateGroup {
                    group,
                    label: Some(text),
                    description: None,
                }
            }
            Selection::Nodes(_) => return,
        };
        self.edit(change, cx);
    }

    /// The details panel's text fields follow the selected node or group.
    pub(super) fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = match &self.selection {
            Some(s @ (Selection::Node(_) | Selection::Group(_))) if !self.locked => s.clone(),
            _ => {
                self.fields = None;
                return;
            }
        };
        if self.fields.as_ref().is_some_and(|f| f.target == target) {
            return;
        }
        let (tech, description) = match &target {
            Selection::Node(id) => {
                let Some(node) = self.working.node(id) else {
                    return;
                };
                (
                    Some(node.tech.clone().unwrap_or_default()),
                    node.description.clone(),
                )
            }
            Selection::Group(id) => {
                let Some(group) = self.working.group(id) else {
                    return;
                };
                (None, group.description.clone())
            }
            _ => return,
        };
        let field = |value: String,
                     placeholder: &'static str,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(crate::shared::i18n::t(placeholder))
                    .default_value(value)
            })
        };
        let tech = tech.map(|t| field(t, TECH_PLACEHOLDER, window, cx));
        let description = field(description.unwrap_or_default(), "Description", window, cx);
        let mut subscriptions = Vec::new();
        for input in tech.iter().chain([&description]) {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => {
                        this.commit_fields(cx);
                        window.focus(&this.focus, cx);
                    }
                    InputEvent::Blur => this.commit_fields(cx),
                    _ => {}
                },
            ));
        }
        self.fields = Some(Fields {
            language: crate::shared::i18n::language(),
            target,
            tech,
            description,
            _subscriptions: subscriptions,
        });
    }

    /// The details fields' values as edits (nothing when unchanged).
    pub(super) fn commit_fields(&mut self, cx: &mut Context<Self>) {
        let Some(fields) = &self.fields else { return };
        let text = |input: &Entity<InputState>| {
            let value = input.read(cx).value().trim().to_string();
            (!value.is_empty()).then_some(value)
        };
        let description = text(&fields.description);
        match fields.target.clone() {
            Selection::Node(node) => {
                let tech = fields.tech.as_ref().and_then(text);
                self.patch_node(
                    &node,
                    NodePatch {
                        tech: Some(tech),
                        description: Some(description),
                        ..NodePatch::default()
                    },
                    cx,
                );
            }
            Selection::Group(group)
                if self
                    .working
                    .group(&group)
                    .is_some_and(|g| g.description != description) =>
            {
                self.edit(
                    Change::UpdateGroup {
                        group,
                        label: None,
                        description: Some(description),
                    },
                    cx,
                );
            }
            _ => {}
        }
    }

    /// Send the unsent edits (and the note, if any) to the agent, or again
    /// after a failure.
    pub(super) fn send(&mut self, cx: &mut Context<Self>) {
        if self.pending == 0 || matches!(self.send, SendState::Sending { .. }) {
            return;
        }
        let (changes, summary) = changes::summarize(&self.base, &self.working);
        let note = self.note.as_ref().map(|n| n.read(cx).value().to_string());
        let command = genui::event_command(
            &self.id,
            self.version,
            &changes,
            &summary,
            &self.working,
            note.as_deref(),
        );
        self.send = SendState::Sending {
            version: self.version,
            spec: self.working.clone(),
        };
        cx.emit(DiagramEvent::Send(command));
        cx.notify();
    }

    /// "Ask about this": a reference to the selection for the composer.
    pub(super) fn ask(&mut self, cx: &mut Context<Self>) {
        let name = |node: &str| {
            self.working
                .node(node)
                .map_or(node.to_string(), |n| n.label.clone())
        };
        let subject = match &self.selection {
            Some(Selection::Node(id)) => self
                .working
                .node(id)
                .map(|n| format!("“{}” ({id})", n.label)),
            Some(Selection::Nodes(ids)) => Some(
                ids.iter()
                    .map(|id| format!("“{}” ({id})", name(id)))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            Some(Selection::Edge(id)) => self
                .working
                .edge(id)
                .map(|e| format!("the connection {} → {} ({id})", name(&e.from), name(&e.to))),
            Some(Selection::Group(id)) => self
                .working
                .group(id)
                .map(|g| format!("the group “{}” ({id})", g.label)),
            None => None,
        };
        let Some(subject) = subject else { return };
        cx.emit(DiagramEvent::Ask(format!(
            "About {subject} in the “{}” diagram ({}): ",
            self.title, self.id
        )));
    }

    pub(super) fn note_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.note.is_none() {
            self.note_language = crate::shared::i18n::language();
            self.note = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(crate::shared::i18n::t(super::NOTE_PLACEHOLDER))
            }));
        }
    }

    pub(super) fn toggle_lock(&mut self, cx: &mut Context<Self>) {
        self.locked = !self.locked;
        if self.locked {
            self.finish_rename(true, cx);
            self.commit_fields(cx);
            self.fields = None;
            self.drag = None;
        }
        cx.notify();
    }
}

/// The nearest grid line.
pub(super) fn snap(v: f32) -> f32 {
    (v / GRID).round() * GRID
}
