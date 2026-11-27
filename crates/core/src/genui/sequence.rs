//! The sequence kind: participants side by side (lifelines left to right),
//! messages between them in time order (top to bottom), fragments (alt,
//! opt, loop, par) around runs of messages and activation bars. The user
//! reorders participants and messages, renames them, adds and removes
//! messages and changes their kind.

use serde_json::{Map, Value, json};

use super::diagram::{Errors, MAX_LABEL, MAX_TEXT, array, id_of, text, truncate, wire_enum};
use super::edits::{Editable, fresh_id, plural};
use super::layout::{Point, Rect};
use super::style::{self, Tint};

pub const MAX_PARTICIPANTS: usize = 30;
pub const MAX_MESSAGES: usize = 300;
pub const MAX_FRAGMENTS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ParticipantKind {
    Actor,
    #[default]
    Service,
    Db,
    Queue,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum MessageKind {
    #[default]
    Sync,
    Async,
    Return,
    /// A call to itself (from == to).
    SelfCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FragmentKind {
    Alt,
    #[default]
    Opt,
    Loop,
    Par,
}

wire_enum!(ParticipantKind, 5, [
    Actor => "actor",
    Service => "service",
    Db => "db",
    Queue => "queue",
    External => "external",
]);
wire_enum!(MessageKind, 4, [
    Sync => "sync",
    Async => "async",
    Return => "return",
    SelfCall => "self",
]);
wire_enum!(FragmentKind, 4, [Alt => "alt", Opt => "opt", Loop => "loop", Par => "par"]);

impl ParticipantKind {
    /// The wire name, or a common synonym ("database", "user").
    fn read(s: &str) -> Option<Self> {
        Self::parse(s).or(match s.trim().to_ascii_lowercase().as_str() {
            "database" | "store" => Some(Self::Db),
            "user" | "person" => Some(Self::Actor),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub id: String,
    pub label: String,
    pub kind: ParticipantKind,
    /// Overrides the kind's color and icon (`color`, `icon`).
    pub color: Option<Tint>,
    pub icon: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: String,
    pub from: String,
    pub to: String,
    /// May be empty.
    pub label: String,
    pub kind: MessageKind,
    pub note: Option<String>,
}

/// An `else` (alt) or `and` (par) divider before message `at`.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub at: String,
    pub label: Option<String>,
}

/// A box around the messages `first..=last` (by their place in the list).
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    pub id: String,
    pub kind: FragmentKind,
    pub label: Option<String>,
    pub first: String,
    pub last: String,
    pub branches: Vec<Branch>,
}

/// `participant` is busy from message `first` to message `last`.
#[derive(Debug, Clone, PartialEq)]
pub struct Activation {
    pub participant: String,
    pub first: String,
    pub last: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sequence {
    pub participants: Vec<Participant>,
    pub messages: Vec<Message>,
    pub fragments: Vec<Fragment>,
    /// `None`: derived from the messages (a sync call activates its
    /// target until the matching return).
    pub activations: Option<Vec<Activation>>,
}

impl Participant {
    fn to_value(&self) -> Value {
        let mut value = json!({"id": self.id, "label": self.label, "kind": self.kind.name()});
        if let Some(color) = self.color {
            value["color"] = json!(color.name());
        }
        if let Some(icon) = self.icon {
            value["icon"] = json!(icon);
        }
        value
    }
}

impl Message {
    pub fn new(id: impl Into<String>, from: &str, to: &str, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            label: label.into(),
            kind: if from == to {
                MessageKind::SelfCall
            } else {
                MessageKind::Sync
            },
            note: None,
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("from".into(), json!(self.from));
        map.insert("to".into(), json!(self.to));
        map.insert("label".into(), json!(self.label));
        map.insert("kind".into(), json!(self.kind.name()));
        if let Some(note) = &self.note {
            map.insert("note".into(), json!(note));
        }
        Value::Object(map)
    }
}

impl Fragment {
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("kind".into(), json!(self.kind.name()));
        if let Some(label) = &self.label {
            map.insert("label".into(), json!(label));
        }
        map.insert("from".into(), json!(self.first));
        map.insert("to".into(), json!(self.last));
        if !self.branches.is_empty() {
            let branches: Vec<Value> = self
                .branches
                .iter()
                .map(|b| match &b.label {
                    Some(label) => json!({"at": b.at, "label": label}),
                    None => json!({"at": b.at}),
                })
                .collect();
            map.insert("else".into(), Value::Array(branches));
        }
        Value::Object(map)
    }
}

impl Activation {
    fn to_value(&self) -> Value {
        json!({"participant": self.participant, "from": self.first, "to": self.last})
    }
}

fn structure_value(fragments: &[Fragment], activations: &Option<Vec<Activation>>) -> Value {
    json!({
        "fragments": fragments.iter().map(Fragment::to_value).collect::<Vec<_>>(),
        "activations": activations
            .as_ref()
            .map(|list| list.iter().map(Activation::to_value).collect::<Vec<_>>()),
    })
}

impl Sequence {
    pub fn parse(spec: &Value) -> Result<Sequence, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(raw_messages) = spec.get("messages").and_then(Value::as_array) else {
            return Err(vec!["\"messages\" must be an array".into()]);
        };
        let raw_participants = array(spec.get("participants"));
        let raw_fragments = array(spec.get("fragments"));
        let mut errors = Errors::default();
        for (what, count, max) in [
            ("participants", raw_participants.len(), MAX_PARTICIPANTS),
            ("messages", raw_messages.len(), MAX_MESSAGES),
            ("fragments", raw_fragments.len(), MAX_FRAGMENTS),
        ] {
            if count > max {
                errors.push(format!("too many {what} ({count}, at most {max})"));
            }
        }

        let mut seq = Sequence::default();
        for (i, raw) in raw_participants.iter().take(MAX_PARTICIPANTS).enumerate() {
            // "participants": ["User", "API"] names them by their labels.
            let label = match raw {
                Value::String(_) => text(Some(raw), MAX_LABEL),
                _ => text(raw.get("label").or_else(|| raw.get("name")), MAX_LABEL),
            };
            let id = match raw {
                Value::String(_) => id_of(raw),
                _ => raw.get("id").and_then(id_of).or_else(|| label.clone()),
            };
            let Some(id) = id else {
                errors.push(format!("participant #{} has no id", i + 1));
                continue;
            };
            if seq.participant(&id).is_some() {
                errors.push(format!("duplicate participant id \"{id}\""));
                continue;
            }
            seq.participants.push(Participant {
                label: label.unwrap_or_else(|| id.clone()),
                kind: raw
                    .get("kind")
                    .and_then(Value::as_str)
                    .and_then(ParticipantKind::read)
                    .unwrap_or_default(),
                color: Tint::field(raw),
                icon: style::icon_field(raw),
                id,
            });
        }
        // Without a participants list they come from the messages, in order.
        let implicit = seq.participants.is_empty();

        let explicit: Vec<String> = raw_messages
            .iter()
            .filter_map(|m| m.get("id").and_then(id_of))
            .collect();
        for (i, raw) in raw_messages.iter().take(MAX_MESSAGES).enumerate() {
            let (Some(from), Some(to)) = (
                raw.get("from").and_then(id_of),
                raw.get("to").and_then(id_of),
            ) else {
                errors.push(format!("message #{} needs \"from\" and \"to\"", i + 1));
                continue;
            };
            for end in [&from, &to] {
                if seq.participant(end).is_none() {
                    if implicit && seq.participants.len() < MAX_PARTICIPANTS {
                        seq.participants.push(Participant {
                            id: end.clone(),
                            label: end.clone(),
                            kind: ParticipantKind::default(),
                            color: None,
                            icon: None,
                        });
                    } else {
                        errors.push(format!(
                            "message #{}: participant \"{end}\" is not in participants",
                            i + 1
                        ));
                    }
                }
            }
            let id = match raw.get("id").and_then(id_of) {
                Some(id) => id,
                None => fresh_id(&format!("m{}", i + 1), "m", |id| {
                    explicit.iter().any(|e| e == id) || seq.message(id).is_some()
                }),
            };
            if seq.message(&id).is_some() {
                errors.push(format!("duplicate message id \"{id}\""));
                continue;
            }
            let kind = raw
                .get("kind")
                .or_else(|| raw.get("type"))
                .and_then(Value::as_str)
                .and_then(MessageKind::parse)
                .unwrap_or_default();
            seq.messages.push(Message {
                label: text(raw.get("label").or_else(|| raw.get("text")), MAX_LABEL)
                    .unwrap_or_default(),
                kind: fit_kind(kind, &from, &to),
                note: text(raw.get("note"), MAX_TEXT),
                id,
                from,
                to,
            });
        }

        // A fragment's ends are message ids or 1-based message numbers.
        let resolve = |seq: &Sequence, value: Option<&Value>| -> Result<Option<String>, String> {
            let Some(value) = value.filter(|v| !v.is_null()) else {
                return Ok(None);
            };
            let found = match value {
                Value::Number(n) => n
                    .as_u64()
                    .and_then(|n| seq.messages.get((n as usize).checked_sub(1)?))
                    .map(|m| m.id.clone()),
                other => id_of(other).filter(|id| seq.message(id).is_some()),
            };
            found
                .map(Some)
                .ok_or_else(|| format!("message {value} does not exist"))
        };
        for (i, raw) in raw_fragments.iter().take(MAX_FRAGMENTS).enumerate() {
            let what = format!("fragment #{}", i + 1);
            let (first, last) = match (resolve(&seq, raw.get("from")), resolve(&seq, raw.get("to")))
            {
                (Ok(Some(first)), Ok(last)) => {
                    let last = last.unwrap_or_else(|| first.clone());
                    (first, last)
                }
                (Ok(None), _) => {
                    errors.push(format!(
                        "{what} needs \"from\" (a message id or 1-based number)"
                    ));
                    continue;
                }
                (Err(e), _) | (_, Err(e)) => {
                    errors.push(format!("{what}: {e}"));
                    continue;
                }
            };
            let (a, b) = (seq.index(&first).unwrap(), seq.index(&last).unwrap());
            let (first, last) = if a <= b { (first, last) } else { (last, first) };
            let (a, b) = (a.min(b), a.max(b));
            let mut branches = Vec::new();
            for raw_branch in array(raw.get("else")) {
                match resolve(&seq, raw_branch.get("at")) {
                    Ok(Some(at)) => {
                        let ix = seq.index(&at).unwrap();
                        if ix <= a || ix > b {
                            errors.push(format!(
                                "{what}: its else at \"{at}\" is outside the fragment"
                            ));
                        } else {
                            branches.push(Branch {
                                at,
                                label: text(raw_branch.get("label"), MAX_LABEL),
                            });
                        }
                    }
                    Ok(None) => errors.push(format!("{what}: an else needs \"at\"")),
                    Err(e) => errors.push(format!("{what}: {e}")),
                }
            }
            branches.sort_by_key(|b| seq.index(&b.at));
            let id = raw.get("id").and_then(id_of).unwrap_or_else(|| {
                fresh_id(&format!("f{}", i + 1), "f", |id| {
                    seq.fragments.iter().any(|f| f.id == id)
                })
            });
            if seq.fragments.iter().any(|f| f.id == id) {
                errors.push(format!("duplicate fragment id \"{id}\""));
                continue;
            }
            seq.fragments.push(Fragment {
                id,
                kind: raw
                    .get("kind")
                    .or_else(|| raw.get("type"))
                    .and_then(Value::as_str)
                    .and_then(FragmentKind::parse)
                    .unwrap_or_default(),
                label: text(raw.get("label"), MAX_LABEL),
                first,
                last,
                branches,
            });
        }

        if let Some(raw_activations) = spec.get("activations").and_then(Value::as_array) {
            let mut list = Vec::new();
            for (i, raw) in raw_activations.iter().take(MAX_MESSAGES).enumerate() {
                let what = format!("activation #{}", i + 1);
                let Some(participant) = raw.get("participant").and_then(id_of) else {
                    errors.push(format!("{what} needs a \"participant\""));
                    continue;
                };
                if seq.participant(&participant).is_none() {
                    errors.push(format!(
                        "{what}: participant \"{participant}\" is not in participants"
                    ));
                    continue;
                }
                match (resolve(&seq, raw.get("from")), resolve(&seq, raw.get("to"))) {
                    (Ok(Some(first)), Ok(last)) => {
                        let last = last.unwrap_or_else(|| first.clone());
                        let swap = seq.index(&first) > seq.index(&last);
                        let (first, last) = if swap { (last, first) } else { (first, last) };
                        list.push(Activation {
                            participant,
                            first,
                            last,
                        });
                    }
                    (Ok(None), _) => errors.push(format!("{what} needs \"from\"")),
                    (Err(e), _) | (_, Err(e)) => errors.push(format!("{what}: {e}")),
                }
            }
            seq.activations = Some(list);
        }

        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(seq)
    }

    pub fn participant(&self, id: &str) -> Option<&Participant> {
        self.participants.iter().find(|p| p.id == id)
    }

    pub fn lane(&self, id: &str) -> Option<usize> {
        self.participants.iter().position(|p| p.id == id)
    }

    pub fn message(&self, id: &str) -> Option<&Message> {
        self.messages.iter().find(|m| m.id == id)
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.messages.iter().position(|m| m.id == id)
    }

    pub fn fresh_message_id(&self) -> String {
        fresh_id(&format!("m{}", self.messages.len() + 1), "m", |id| {
            self.message(id).is_some()
        })
    }

    pub fn fresh_participant_id(&self, label: &str) -> String {
        fresh_id(label, "participant", |id| self.participant(id).is_some())
    }

    /// The order `id` dropped at `index` (among the others) gives.
    pub fn moved<'a>(ids: impl Iterator<Item = &'a str>, id: &str, index: usize) -> Vec<String> {
        let mut out: Vec<String> = ids.filter(|i| *i != id).map(str::to_string).collect();
        out.insert(index.min(out.len()), id.to_string());
        out
    }

    /// A message's ends and label as the agent would read it.
    fn name(&self, id: &str) -> String {
        match self.message(id) {
            Some(m) if !m.label.is_empty() => format!("“{}”", truncate(&m.label, 60)),
            Some(m) => format!("{} → {}", self.who(&m.from), self.who(&m.to)),
            None => id.to_string(),
        }
    }

    fn who(&self, id: &str) -> String {
        self.participant(id)
            .map_or_else(|| id.to_string(), |p| p.label.clone())
    }

    /// Where a range's ends sit now (in order), when both still exist.
    fn span(&self, first: &str, last: &str) -> Option<(usize, usize)> {
        let (a, b) = (self.index(first)?, self.index(last)?);
        Some((a.min(b), a.max(b)))
    }

    /// Activation bars as (lane, first message, last message): the spec's,
    /// or derived: a sync call activates its target until the target's
    /// next return (or its last message, when none comes back).
    pub fn bars(&self) -> Vec<(usize, usize, usize)> {
        if let Some(list) = &self.activations {
            return list
                .iter()
                .filter_map(|a| {
                    let (first, last) = self.span(&a.first, &a.last)?;
                    Some((self.lane(&a.participant)?, first, last))
                })
                .collect();
        }
        let mut open: Vec<(String, usize)> = Vec::new();
        let mut bars = Vec::new();
        let close = |participant: &str, at: usize, bars: &mut Vec<(usize, usize, usize)>, first| {
            if let Some(lane) = self.lane(participant) {
                bars.push((lane, first, at));
            }
        };
        for (i, m) in self.messages.iter().enumerate() {
            match m.kind {
                MessageKind::Sync => open.push((m.to.clone(), i)),
                MessageKind::Return => {
                    if let Some(at) = open.iter().rposition(|(p, _)| *p == m.from) {
                        let (p, first) = open.remove(at);
                        close(&p, i, &mut bars, first);
                    }
                }
                _ => {}
            }
        }
        for (p, first) in open {
            let last = (first..self.messages.len())
                .rev()
                .find(|&j| self.messages[j].from == p || self.messages[j].to == p)
                .unwrap_or(first);
            close(&p, last, &mut bars, first);
        }
        bars.sort_by_key(|&(lane, first, _)| (lane, first));
        bars
    }

    /// Takes message `id` out, shrinking the fragments and activations
    /// that started or ended at it (one left empty goes too).
    fn take_message(&mut self, id: &str) -> Option<Message> {
        let at = self.index(id)?;
        let ids: Vec<String> = self.messages.iter().map(|m| m.id.clone()).collect();
        let shrink = |first: &mut String, last: &mut String| -> bool {
            let pos = |m: &str| ids.iter().position(|i| i == m);
            let (Some(a), Some(b)) = (pos(first), pos(last)) else {
                return false;
            };
            let (a, b) = (a.min(b), a.max(b));
            if a == at && b == at {
                return false;
            }
            let a = if a == at { a + 1 } else { a };
            let b = if b == at { b - 1 } else { b };
            *first = ids[a].clone();
            *last = ids[b].clone();
            true
        };
        self.fragments.retain_mut(|f| {
            if !shrink(&mut f.first, &mut f.last) {
                return false;
            }
            let start = ids.iter().position(|i| *i == f.first).unwrap_or(0);
            let end = ids.iter().position(|i| *i == f.last).unwrap_or(0);
            for branch in &mut f.branches {
                if branch.at == id {
                    // The divider moves to the next message, if any is left.
                    branch.at = ids
                        .get(at + 1)
                        .filter(|_| at < end)
                        .cloned()
                        .unwrap_or_default();
                }
            }
            f.branches.retain(|b| {
                ids.iter()
                    .position(|i| *i == b.at)
                    .is_some_and(|ix| ix > start && ix <= end)
            });
            f.branches.dedup_by(|a, b| a.at == b.at);
            true
        });
        if let Some(list) = &mut self.activations {
            list.retain_mut(|a| shrink(&mut a.first, &mut a.last));
        }
        Some(self.messages.remove(at))
    }
}

/// A message to itself is a self call; only one can be.
fn fit_kind(kind: MessageKind, from: &str, to: &str) -> MessageKind {
    match (from == to, kind) {
        (true, _) => MessageKind::SelfCall,
        (false, MessageKind::SelfCall) => MessageKind::Sync,
        (false, kind) => kind,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    AddParticipant {
        participant: Participant,
    },
    /// Removes its messages too.
    RemoveParticipant {
        participant: String,
    },
    OrderParticipants {
        participants: Vec<String>,
    },
    /// `None` = unchanged.
    UpdateParticipant {
        participant: String,
        label: Option<String>,
        kind: Option<ParticipantKind>,
    },
    /// Inserted at `index` (clamped).
    AddMessage {
        message: Message,
        index: usize,
    },
    RemoveMessage {
        message: String,
    },
    /// Every message id, in their new order.
    OrderMessages {
        messages: Vec<String>,
    },
    /// `None` = unchanged; a note of `Some(None)` is cleared.
    UpdateMessage {
        message: String,
        label: Option<String>,
        kind: Option<MessageKind>,
        from: Option<String>,
        to: Option<String>,
        note: Option<Option<String>>,
    },
    /// Fragments and activations as a whole (what removing and reordering
    /// messages left them as).
    SetStructure {
        fragments: Vec<Fragment>,
        activations: Option<Vec<Activation>>,
    },
}

impl Change {
    /// An `UpdateMessage` changing only the given label, kind and target.
    pub fn update_message(
        message: &str,
        label: Option<String>,
        kind: Option<MessageKind>,
        to: Option<String>,
    ) -> Self {
        Change::UpdateMessage {
            message: message.to_string(),
            label,
            kind,
            from: None,
            to,
            note: None,
        }
    }
}

impl Editable for Sequence {
    type Change = Change;

    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut sim = old.clone();
        let mut push = |change: Change, sim: &mut Sequence| {
            let _ = sim.apply(&change);
            changes.push(change);
        };
        for p in &new.participants {
            if sim.participant(&p.id).is_none() {
                push(
                    Change::AddParticipant {
                        participant: p.clone(),
                    },
                    &mut sim,
                );
            }
        }
        for p in &old.participants {
            if new.participant(&p.id).is_none() {
                push(
                    Change::RemoveParticipant {
                        participant: p.id.clone(),
                    },
                    &mut sim,
                );
            }
        }
        let lanes =
            |s: &Sequence| -> Vec<String> { s.participants.iter().map(|p| p.id.clone()).collect() };
        if lanes(&sim) != lanes(new) {
            push(
                Change::OrderParticipants {
                    participants: lanes(new),
                },
                &mut sim,
            );
        }
        for m in &old.messages {
            if new.message(&m.id).is_none() && sim.message(&m.id).is_some() {
                push(
                    Change::RemoveMessage {
                        message: m.id.clone(),
                    },
                    &mut sim,
                );
            }
        }
        for (index, m) in new.messages.iter().enumerate() {
            if sim.message(&m.id).is_none() {
                push(
                    Change::AddMessage {
                        message: m.clone(),
                        index,
                    },
                    &mut sim,
                );
            }
        }
        let order =
            |s: &Sequence| -> Vec<String> { s.messages.iter().map(|m| m.id.clone()).collect() };
        if order(&sim) != order(new) {
            push(
                Change::OrderMessages {
                    messages: order(new),
                },
                &mut sim,
            );
        }
        for p in &new.participants {
            let Some(before) = sim.participant(&p.id) else {
                continue;
            };
            if before != p {
                let change = Change::UpdateParticipant {
                    participant: p.id.clone(),
                    label: (before.label != p.label).then(|| p.label.clone()),
                    kind: (before.kind != p.kind).then_some(p.kind),
                };
                push(change, &mut sim);
            }
        }
        for m in &new.messages {
            let Some(before) = sim.message(&m.id) else {
                continue;
            };
            if before != m {
                let change = Change::UpdateMessage {
                    message: m.id.clone(),
                    label: (before.label != m.label).then(|| m.label.clone()),
                    kind: (before.kind != m.kind).then_some(m.kind),
                    from: (before.from != m.from).then(|| m.from.clone()),
                    to: (before.to != m.to).then(|| m.to.clone()),
                    note: (before.note != m.note).then(|| m.note.clone()),
                };
                push(change, &mut sim);
            }
        }
        if sim.fragments != new.fragments || sim.activations != new.activations {
            push(
                Change::SetStructure {
                    fragments: new.fragments.clone(),
                    activations: new.activations.clone(),
                },
                &mut sim,
            );
        }
        changes
    }

    fn apply(&mut self, change: &Change) -> Result<(), String> {
        let no_participant = |id: &str| format!("unknown participant \"{id}\"");
        let no_message = |id: &str| format!("unknown message \"{id}\"");
        match change {
            Change::AddParticipant { participant } => {
                if participant.id.trim().is_empty() || participant.label.trim().is_empty() {
                    return Err("a new participant needs an id and a label".into());
                }
                if self.participant(&participant.id).is_some() {
                    return Err(format!("participant \"{}\" already exists", participant.id));
                }
                if self.participants.len() >= MAX_PARTICIPANTS {
                    return Err(format!(
                        "too many participants (at most {MAX_PARTICIPANTS})"
                    ));
                }
                let mut p = participant.clone();
                p.label = truncate(p.label.trim(), MAX_LABEL);
                self.participants.push(p);
            }
            Change::RemoveParticipant { participant } => {
                let lane = self
                    .lane(participant)
                    .ok_or_else(|| no_participant(participant))?;
                let gone: Vec<String> = self
                    .messages
                    .iter()
                    .filter(|m| m.from == *participant || m.to == *participant)
                    .map(|m| m.id.clone())
                    .collect();
                for id in gone {
                    self.take_message(&id);
                }
                if let Some(list) = &mut self.activations {
                    list.retain(|a| a.participant != *participant);
                }
                self.participants.remove(lane);
            }
            Change::OrderParticipants { participants } => {
                if let Some(missing) = participants.iter().find(|p| self.participant(p).is_none()) {
                    return Err(no_participant(missing));
                }
                let mut next: Vec<Participant> = participants
                    .iter()
                    .filter_map(|id| self.participant(id).cloned())
                    .collect();
                next.dedup_by(|a, b| a.id == b.id);
                for p in &self.participants {
                    if !participants.contains(&p.id) {
                        next.push(p.clone());
                    }
                }
                self.participants = next;
            }
            Change::UpdateParticipant {
                participant,
                label,
                kind,
            } => {
                let p = self
                    .participants
                    .iter_mut()
                    .find(|p| p.id == *participant)
                    .ok_or_else(|| no_participant(participant))?;
                if let Some(label) = label {
                    let label = label.trim();
                    if label.is_empty() {
                        return Err("a participant needs a label".into());
                    }
                    p.label = truncate(label, MAX_LABEL);
                }
                if let Some(kind) = kind {
                    p.kind = *kind;
                }
            }
            Change::AddMessage { message, index } => {
                if message.id.trim().is_empty() {
                    return Err("a new message needs an id".into());
                }
                if self.message(&message.id).is_some() {
                    return Err(format!("message \"{}\" already exists", message.id));
                }
                for end in [&message.from, &message.to] {
                    if self.participant(end).is_none() {
                        return Err(no_participant(end));
                    }
                }
                if self.messages.len() >= MAX_MESSAGES {
                    return Err(format!("too many messages (at most {MAX_MESSAGES})"));
                }
                let mut m = message.clone();
                m.label = truncate(m.label.trim(), MAX_LABEL);
                m.kind = fit_kind(m.kind, &m.from, &m.to);
                let at = (*index).min(self.messages.len());
                self.messages.insert(at, m);
            }
            Change::RemoveMessage { message } => {
                self.take_message(message)
                    .ok_or_else(|| no_message(message))?;
            }
            Change::OrderMessages { messages } => {
                if let Some(missing) = messages.iter().find(|m| self.message(m).is_none()) {
                    return Err(no_message(missing));
                }
                let mut next: Vec<Message> = messages
                    .iter()
                    .filter_map(|id| self.message(id).cloned())
                    .collect();
                next.dedup_by(|a, b| a.id == b.id);
                for m in &self.messages {
                    if !messages.contains(&m.id) {
                        next.push(m.clone());
                    }
                }
                self.messages = next;
            }
            Change::UpdateMessage {
                message,
                label,
                kind,
                from,
                to,
                note,
            } => {
                for end in [from, to].into_iter().flatten() {
                    if self.participant(end).is_none() {
                        return Err(no_participant(end));
                    }
                }
                let m = self
                    .messages
                    .iter_mut()
                    .find(|m| m.id == *message)
                    .ok_or_else(|| no_message(message))?;
                let mut next = m.clone();
                if let Some(label) = label {
                    next.label = truncate(label.trim(), MAX_LABEL);
                }
                if let Some(from) = from {
                    next.from = from.clone();
                }
                if let Some(to) = to {
                    next.to = to.clone();
                }
                if let Some(kind) = kind {
                    if *kind == MessageKind::SelfCall && next.from != next.to {
                        return Err("only a message to its own sender can be a self call".into());
                    }
                    next.kind = *kind;
                }
                next.kind = fit_kind(next.kind, &next.from, &next.to);
                if let Some(note) = note {
                    next.note = note
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .map(|n| truncate(n, MAX_TEXT));
                }
                *m = next;
            }
            Change::SetStructure {
                fragments,
                activations,
            } => {
                let ends = fragments
                    .iter()
                    .flat_map(|f| {
                        [&f.first, &f.last]
                            .into_iter()
                            .chain(f.branches.iter().map(|b| &b.at))
                    })
                    .chain(
                        activations
                            .iter()
                            .flatten()
                            .flat_map(|a| [&a.first, &a.last]),
                    );
                for id in ends {
                    if self.message(id).is_none() {
                        return Err(no_message(id));
                    }
                }
                if let Some(p) = activations
                    .iter()
                    .flatten()
                    .find(|a| self.participant(&a.participant).is_none())
                {
                    return Err(no_participant(&p.participant));
                }
                self.fragments = fragments.clone();
                self.activations = activations.clone();
            }
        }
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        match change {
            Change::AddParticipant { participant } => {
                json!({"op": "add_participant", "participant": participant.to_value()})
            }
            Change::RemoveParticipant { participant } => {
                json!({"op": "remove_participant", "participant": participant})
            }
            Change::OrderParticipants { participants } => {
                json!({"op": "order_participants", "participants": participants})
            }
            Change::UpdateParticipant {
                participant,
                label,
                kind,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_participant"));
                map.insert("participant".into(), json!(participant));
                if let Some(label) = label {
                    map.insert("label".into(), json!(label));
                }
                if let Some(kind) = kind {
                    map.insert("kind".into(), json!(kind.name()));
                }
                Value::Object(map)
            }
            Change::AddMessage { message, index } => {
                json!({"op": "add_message", "message": message.to_value(), "index": index})
            }
            Change::RemoveMessage { message } => {
                json!({"op": "remove_message", "message": message})
            }
            Change::OrderMessages { messages } => {
                json!({"op": "order_messages", "messages": messages})
            }
            Change::UpdateMessage {
                message,
                label,
                kind,
                from,
                to,
                note,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_message"));
                map.insert("message".into(), json!(message));
                if let Some(label) = label {
                    map.insert("label".into(), json!(label));
                }
                if let Some(kind) = kind {
                    map.insert("kind".into(), json!(kind.name()));
                }
                if let Some(from) = from {
                    map.insert("from".into(), json!(from));
                }
                if let Some(to) = to {
                    map.insert("to".into(), json!(to));
                }
                if let Some(note) = note {
                    map.insert("note".into(), json!(note));
                }
                Value::Object(map)
            }
            Change::SetStructure {
                fragments,
                activations,
            } => {
                let mut value = structure_value(fragments, activations);
                value["op"] = json!("set_structure");
                value
            }
        }
    }

    fn describe(change: &Change, before: &Self, after: &Self) -> String {
        let who = |id: &str| after.who(id);
        match change {
            Change::AddParticipant { participant } => {
                format!("added participant “{}”", participant.label)
            }
            Change::RemoveParticipant { participant } => {
                let gone = before
                    .messages
                    .iter()
                    .filter(|m| m.from == *participant || m.to == *participant)
                    .count();
                let name = before.who(participant);
                if gone == 0 {
                    format!("removed participant “{name}”")
                } else {
                    format!(
                        "removed participant “{name}” (and its {})",
                        plural(gone, "message")
                    )
                }
            }
            Change::OrderParticipants { .. } => format!(
                "reordered the participants: {}",
                after
                    .participants
                    .iter()
                    .map(|p| p.label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Change::UpdateParticipant {
                participant,
                label,
                kind,
            } => {
                let mut parts = Vec::new();
                if let Some(label) = label {
                    parts.push(format!(
                        "renamed participant “{}” to “{label}”",
                        before.who(participant)
                    ));
                }
                if let Some(kind) = kind {
                    parts.push(format!("made “{}” a {}", who(participant), kind.name()));
                }
                parts.join(", ")
            }
            Change::AddMessage { message, index } => {
                let ends = format!("from {} to {}", who(&message.from), who(&message.to));
                let step = index + 1;
                if message.label.is_empty() {
                    format!("added a message {ends} (step {step})")
                } else {
                    format!("added message “{}” {ends} (step {step})", message.label)
                }
            }
            Change::RemoveMessage { message } => {
                format!("removed message {}", before.name(message))
            }
            Change::OrderMessages { messages } => {
                let old: Vec<&str> = before.messages.iter().map(|m| m.id.as_str()).collect();
                // One message dragged elsewhere: say which and where.
                let moved = messages.iter().find(|id| {
                    let without = |list: Vec<&str>| -> Vec<String> {
                        list.into_iter()
                            .filter(|i| i != id)
                            .map(str::to_string)
                            .collect()
                    };
                    without(old.clone()) == without(messages.iter().map(String::as_str).collect())
                        && before.index(id) != after.index(id)
                });
                match moved.and_then(|id| Some((id, after.index(id)?))) {
                    Some((id, at)) => {
                        format!("moved message {} to step {}", after.name(id), at + 1)
                    }
                    None => "reordered the messages".into(),
                }
            }
            Change::UpdateMessage {
                message,
                label,
                kind,
                from,
                to,
                note,
            } => {
                let mut parts = Vec::new();
                if let Some(label) = label {
                    parts.push(format!(
                        "renamed message {} to “{label}”",
                        before.name(message)
                    ));
                }
                if (from.is_some() || to.is_some())
                    && let Some(m) = after.message(message)
                {
                    parts.push(format!(
                        "made {} go from {} to {}",
                        after.name(message),
                        who(&m.from),
                        who(&m.to)
                    ));
                }
                if let Some(kind) = kind {
                    let what = match kind {
                        MessageKind::Sync => "a sync call",
                        MessageKind::Async => "async",
                        MessageKind::Return => "a return",
                        MessageKind::SelfCall => "a self call",
                    };
                    parts.push(format!("made {} {what}", after.name(message)));
                }
                match note {
                    Some(Some(_)) => {
                        parts.push(format!("updated the note on {}", after.name(message)))
                    }
                    Some(None) => {
                        parts.push(format!("cleared the note on {}", after.name(message)))
                    }
                    None => {}
                }
                parts.join(", ")
            }
            Change::SetStructure { .. } => "adjusted the fragments and activations".into(),
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "participants".into(),
            self.participants
                .iter()
                .map(Participant::to_value)
                .collect(),
        );
        map.insert(
            "messages".into(),
            self.messages.iter().map(Message::to_value).collect(),
        );
        map.insert(
            "fragments".into(),
            self.fragments.iter().map(Fragment::to_value).collect(),
        );
        if let Some(list) = &self.activations {
            map.insert(
                "activations".into(),
                list.iter().map(Activation::to_value).collect(),
            );
        }
        Value::Object(map)
    }
}

// ── Layout ─────────────────────────────────────────────────────────────────

/// Lifelines' spacing, center to center.
pub const LANE: f32 = 200.;
pub const HEAD_W: f32 = 150.;
pub const HEAD_H: f32 = 48.;
pub const ROW: f32 = 46.;
pub const SELF_ROW: f32 = 64.;
pub const BAR_W: f32 = 10.;
/// How far a self call loops out to the right.
pub const SELF_LOOP: f32 = 44.;
const FIRST_ROW: f32 = HEAD_H + 30.;
const FRAGMENT_HEAD: f32 = 26.;
const BRANCH_GAP: f32 = 22.;
const FRAGMENT_FOOT: f32 = 10.;

#[derive(Debug, Clone, PartialEq)]
pub struct MessageGeom {
    pub from: Point,
    pub to: Point,
    /// The band the message takes (for picking it).
    pub row: Rect,
    /// Where its label's center goes.
    pub label: Point,
    pub self_loop: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FragmentGeom {
    pub rect: Rect,
    /// Each branch's divider height, with its label.
    pub branches: Vec<(f32, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceLayout {
    /// The participants' header boxes, index-aligned.
    pub heads: Vec<Rect>,
    /// Where the lifelines end.
    pub lifeline_end: f32,
    pub messages: Vec<MessageGeom>,
    pub fragments: Vec<FragmentGeom>,
    /// (lane, bar).
    pub bars: Vec<(usize, Rect)>,
    pub bounds: Rect,
}

pub fn lane_center(lane: usize) -> f32 {
    lane as f32 * LANE + HEAD_W / 2.
}

impl SequenceLayout {
    /// The lane nearest to `x`.
    pub fn lane_at(&self, x: f32) -> usize {
        let last = self.heads.len().saturating_sub(1);
        ((x - HEAD_W / 2.) / LANE).round().clamp(0., last as f32) as usize
    }

    /// The index a message dropped at `y` takes among the others.
    pub fn insert_at(&self, y: f32, without: Option<usize>) -> usize {
        self.messages
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != without)
            .filter(|(_, m)| m.from.y < y)
            .count()
    }

    pub fn message_at(&self, p: Point) -> Option<usize> {
        self.messages.iter().position(|m| m.row.contains(p))
    }
}

pub fn layout(seq: &Sequence) -> SequenceLayout {
    let n = seq.messages.len();
    let heads: Vec<Rect> = (0..seq.participants.len())
        .map(|lane| Rect {
            x: lane_center(lane) - HEAD_W / 2.,
            y: 0.,
            w: HEAD_W,
            h: HEAD_H,
        })
        .collect();
    let spans: Vec<(usize, usize)> = seq
        .fragments
        .iter()
        .map(|f| seq.span(&f.first, &f.last).unwrap_or((0, 0)))
        .collect();
    let branch_at = |f: &Fragment, (a, b): (usize, usize)| -> Vec<(usize, Option<String>)> {
        f.branches
            .iter()
            .filter_map(|br| {
                let ix = seq.index(&br.at)?;
                (ix > a && ix <= b).then(|| (ix, br.label.clone()))
            })
            .collect()
    };
    // Fragments starting at a message, outermost (longest) first; ending
    // at one, innermost first.
    let mut starting: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut ending: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut branches_at = vec![0usize; n];
    for (fi, f) in seq.fragments.iter().enumerate() {
        let (a, b) = spans[fi];
        if n == 0 {
            break;
        }
        starting[a].push(fi);
        ending[b].push(fi);
        for (ix, _) in branch_at(f, (a, b)) {
            branches_at[ix] += 1;
        }
    }
    for list in &mut starting {
        list.sort_by_key(|&fi| std::cmp::Reverse(spans[fi].1));
    }
    for list in &mut ending {
        list.sort_by_key(|&fi| std::cmp::Reverse(spans[fi].0));
    }

    let mut tops = vec![0.; n];
    let mut frag_top = vec![0.; seq.fragments.len()];
    let mut frag_bottom = vec![0.; seq.fragments.len()];
    let mut branch_y: Vec<f32> = vec![0.; n];
    let mut y = FIRST_ROW;
    for (i, m) in seq.messages.iter().enumerate() {
        if branches_at[i] > 0 {
            branch_y[i] = y + 4.;
            y += BRANCH_GAP;
        }
        for &fi in &starting[i] {
            frag_top[fi] = y;
            y += FRAGMENT_HEAD;
        }
        tops[i] = y;
        y += if m.kind == MessageKind::SelfCall {
            SELF_ROW
        } else {
            ROW
        };
        for (k, &fi) in ending[i].iter().enumerate() {
            frag_bottom[fi] = y + FRAGMENT_FOOT * (k as f32 + 1.);
        }
        y += FRAGMENT_FOOT * ending[i].len() as f32;
    }
    let lifeline_end = y + 28.;
    let arrow_y = |i: usize| tops[i] + ROW * 0.62;

    let bar_list = seq.bars();
    // Overlapping bars on one lane step right.
    let mut bars: Vec<(usize, Rect)> = Vec::new();
    let mut covering: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    for (lane, first, last) in &bar_list {
        let depth = bars
            .iter()
            .zip(&bar_list)
            .filter(|(_, (l, f, e))| l == lane && *f <= *first && *e >= *first)
            .count() as f32;
        let x = lane_center(*lane) - BAR_W / 2. + depth * BAR_W * 0.6;
        let rect = Rect {
            x,
            y: arrow_y(*first) - 6.,
            w: BAR_W,
            h: arrow_y(*last) - arrow_y(*first) + 12.,
        };
        for cover in covering.iter_mut().take(last + 1).skip(*first) {
            cover.push((*lane, x));
        }
        bars.push((*lane, rect));
    }

    let messages: Vec<MessageGeom> = seq
        .messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let (Some(a), Some(b)) = (seq.lane(&m.from), seq.lane(&m.to)) else {
                return MessageGeom {
                    from: Point::default(),
                    to: Point::default(),
                    row: Rect::default(),
                    label: Point::default(),
                    self_loop: false,
                };
            };
            let y = arrow_y(i);
            // The outermost bar side facing the other end.
            let edge = |lane: usize, right: bool| -> f32 {
                let xs = covering[i]
                    .iter()
                    .filter(|(l, _)| *l == lane)
                    .map(|(_, x)| *x);
                if right {
                    xs.map(|x| x + BAR_W).fold(lane_center(lane), f32::max)
                } else {
                    xs.fold(lane_center(lane), f32::min)
                }
            };
            if a == b {
                let x = edge(a, true);
                return MessageGeom {
                    from: Point { x, y },
                    to: Point { x, y: y + 22. },
                    row: Rect {
                        x: lane_center(a) - 20.,
                        y: tops[i],
                        w: SELF_LOOP + 40.,
                        h: SELF_ROW,
                    },
                    label: Point {
                        x: x + SELF_LOOP + 76.,
                        y: y + 11.,
                    },
                    self_loop: true,
                };
            }
            let right = b > a;
            let from = Point {
                x: edge(a, right),
                y,
            };
            let to = Point {
                x: edge(b, !right),
                y,
            };
            MessageGeom {
                row: Rect {
                    x: from.x.min(to.x),
                    y: tops[i],
                    w: (to.x - from.x).abs(),
                    h: ROW,
                },
                label: Point {
                    x: (from.x + to.x) / 2.,
                    y: y - 13.,
                },
                from,
                to,
                self_loop: false,
            }
        })
        .collect();

    // Fragments span their messages' lanes, wider the more they hold.
    let fragments: Vec<FragmentGeom> = seq
        .fragments
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            let (a, b) = spans[fi];
            let inner = spans
                .iter()
                .enumerate()
                .filter(|&(gi, &(c, d))| gi != fi && c >= a && d <= b && (c, d) != (a, b))
                .count()
                .min(3) as f32;
            let pad = 56. + 12. * inner;
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for (m, g) in seq.messages[a..=b.min(n.saturating_sub(1))]
                .iter()
                .zip(&messages[a..=b.min(n.saturating_sub(1))])
            {
                for lane in [seq.lane(&m.from), seq.lane(&m.to)].into_iter().flatten() {
                    lo = lo.min(lane_center(lane));
                    hi = hi.max(lane_center(lane));
                }
                if g.self_loop {
                    hi = hi.max(g.from.x + SELF_LOOP);
                }
            }
            if !lo.is_finite() {
                lo = 0.;
                hi = 0.;
            }
            let rect = Rect {
                x: lo - pad,
                y: frag_top[fi],
                w: hi - lo + 2. * pad,
                h: frag_bottom[fi] - frag_top[fi],
            };
            FragmentGeom {
                rect,
                branches: branch_at(f, (a, b))
                    .into_iter()
                    .map(|(ix, label)| (branch_y[ix], label))
                    .collect(),
            }
        })
        .collect();

    let bounds = heads
        .iter()
        .copied()
        .chain(fragments.iter().map(|f| f.rect))
        .chain(messages.iter().map(|m| m.row.inflate(10.)))
        .chain(messages.iter().filter(|m| m.self_loop).map(|m| Rect {
            x: m.label.x - 70.,
            y: m.label.y - 10.,
            w: 140.,
            h: 20.,
        }))
        .chain(std::iter::once(Rect {
            x: 0.,
            y: 0.,
            w: HEAD_W,
            h: lifeline_end,
        }))
        .reduce(|a, b| a.union(&b))
        .unwrap_or_default();
    SequenceLayout {
        heads,
        lifeline_end,
        messages,
        fragments,
        bars,
        bounds,
    }
}

/// The header's counts.
pub fn counts(seq: &Sequence) -> String {
    counts_in(seq, crate::i18n::Language::English)
}

pub fn counts_in(seq: &Sequence, language: crate::i18n::Language) -> String {
    use super::edits::count_in;
    format!(
        "{} · {}",
        count_in(
            language,
            seq.participants.len(),
            "{count} participant",
            "{count} participants"
        ),
        count_in(
            language,
            seq.messages.len(),
            "{count} message",
            "{count} messages"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn sample() -> Sequence {
        Sequence::parse(&json!({
            "participants": [
                {"id": "user", "label": "User", "kind": "actor"},
                {"id": "api", "label": "API"},
                {"id": "db", "label": "Orders DB", "kind": "database"}
            ],
            "messages": [
                {"from": "user", "to": "api", "label": "POST /orders"},
                {"from": "api", "to": "db", "label": "insert"},
                {"from": "db", "to": "api", "label": "id", "kind": "return"},
                {"from": "api", "to": "api", "label": "validate"},
                {"from": "api", "to": "user", "label": "201", "kind": "return"}
            ],
            "fragments": [
                {"kind": "alt", "label": "valid", "from": 2, "to": "m4", "else": [{"at": 4, "label": "invalid"}]}
            ]
        }))
        .unwrap()
    }

    fn round_trip(old: &Sequence, new: &Sequence) {
        let mut replay = old.clone();
        for change in Sequence::diff(old, new) {
            replay
                .apply(&change)
                .unwrap_or_else(|e| panic!("{change:?}: {e}"));
        }
        assert_eq!(&replay, new);
    }

    #[test]
    fn parses_with_defaults_and_round_trips_to_json() {
        let seq = sample();
        assert_eq!(seq.participants[2].kind, ParticipantKind::Db);
        assert_eq!(seq.messages[0].id, "m1");
        assert_eq!(seq.messages[3].kind, MessageKind::SelfCall);
        let f = &seq.fragments[0];
        assert_eq!((f.first.as_str(), f.last.as_str()), ("m2", "m4"));
        assert_eq!(f.branches[0].at, "m4");
        assert_eq!(Sequence::parse(&seq.to_value()).unwrap(), seq);
        // Participants come from the messages when there's no list.
        let implicit = Sequence::parse(&json!({"messages": [
            {"from": "a", "to": "b"}, {"from": "b", "to": "c", "kind": "self"}
        ]}))
        .unwrap();
        let ids: Vec<&str> = implicit
            .participants
            .iter()
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(implicit.messages[1].kind, MessageKind::Sync);
    }

    #[test]
    fn readable_errors() {
        let errors = Sequence::parse(&json!({
            "participants": ["A", "A"],
            "messages": [{"from": "A"}, {"from": "A", "to": "B"}],
            "fragments": [{"kind": "loop"}, {"from": 9}]
        }))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "duplicate participant id \"A\"",
                "message #1 needs \"from\" and \"to\"",
                "message #2: participant \"B\" is not in participants",
                "fragment #1 needs \"from\" (a message id or 1-based number)",
                "fragment #2: message 9 does not exist",
            ]
        );
        assert_eq!(
            Sequence::parse(&json!({"participants": []})).unwrap_err(),
            vec!["\"messages\" must be an array"]
        );
    }

    #[test]
    fn derived_activations_follow_calls_and_returns() {
        let seq = sample();
        // api from POST until its 201 return; db from insert to its return.
        assert_eq!(seq.bars(), vec![(1, 0, 4), (2, 1, 2)]);
        let l = layout(&seq);
        assert_eq!(l.heads.len(), 3);
        assert_eq!(l.bars.len(), 2);
        // Time runs down; the self call loops on its own lane.
        assert!(l.messages.windows(2).all(|w| w[0].from.y < w[1].from.y));
        assert!(l.messages[3].self_loop);
        // Arrows stop at the activation bars.
        assert_eq!(l.messages[1].from.x, lane_center(1) + BAR_W / 2.);
        assert_eq!(l.messages[1].to.x, lane_center(2) - BAR_W / 2.);
        // The alt box holds its messages and the else divider sits inside.
        let alt = &l.fragments[0];
        assert!(alt.rect.y < l.messages[1].row.y);
        assert!(alt.rect.bottom() > l.messages[3].row.bottom());
        let (divider, label) = &alt.branches[0];
        assert!(*divider > l.messages[2].row.bottom() && *divider < l.messages[3].row.y);
        assert_eq!(label.as_deref(), Some("invalid"));
        assert_eq!(l.lane_at(lane_center(2) + 30.), 2);
        assert_eq!(l.insert_at(l.messages[2].from.y + 1., None), 3);
    }

    #[test]
    fn edits_describe_and_round_trip() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        let order = Sequence::moved(base.participants.iter().map(|p| p.id.as_str()), "db", 0);
        edits
            .edit(&Change::OrderParticipants {
                participants: order,
            })
            .unwrap();
        let message = Message::new(base.fresh_message_id(), "user", "api", "GET /orders/1");
        edits
            .edit(&Change::AddMessage { message, index: 5 })
            .unwrap();
        edits
            .edit(&Change::update_message(
                "m2",
                None,
                Some(MessageKind::Async),
                None,
            ))
            .unwrap();
        let order = Sequence::moved(
            edits.working().messages.iter().map(|m| m.id.as_str()),
            "m6",
            0,
        );
        edits
            .edit(&Change::OrderMessages { messages: order })
            .unwrap();
        edits
            .edit(&Change::UpdateParticipant {
                participant: "api".into(),
                label: Some("Orders API".into()),
                kind: None,
            })
            .unwrap();
        let (values, lines) = edits.summarize();
        assert_eq!(values[0]["op"], "order_participants");
        assert_eq!(
            lines,
            vec![
                "reordered the participants: Orders DB, User, API",
                "added message “GET /orders/1” from User to API (step 1)",
                "renamed participant “API” to “Orders API”",
                "made “insert” async",
            ]
        );
        round_trip(&base, edits.working());
    }

    #[test]
    fn removing_messages_shrinks_fragments_and_round_trips() {
        let base = sample();
        let mut seq = base.clone();
        // The alt's first message, then its else divider's message.
        seq.apply(&Change::RemoveMessage {
            message: "m2".into(),
        })
        .unwrap();
        assert_eq!(seq.fragments[0].first, "m3");
        seq.apply(&Change::RemoveMessage {
            message: "m4".into(),
        })
        .unwrap();
        assert_eq!(
            (
                seq.fragments[0].first.as_str(),
                seq.fragments[0].last.as_str()
            ),
            ("m3", "m3")
        );
        assert!(seq.fragments[0].branches.is_empty());
        round_trip(&base, &seq);
        seq.apply(&Change::RemoveMessage {
            message: "m3".into(),
        })
        .unwrap();
        assert!(seq.fragments.is_empty());
        round_trip(&base, &seq);
        // A participant goes with its messages.
        let mut seq = base.clone();
        seq.apply(&Change::RemoveParticipant {
            participant: "db".into(),
        })
        .unwrap();
        assert_eq!(seq.messages.len(), 3);
        round_trip(&base, &seq);
        // Reorder after a removal still replays exactly.
        let order = Sequence::moved(seq.messages.iter().map(|m| m.id.as_str()), "m1", 2);
        seq.apply(&Change::OrderMessages { messages: order })
            .unwrap();
        round_trip(&base, &seq);
    }

    #[test]
    fn self_calls_stay_on_one_lane() {
        let mut seq = sample();
        let refused = seq.apply(&Change::update_message(
            "m1",
            None,
            Some(MessageKind::SelfCall),
            None,
        ));
        assert!(refused.is_err());
        seq.apply(&Change::update_message(
            "m1",
            None,
            None,
            Some("user".into()),
        ))
        .unwrap();
        assert_eq!(seq.messages[0].kind, MessageKind::SelfCall);
    }
}
