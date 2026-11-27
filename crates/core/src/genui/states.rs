//! The state kind: a state machine. States (initial, normal, final,
//! choice) optionally inside composite states, transitions labeled
//! `event [guard] / action`, and the machine's current state highlighted.
//! Laid out with the diagram's layered layout; edited like a diagram.

use serde_json::{Map, Value, json};

use super::diagram::{
    self, Diagram, Direction, Edge, EdgeStyle, Errors, MAX_LABEL, MAX_TEXT, Node, NodeKind, array,
    id_of, text, truncate, wire_enum,
};
use super::edits::{Editable, fresh_id, plural};
use super::layout::{self, Layout, Point};
use super::style::{self, Tint};

pub const MAX_STATES: usize = 150;
pub const MAX_TRANSITIONS: usize = 300;
pub const MAX_GROUPS: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum StateKind {
    Initial,
    #[default]
    Normal,
    Final,
    Choice,
}

wire_enum!(StateKind, 4, [
    Initial => "initial",
    Normal => "normal",
    Final => "final",
    Choice => "choice",
]);

impl StateKind {
    /// Drawn as a small shape (dot, ring, diamond) rather than a card.
    pub fn pseudo(self) -> bool {
        self != StateKind::Normal
    }

    /// The shape's radius (half-diagonal for the choice diamond).
    pub fn radius(self) -> f32 {
        match self {
            StateKind::Initial => 11.,
            StateKind::Final => 13.,
            StateKind::Choice => 20.,
            StateKind::Normal => 0.,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub id: String,
    pub label: String,
    pub kind: StateKind,
    /// The composite state it sits in.
    pub group: Option<String>,
    pub description: Option<String>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    /// Overrides the state's look (`color`, `icon`).
    pub color: Option<Tint>,
    pub icon: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Composite {
    pub id: String,
    pub label: String,
    pub color: Option<Tint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub id: String,
    pub from: String,
    pub to: String,
    pub event: Option<String>,
    pub guard: Option<String>,
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Machine {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub groups: Vec<Composite>,
    pub current: Option<String>,
    pub direction: Direction,
}

impl State {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: StateKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            group: None,
            description: None,
            x: None,
            y: None,
            color: None,
            icon: None,
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("label".into(), json!(self.label));
        map.insert("kind".into(), json!(self.kind.name()));
        if let Some(group) = &self.group {
            map.insert("group".into(), json!(group));
        }
        if let Some(description) = &self.description {
            map.insert("description".into(), json!(description));
        }
        if let (Some(x), Some(y)) = (self.x, self.y) {
            map.insert("x".into(), json!(x.round() as i64));
            map.insert("y".into(), json!(y.round() as i64));
        }
        if let Some(color) = self.color {
            map.insert("color".into(), json!(color.name()));
        }
        if let Some(icon) = self.icon {
            map.insert("icon".into(), json!(icon));
        }
        Value::Object(map)
    }
}

impl Transition {
    /// `event [guard] / action`, the parts it has.
    pub fn label(&self) -> String {
        let mut out = self.event.clone().unwrap_or_default();
        if let Some(guard) = &self.guard {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!("[{guard}]"));
        }
        if let Some(action) = &self.action {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&format!("/ {action}"));
        }
        out
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("from".into(), json!(self.from));
        map.insert("to".into(), json!(self.to));
        for (key, value) in [
            ("event", &self.event),
            ("guard", &self.guard),
            ("action", &self.action),
        ] {
            if let Some(value) = value {
                map.insert(key.into(), json!(value));
            }
        }
        Value::Object(map)
    }
}

impl Machine {
    pub fn parse(spec: &Value) -> Result<Machine, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(raw_states) = spec.get("states").and_then(Value::as_array) else {
            return Err(vec!["\"states\" must be an array".into()]);
        };
        let raw_transitions = array(spec.get("transitions"));
        let raw_groups = array(spec.get("groups"));
        let mut errors = Errors::default();
        for (what, count, max) in [
            ("states", raw_states.len(), MAX_STATES),
            ("transitions", raw_transitions.len(), MAX_TRANSITIONS),
            ("groups", raw_groups.len(), MAX_GROUPS),
        ] {
            if count > max {
                errors.push(format!("too many {what} ({count}, at most {max})"));
            }
        }
        let mut m = Machine {
            direction: spec
                .get("direction")
                .and_then(Value::as_str)
                .and_then(Direction::parse)
                .unwrap_or_default(),
            ..Machine::default()
        };
        for (i, raw) in raw_groups.iter().take(MAX_GROUPS).enumerate() {
            let Some(id) = raw.get("id").and_then(id_of) else {
                errors.push(format!("group #{} has no id", i + 1));
                continue;
            };
            if m.groups.iter().any(|g| g.id == id) {
                errors.push(format!("duplicate group id \"{id}\""));
                continue;
            }
            m.groups.push(Composite {
                label: text(raw.get("label"), MAX_LABEL).unwrap_or_else(|| id.clone()),
                color: Tint::field(raw),
                id,
            });
        }
        for (i, raw) in raw_states.iter().take(MAX_STATES).enumerate() {
            let label = match raw {
                Value::String(_) => text(Some(raw), MAX_LABEL),
                _ => text(raw.get("label"), MAX_LABEL),
            };
            let id = match raw {
                Value::String(_) => id_of(raw),
                _ => raw.get("id").and_then(id_of).or_else(|| label.clone()),
            };
            let Some(id) = id else {
                errors.push(format!("state #{} has no id", i + 1));
                continue;
            };
            if m.state(&id).is_some() {
                errors.push(format!("duplicate state id \"{id}\""));
                continue;
            }
            let group = raw
                .get("group")
                .or_else(|| raw.get("parent"))
                .and_then(id_of);
            if let Some(group) = &group
                && !m.groups.iter().any(|g| &g.id == group)
            {
                errors.push(format!(
                    "state \"{id}\": group \"{group}\" is not in groups"
                ));
            }
            let number = |key: &str| {
                raw.get(key)
                    .and_then(Value::as_f64)
                    .map(|n| n as f32)
                    .filter(|n| n.is_finite())
            };
            let (x, y) = match (number("x"), number("y")) {
                (Some(x), Some(y)) => (Some(x), Some(y)),
                _ => (None, None),
            };
            m.states.push(State {
                label: label.unwrap_or_else(|| id.clone()),
                kind: raw
                    .get("kind")
                    .and_then(Value::as_str)
                    .and_then(StateKind::parse)
                    .unwrap_or_default(),
                group,
                description: text(raw.get("description"), MAX_TEXT),
                x,
                y,
                color: Tint::field(raw),
                icon: style::icon_field(raw),
                id,
            });
        }
        let explicit: Vec<String> = raw_transitions
            .iter()
            .filter_map(|t| t.get("id").and_then(id_of))
            .collect();
        for (i, raw) in raw_transitions.iter().take(MAX_TRANSITIONS).enumerate() {
            let (Some(from), Some(to)) = (
                raw.get("from").and_then(id_of),
                raw.get("to").and_then(id_of),
            ) else {
                errors.push(format!("transition #{} needs \"from\" and \"to\"", i + 1));
                continue;
            };
            let id = match raw.get("id").and_then(id_of) {
                Some(id) => id,
                None => m.fresh_transition_id(&from, &to, |id| explicit.iter().any(|e| e == id)),
            };
            if m.transition(&id).is_some() {
                errors.push(format!("duplicate transition id \"{id}\""));
                continue;
            }
            for end in [&from, &to] {
                if m.state(end).is_none() {
                    errors.push(format!(
                        "transition \"{id}\": state \"{end}\" does not exist"
                    ));
                }
            }
            m.transitions.push(Transition {
                event: text(raw.get("event").or_else(|| raw.get("label")), MAX_LABEL),
                guard: text(raw.get("guard"), MAX_LABEL),
                action: text(raw.get("action"), MAX_LABEL),
                id,
                from,
                to,
            });
        }
        m.current = spec.get("current").and_then(id_of);
        if let Some(current) = &m.current
            && m.state(current).is_none()
        {
            errors.push(format!("current state \"{current}\" does not exist"));
        }
        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(m)
    }

    pub fn state(&self, id: &str) -> Option<&State> {
        self.states.iter().find(|s| s.id == id)
    }

    pub fn state_index(&self, id: &str) -> Option<usize> {
        self.states.iter().position(|s| s.id == id)
    }

    pub fn transition(&self, id: &str) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.id == id)
    }

    pub fn fresh_state_id(&self, label: &str) -> String {
        fresh_id(label, "state", |id| self.state(id).is_some())
    }

    /// `"<from>-><to>"`, then `#2`, `#3`… when taken.
    pub fn fresh_transition_id(
        &self,
        from: &str,
        to: &str,
        taken: impl Fn(&str) -> bool,
    ) -> String {
        let base = format!("{from}->{to}");
        let free = |id: &str| self.transition(id).is_none() && !taken(id);
        if free(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}#{n}"))
            .find(|id| free(id))
            .expect("an unused suffix exists")
    }

    fn name(&self, id: &str) -> String {
        match self.state(id) {
            Some(s) if s.kind == StateKind::Initial => "the initial state".into(),
            Some(s) if s.kind == StateKind::Final && s.label == s.id => "a final state".into(),
            Some(s) => format!("“{}”", truncate(&s.label, 60)),
            None => id.to_string(),
        }
    }

    fn arrow(&self, t: &Transition) -> String {
        let label = t.label();
        let ends = format!("{} → {}", self.name(&t.from), self.name(&t.to));
        if label.is_empty() {
            ends
        } else {
            format!("“{label}” ({ends})")
        }
    }

    /// The machine as the diagram the layered layout places.
    fn proxy(&self) -> Diagram {
        Diagram {
            nodes: self
                .states
                .iter()
                .map(|s| {
                    let mut node = Node::new(s.id.clone(), s.label.clone(), NodeKind::Service);
                    node.group = s.group.clone();
                    node.x = s.x;
                    node.y = s.y;
                    node
                })
                .collect(),
            edges: self
                .transitions
                .iter()
                .map(|t| Edge {
                    id: t.id.clone(),
                    from: t.from.clone(),
                    to: t.to.clone(),
                    // Labelled so the layout sizes gaps and places labels for it.
                    label: Some(t.label()).filter(|l| !l.is_empty()),
                    style: EdgeStyle::Sync,
                    color: None,
                })
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|g| diagram::Group {
                    id: g.id.clone(),
                    label: g.label.clone(),
                    description: None,
                    color: g.color,
                })
                .collect(),
            direction: self.direction,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Move {
        state: String,
        x: f32,
        y: f32,
    },
    AddState {
        state: State,
    },
    /// Its transitions go too.
    RemoveState {
        state: String,
    },
    /// `None` = unchanged; `Some(None)` clears.
    UpdateState {
        state: String,
        label: Option<String>,
        kind: Option<StateKind>,
        group: Option<Option<String>>,
        description: Option<Option<String>>,
    },
    AddTransition {
        transition: Transition,
    },
    RemoveTransition {
        transition: String,
    },
    UpdateTransition {
        transition: String,
        event: Option<Option<String>>,
        guard: Option<Option<String>>,
        action: Option<Option<String>>,
    },
    SetCurrent {
        state: Option<String>,
    },
}

impl Change {
    pub fn rename_state(state: &str, label: String) -> Change {
        Change::UpdateState {
            state: state.into(),
            label: Some(label),
            kind: None,
            group: None,
            description: None,
        }
    }

    pub fn state_kind(state: &str, kind: StateKind) -> Change {
        Change::UpdateState {
            state: state.into(),
            label: None,
            kind: Some(kind),
            group: None,
            description: None,
        }
    }

    pub fn event(transition: &str, event: Option<String>) -> Change {
        Change::UpdateTransition {
            transition: transition.into(),
            event: Some(event),
            guard: None,
            action: None,
        }
    }
}

fn clean(value: &Option<String>, max: usize) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| truncate(s, max))
}

impl Editable for Machine {
    type Change = Change;

    /// Removals, then states (add, update, move), transitions (add,
    /// update) and the current state.
    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut sim = old.clone();
        let mut push = |change: Change, sim: &mut Machine| {
            let _ = sim.apply(&change);
            changes.push(change);
        };
        for t in &old.transitions {
            let kept = new
                .transition(&t.id)
                .is_some_and(|n| n.from == t.from && n.to == t.to);
            // A removed state takes its transitions along (said once, there).
            let ends_kept = new.state(&t.from).is_some() && new.state(&t.to).is_some();
            if !kept && ends_kept && sim.transition(&t.id).is_some() {
                push(
                    Change::RemoveTransition {
                        transition: t.id.clone(),
                    },
                    &mut sim,
                );
            }
        }
        for s in &old.states {
            if new.state(&s.id).is_none() {
                push(
                    Change::RemoveState {
                        state: s.id.clone(),
                    },
                    &mut sim,
                );
            }
        }
        for s in &new.states {
            let Some(was) = sim.state(&s.id).cloned() else {
                push(Change::AddState { state: s.clone() }, &mut sim);
                continue;
            };
            let change = Change::UpdateState {
                state: s.id.clone(),
                label: (was.label != s.label).then(|| s.label.clone()),
                kind: (was.kind != s.kind).then_some(s.kind),
                group: (was.group != s.group).then(|| s.group.clone()),
                description: (was.description != s.description).then(|| s.description.clone()),
            };
            if was.label != s.label
                || was.kind != s.kind
                || was.group != s.group
                || was.description != s.description
            {
                push(change, &mut sim);
            }
            if (was.x, was.y) != (s.x, s.y)
                && let (Some(x), Some(y)) = (s.x, s.y)
            {
                push(
                    Change::Move {
                        state: s.id.clone(),
                        x,
                        y,
                    },
                    &mut sim,
                );
            }
        }
        for t in &new.transitions {
            match sim.transition(&t.id) {
                None => push(
                    Change::AddTransition {
                        transition: t.clone(),
                    },
                    &mut sim,
                ),
                Some(was) if was != t => {
                    let change = Change::UpdateTransition {
                        transition: t.id.clone(),
                        event: (was.event != t.event).then(|| t.event.clone()),
                        guard: (was.guard != t.guard).then(|| t.guard.clone()),
                        action: (was.action != t.action).then(|| t.action.clone()),
                    };
                    push(change, &mut sim);
                }
                Some(_) => {}
            }
        }
        if sim.current != new.current {
            push(
                Change::SetCurrent {
                    state: new.current.clone(),
                },
                &mut sim,
            );
        }
        changes
    }

    fn apply(&mut self, change: &Change) -> Result<(), String> {
        let unknown = |id: &str| format!("unknown state \"{id}\"");
        let no_transition = |id: &str| format!("unknown transition \"{id}\"");
        match change {
            Change::Move { state, x, y } => {
                let s = self
                    .states
                    .iter_mut()
                    .find(|s| s.id == *state)
                    .ok_or_else(|| unknown(state))?;
                s.x = Some(*x);
                s.y = Some(*y);
            }
            Change::AddState { state } => {
                if state.id.trim().is_empty() || state.label.trim().is_empty() {
                    return Err("a new state needs an id and a label".into());
                }
                if self.state(&state.id).is_some() {
                    return Err(format!("state \"{}\" already exists", state.id));
                }
                if let Some(group) = &state.group
                    && !self.groups.iter().any(|g| g.id == *group)
                {
                    return Err(format!("unknown group \"{group}\""));
                }
                if self.states.len() >= MAX_STATES {
                    return Err(format!("too many states (at most {MAX_STATES})"));
                }
                self.states.push(state.clone());
            }
            Change::RemoveState { state } => {
                let at = self.state_index(state).ok_or_else(|| unknown(state))?;
                self.states.remove(at);
                self.transitions
                    .retain(|t| t.from != *state && t.to != *state);
                if self.current.as_deref() == Some(state) {
                    self.current = None;
                }
            }
            Change::UpdateState {
                state,
                label,
                kind,
                group,
                description,
            } => {
                if let Some(Some(group)) = group
                    && !self.groups.iter().any(|g| g.id == *group)
                {
                    return Err(format!("unknown group \"{group}\""));
                }
                let s = self
                    .states
                    .iter_mut()
                    .find(|s| s.id == *state)
                    .ok_or_else(|| unknown(state))?;
                if let Some(label) = label {
                    s.label =
                        clean(&Some(label.clone()), MAX_LABEL).ok_or("a state needs a label")?;
                }
                if let Some(kind) = kind {
                    s.kind = *kind;
                }
                if let Some(group) = group {
                    s.group = group.clone();
                }
                if let Some(description) = description {
                    s.description = clean(description, MAX_TEXT);
                }
            }
            Change::AddTransition { transition } => {
                for end in [&transition.from, &transition.to] {
                    if self.state(end).is_none() {
                        return Err(unknown(end));
                    }
                }
                if transition.id.trim().is_empty() {
                    return Err("a new transition needs an id".into());
                }
                if self.transition(&transition.id).is_some() {
                    return Err(format!("transition \"{}\" already exists", transition.id));
                }
                if self.transitions.len() >= MAX_TRANSITIONS {
                    return Err(format!("too many transitions (at most {MAX_TRANSITIONS})"));
                }
                self.transitions.push(transition.clone());
            }
            Change::RemoveTransition { transition } => {
                let at = self
                    .transitions
                    .iter()
                    .position(|t| t.id == *transition)
                    .ok_or_else(|| no_transition(transition))?;
                self.transitions.remove(at);
            }
            Change::UpdateTransition {
                transition,
                event,
                guard,
                action,
            } => {
                let t = self
                    .transitions
                    .iter_mut()
                    .find(|t| t.id == *transition)
                    .ok_or_else(|| no_transition(transition))?;
                if let Some(event) = event {
                    t.event = clean(event, MAX_LABEL);
                }
                if let Some(guard) = guard {
                    t.guard = clean(guard, MAX_LABEL);
                }
                if let Some(action) = action {
                    t.action = clean(action, MAX_LABEL);
                }
            }
            Change::SetCurrent { state } => {
                if let Some(id) = state
                    && self.state(id).is_none()
                {
                    return Err(unknown(id));
                }
                self.current = state.clone();
            }
        }
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        let some = |map: &mut Map<String, Value>, key: &str, value: &Option<Option<String>>| {
            if let Some(value) = value {
                map.insert(key.into(), json!(value));
            }
        };
        match change {
            Change::Move { state, x, y } => json!({
                "op": "move", "state": state, "x": x.round() as i64, "y": y.round() as i64,
            }),
            Change::AddState { state } => json!({"op": "add_state", "state": state.to_value()}),
            Change::RemoveState { state } => json!({"op": "remove_state", "state": state}),
            Change::UpdateState {
                state,
                label,
                kind,
                group,
                description,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_state"));
                map.insert("state".into(), json!(state));
                if let Some(label) = label {
                    map.insert("label".into(), json!(label));
                }
                if let Some(kind) = kind {
                    map.insert("kind".into(), json!(kind.name()));
                }
                some(&mut map, "group", group);
                some(&mut map, "description", description);
                Value::Object(map)
            }
            Change::AddTransition { transition } => {
                json!({"op": "add_transition", "transition": transition.to_value()})
            }
            Change::RemoveTransition { transition } => {
                json!({"op": "remove_transition", "transition": transition})
            }
            Change::UpdateTransition {
                transition,
                event,
                guard,
                action,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_transition"));
                map.insert("transition".into(), json!(transition));
                some(&mut map, "event", event);
                some(&mut map, "guard", guard);
                some(&mut map, "action", action);
                Value::Object(map)
            }
            Change::SetCurrent { state } => json!({"op": "set_current", "state": state}),
        }
    }

    fn describe(change: &Change, before: &Self, after: &Self) -> String {
        match change {
            Change::Move { state, .. } => format!("moved {}", after.name(state)),
            Change::AddState { state } => match state.kind {
                StateKind::Normal => format!("added state “{}”", state.label),
                kind => format!("added a {} state “{}”", kind.name(), state.label),
            },
            Change::RemoveState { state } => {
                let gone = before
                    .transitions
                    .iter()
                    .filter(|t| t.from == *state || t.to == *state)
                    .count();
                if gone == 0 {
                    format!("removed {}", before.name(state))
                } else {
                    format!(
                        "removed {} (and its {})",
                        before.name(state),
                        plural(gone, "transition")
                    )
                }
            }
            Change::UpdateState {
                state,
                label,
                kind,
                group,
                description,
            } => {
                let mut parts = Vec::new();
                if let Some(label) = label {
                    parts.push(format!("renamed {} to “{label}”", before.name(state)));
                }
                if let Some(kind) = kind {
                    parts.push(format!(
                        "made {} a {} state",
                        after.name(state),
                        kind.name()
                    ));
                }
                match group {
                    Some(Some(g)) => {
                        let label = after
                            .groups
                            .iter()
                            .find(|c| c.id == *g)
                            .map_or(g.as_str(), |c| &c.label);
                        parts.push(format!("moved {} into “{label}”", after.name(state)));
                    }
                    Some(None) => {
                        parts.push(format!("took {} out of its group", after.name(state)))
                    }
                    None => {}
                }
                if description.is_some() {
                    parts.push(format!("changed the description of {}", after.name(state)));
                }
                parts.join(", ")
            }
            Change::AddTransition { transition } => {
                format!("added transition {}", after.arrow(transition))
            }
            Change::RemoveTransition { transition } => match before.transition(transition) {
                Some(t) => format!("removed transition {}", before.arrow(t)),
                None => format!("removed transition {transition}"),
            },
            Change::UpdateTransition { transition, .. } => {
                match (before.transition(transition), after.transition(transition)) {
                    (Some(was), Some(now)) => format!(
                        "relabeled {} → {} from “{}” to “{}”",
                        after.name(&now.from),
                        after.name(&now.to),
                        was.label(),
                        now.label()
                    ),
                    _ => format!("changed transition {transition}"),
                }
            }
            Change::SetCurrent { state } => match state {
                Some(id) => format!("made {} the current state", after.name(id)),
                None => "cleared the current state".into(),
            },
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("direction".into(), json!(self.direction.name()));
        map.insert(
            "groups".into(),
            self.groups
                .iter()
                .map(|g| match g.color {
                    Some(color) => json!({"id": g.id, "label": g.label, "color": color.name()}),
                    None => json!({"id": g.id, "label": g.label}),
                })
                .collect(),
        );
        map.insert(
            "states".into(),
            self.states.iter().map(State::to_value).collect(),
        );
        map.insert(
            "transitions".into(),
            self.transitions.iter().map(Transition::to_value).collect(),
        );
        if let Some(current) = &self.current {
            map.insert("current".into(), json!(current));
        }
        Value::Object(map)
    }
}

/// The diagram's layered layout, with initial, final and choice states
/// drawn as small shapes in the middle of their slot (their `nodes` rect
/// is the shape's box) and transitions ending on those shapes.
pub fn layout(m: &Machine) -> Layout {
    let mut l = layout::layout_with(&m.proxy(), layout::STATES);
    let index = |id: &str| m.state_index(id);
    // Where the end run through `p`, heading along `dir` (a unit vector,
    // toward the shape), meets the shape's border: slid along the run so it
    // stays straight, or aimed at the center when the run misses the shape.
    let shift = |p: Point, dir: Point, center: Point, kind: StateKind| -> Point {
        let r = kind.radius();
        // Offset across the run, and the half-chord at that offset.
        let off = (p.x - center.x) * -dir.y + (p.y - center.y) * dir.x;
        let chord = match kind {
            // A diamond's border: |x| + |y| = r.
            StateKind::Choice => r - off.abs(),
            _ => {
                let sq = r * r - off * off;
                if sq > 0. { sq.sqrt() } else { 0. }
            }
        };
        if chord > 0. {
            // The near side: the end lies before the center along `dir`.
            let side = if (p.x - center.x) * dir.x + (p.y - center.y) * dir.y < 0. {
                -1.
            } else {
                1.
            };
            return Point {
                x: center.x - dir.y * off + dir.x * side * chord,
                y: center.y + dir.x * off + dir.y * side * chord,
            };
        }
        let (dx, dy) = (p.x - center.x, p.y - center.y);
        let scale = match kind {
            StateKind::Choice => r / (dx.abs() + dy.abs()).max(1e-3),
            _ => r / dx.hypot(dy).max(1e-3),
        };
        Point {
            x: center.x + dx * scale,
            y: center.y + dy * scale,
        }
    };
    for (t, path) in m.transitions.iter().zip(l.edges.iter_mut()) {
        if t.from == t.to {
            continue;
        }
        for (end, at_start) in [(&t.from, true), (&t.to, false)] {
            let Some(i) = index(end) else { continue };
            let kind = m.states[i].kind;
            if !kind.pseudo() {
                continue;
            }
            let center = l.nodes[i].center();
            let segment = if at_start {
                path.segments.first_mut()
            } else {
                path.segments.last_mut()
            };
            let Some(seg) = segment else { continue };
            // The end run's direction, heading into the shape.
            let (inner, outer) = if at_start {
                (seg.from, seg.to)
            } else {
                (seg.to, seg.from)
            };
            let len = (inner.x - outer.x).hypot(inner.y - outer.y).max(1e-3);
            let dir = Point {
                x: (inner.x - outer.x) / len,
                y: (inner.y - outer.y) / len,
            };
            let (point, control) = if at_start {
                (&mut seg.from, &mut seg.c1)
            } else {
                (&mut seg.to, &mut seg.c2)
            };
            let moved = shift(*point, dir, center, kind);
            control.x += moved.x - point.x;
            control.y += moved.y - point.y;
            *point = moved;
        }
    }
    for (rect, s) in l.nodes.iter_mut().zip(&m.states) {
        if s.kind.pseudo() {
            let c = rect.center();
            let r = s.kind.radius();
            *rect = layout::Rect {
                x: c.x - r,
                y: c.y - r,
                w: 2. * r,
                h: 2. * r,
            };
        }
    }
    l
}

/// The header's counts.
pub fn counts(m: &Machine) -> String {
    counts_in(m, crate::i18n::Language::English)
}

pub fn counts_in(m: &Machine, language: crate::i18n::Language) -> String {
    use super::edits::count_in;
    format!(
        "{} · {}",
        count_in(language, m.states.len(), "{count} state", "{count} states"),
        count_in(
            language,
            m.transitions.len(),
            "{count} transition",
            "{count} transitions"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn sample() -> Machine {
        Machine::parse(&json!({
            "groups": [{"id": "active", "label": "Active"}],
            "states": [
                {"id": "start", "kind": "initial"},
                "Idle",
                {"id": "running", "label": "Running", "group": "active"},
                {"id": "paused", "label": "Paused", "group": "active"},
                {"id": "check", "kind": "choice"},
                {"id": "done", "kind": "final"}
            ],
            "transitions": [
                {"from": "start", "to": "Idle"},
                {"from": "Idle", "to": "running", "event": "start", "guard": "ready", "action": "log"},
                {"from": "running", "to": "paused", "event": "pause"},
                {"from": "paused", "to": "running", "event": "resume"},
                {"from": "running", "to": "check", "event": "tick"},
                {"from": "check", "to": "done", "guard": "finished"}
            ],
            "current": "running"
        }))
        .unwrap()
    }

    fn round_trip(old: &Machine, new: &Machine) {
        let mut replay = old.clone();
        for change in Machine::diff(old, new) {
            replay
                .apply(&change)
                .unwrap_or_else(|e| panic!("{change:?}: {e}"));
        }
        assert_eq!(&replay, new);
    }

    #[test]
    fn parses_and_labels_transitions() {
        let m = sample();
        assert_eq!(m.states[1].id, "Idle");
        assert_eq!(m.states[0].kind, StateKind::Initial);
        assert_eq!(m.transitions[1].id, "Idle->running");
        assert_eq!(m.transitions[1].label(), "start [ready] / log");
        assert_eq!(m.transitions[5].label(), "[finished]");
        assert_eq!(m.current.as_deref(), Some("running"));
        assert_eq!(Machine::parse(&m.to_value()).unwrap(), m);
    }

    #[test]
    fn readable_errors() {
        let errors = Machine::parse(&json!({
            "states": [{"id": "a", "group": "g"}, {"id": "a"}, {}],
            "transitions": [{"from": "a"}, {"from": "a", "to": "b"}],
            "current": "z"
        }))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "state \"a\": group \"g\" is not in groups",
                "duplicate state id \"a\"",
                "state #3 has no id",
                "transition #1 needs \"from\" and \"to\"",
                "transition \"a->b\": state \"b\" does not exist",
                "current state \"z\" does not exist",
            ]
        );
    }

    #[test]
    fn pseudo_states_are_small_and_their_transitions_touch_them() {
        let m = sample();
        let l = layout(&m);
        let start = l.nodes[0];
        assert_eq!((start.w, start.h), (22., 22.));
        let first = &l.edges[0];
        let c = start.center();
        let p = first.start();
        assert!(((p.x - c.x).hypot(p.y - c.y) - 11.).abs() < 0.01);
        // The choice's outgoing edge leaves from the diamond's border.
        let check = l.nodes[4].center();
        let q = l.edges[5].start();
        assert!(((q.x - check.x).abs() + (q.y - check.y).abs() - 20.).abs() < 0.01);
        // States in a composite get its box.
        assert!(l.groups[0].is_some_and(|g| g.contains(l.nodes[2].center())));
        assert_eq!(l.node_at(c), Some(0));
    }

    #[test]
    fn edits_describe_and_round_trip() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        edits
            .edit(&Change::Move {
                state: "paused".into(),
                x: 600.,
                y: 200.,
            })
            .unwrap();
        let error = State::new(base.fresh_state_id("Error"), "Error", StateKind::Normal);
        edits.edit(&Change::AddState { state: error }).unwrap();
        let transition = Transition {
            id: base.fresh_transition_id("running", "error", |_| false),
            from: "running".into(),
            to: "error".into(),
            event: Some("fail".into()),
            guard: None,
            action: None,
        };
        edits.edit(&Change::AddTransition { transition }).unwrap();
        edits
            .edit(&Change::event("running->paused", Some("suspend".into())))
            .unwrap();
        edits
            .edit(&Change::rename_state("Idle", "Waiting".into()))
            .unwrap();
        edits
            .edit(&Change::RemoveState {
                state: "check".into(),
            })
            .unwrap();
        let (_, lines) = edits.summarize();
        assert_eq!(
            lines,
            vec![
                "removed “check” (and its 2 transitions)",
                "renamed “Idle” to “Waiting”",
                "moved “Paused”",
                "added state “Error”",
                "relabeled “Running” → “Paused” from “pause” to “suspend”",
                "added transition “fail” (“Running” → “Error”)",
            ]
        );
        round_trip(&base, edits.working());
        // Removing the current state clears it.
        let mut m = base.clone();
        m.apply(&Change::RemoveState {
            state: "running".into(),
        })
        .unwrap();
        assert_eq!(m.current, None);
        round_trip(&base, &m);
    }
}
