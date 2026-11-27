//! The editing model the native kinds share (table, form, checklist,
//! kanban): the agent's spec as the base, the user's local edits on top,
//! undo, the change list sent back, and replaying unsent edits when the
//! agent sends a newer version. Each kind supplies its changes through
//! [`Editable`].

use serde_json::Value;

/// Undo keeps this many steps.
const UNDO_DEPTH: usize = 100;

/// A spec the user edits through discrete changes.
pub trait Editable: Clone + PartialEq {
    type Change: Clone + std::fmt::Debug + PartialEq;

    /// Changes turning `old` into `new`, in an order [`Editable::apply`] accepts.
    fn diff(old: &Self, new: &Self) -> Vec<Self::Change>;
    /// Apply one change; on error `self` is unchanged.
    fn apply(&mut self, change: &Self::Change) -> Result<(), String>;
    /// The change as the JSON the agent reads (`{"op": …}`).
    fn change_value(change: &Self::Change) -> Value;
    /// One sentence about `change`, with labels rather than ids.
    fn describe(change: &Self::Change, before: &Self, after: &Self) -> String;
    /// The spec as the wire JSON.
    fn to_value(&self) -> Value;
}

#[derive(Debug, Clone)]
pub struct Edits<S> {
    base: S,
    working: S,
    undo: Vec<S>,
    /// The working copy last sent, until the agent's answer about it.
    sent: Option<S>,
}

impl<S: Editable> Edits<S> {
    pub fn new(base: S) -> Self {
        Self {
            working: base.clone(),
            base,
            undo: Vec::new(),
            sent: None,
        }
    }

    /// The agent's latest version.
    pub fn base(&self) -> &S {
        &self.base
    }

    /// The base plus the user's unsent edits.
    pub fn working(&self) -> &S {
        &self.working
    }

    pub fn changes(&self) -> Vec<S::Change> {
        S::diff(&self.base, &self.working)
    }

    pub fn pending(&self) -> usize {
        self.changes().len()
    }

    /// Apply one user edit; a refused one leaves everything as it was.
    pub fn edit(&mut self, change: &S::Change) -> Result<(), String> {
        let mut next = self.working.clone();
        next.apply(change)?;
        if next != self.working {
            self.remember();
            self.working = next;
        }
        Ok(())
    }

    fn remember(&mut self) {
        self.undo.push(self.working.clone());
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                self.working = previous;
                true
            }
            None => false,
        }
    }

    /// Back to the base (one undo step).
    pub fn discard(&mut self) {
        if self.working != self.base {
            self.remember();
            self.working = self.base.clone();
        }
    }

    /// A newer version from the agent: the unsent edits that still apply
    /// are replayed on top of it; undo starts over, and an answer about an
    /// earlier send no longer applies.
    pub fn rebase(&mut self, base: S) {
        let pending = self.changes();
        let mut working = base.clone();
        for change in &pending {
            // An edit the new version makes impossible is dropped.
            let _ = working.apply(change);
        }
        self.base = base;
        self.working = working;
        self.undo.clear();
        self.sent = None;
    }

    /// The working copy is going to the agent: remember it for its answer.
    pub fn mark_sent(&mut self) {
        self.sent = Some(self.working.clone());
    }

    /// The agent took what was sent: it becomes the base, and edits made
    /// since stay pending (undo keeps working on the working copy).
    pub fn accept_sent(&mut self) {
        if let Some(sent) = self.sent.take() {
            self.base = sent;
        }
    }

    /// The pending changes as JSON and one sentence each.
    pub fn summarize(&self) -> (Vec<Value>, Vec<String>) {
        let mut state = self.base.clone();
        let mut values = Vec::new();
        let mut lines = Vec::new();
        for change in self.changes() {
            let before = state.clone();
            if state.apply(&change).is_err() {
                continue;
            }
            values.push(S::change_value(&change));
            lines.push(S::describe(&change, &before, &state));
        }
        (values, lines)
    }

    /// `/gentle-ui:event {json}` for the pending changes.
    pub fn event_command(&self, id: &str, version: u64, note: Option<&str>) -> String {
        let (changes, summary) = self.summarize();
        super::event_command_json(
            id,
            version,
            &changes,
            &summary,
            &self.working.to_value(),
            note,
        )
    }
}

/// A slug of `base` ("Write tests" → "write-tests", `fallback` when it has
/// no letters), then "-2", "-3"… until `taken` says no.
pub fn fresh_id(base: &str, fallback: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut slug = String::new();
    for c in base.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '_' {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug: String = if slug.is_empty() {
        fallback.into()
    } else {
        slug.chars().take(super::diagram::MAX_ID - 4).collect()
    };
    if !taken(&slug) {
        return slug;
    }
    (2..)
        .map(|n| format!("{slug}-{n}"))
        .find(|id| !taken(id))
        .expect("an unused suffix exists")
}

/// A finite JSON number, or a numeric string ("1,200" included).
pub fn number(value: &Value) -> Option<f64> {
    let n = match value {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().replace(',', "").parse().ok()?,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

/// `n` without a trailing ".0" (2 → "2", 2.5 → "2.5").
pub fn format_number(n: f64) -> String {
    if n.fract() == 0. && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn count_in(
    language: crate::i18n::Language,
    count: usize,
    one: &'static str,
    many: &'static str,
) -> String {
    crate::i18n::tr(language, if count == 1 { one } else { many })
        .replace("{count}", &count.to_string())
}

pub fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

/// Up to `max` names, "a, b and c" / "a, b and 3 more".
pub fn list_names(names: &[String], max: usize) -> String {
    let shown: Vec<&str> = names.iter().take(max).map(String::as_str).collect();
    let more = names.len().saturating_sub(max);
    match (shown.as_slice(), more) {
        ([], _) => String::new(),
        ([one], 0) => (*one).to_string(),
        (all, 0) => format!(
            "{} and {}",
            all[..all.len() - 1].join(", "),
            all[all.len() - 1]
        ),
        (all, more) => format!("{} and {more} more", all.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A list of words edited by replacing one at a time.
    #[derive(Debug, Clone, PartialEq)]
    struct Words(Vec<String>);

    #[derive(Debug, Clone, PartialEq)]
    struct Set(usize, String);

    impl Editable for Words {
        type Change = Set;
        fn diff(old: &Self, new: &Self) -> Vec<Set> {
            old.0
                .iter()
                .zip(&new.0)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (_, b))| Set(i, b.clone()))
                .collect()
        }
        fn apply(&mut self, change: &Set) -> Result<(), String> {
            let slot = self.0.get_mut(change.0).ok_or("no such word")?;
            *slot = change.1.clone();
            Ok(())
        }
        fn change_value(change: &Set) -> Value {
            json!({"op": "set", "at": change.0, "word": change.1})
        }
        fn describe(change: &Set, before: &Self, _: &Self) -> String {
            format!("{} → {}", before.0[change.0], change.1)
        }
        fn to_value(&self) -> Value {
            json!(self.0)
        }
    }

    fn words(list: &[&str]) -> Words {
        Words(list.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn edits_count_undo_discard_and_rebase() {
        let mut edits = Edits::new(words(&["a", "b", "c"]));
        assert_eq!(edits.pending(), 0);
        edits.edit(&Set(0, "x".into())).unwrap();
        edits.edit(&Set(2, "z".into())).unwrap();
        assert_eq!(edits.pending(), 2);
        assert!(edits.edit(&Set(9, "?".into())).is_err());
        assert_eq!(edits.working(), &words(&["x", "b", "z"]));

        assert!(edits.undo());
        assert_eq!(edits.working(), &words(&["x", "b", "c"]));
        edits.discard();
        assert_eq!(edits.pending(), 0);
        assert!(edits.undo(), "discard is one undo step");
        assert_eq!(edits.pending(), 1);

        // The agent's new version keeps the user's edit and drops undo.
        edits.rebase(words(&["a", "B", "c"]));
        assert_eq!(edits.working(), &words(&["x", "B", "c"]));
        assert!(!edits.can_undo());
        // An edit the new version can't take is dropped.
        edits.rebase(words(&[]));
        assert_eq!(edits.pending(), 0);
    }

    #[test]
    fn an_accepted_send_becomes_the_base_and_later_edits_stay_pending() {
        let mut edits = Edits::new(words(&["a", "b"]));
        edits.edit(&Set(0, "x".into())).unwrap();
        edits.mark_sent();
        // Edited again while the answer is on its way.
        edits.edit(&Set(1, "y".into())).unwrap();
        edits.accept_sent();
        assert_eq!(edits.base(), &words(&["x", "b"]));
        assert_eq!(edits.changes(), vec![Set(1, "y".into())]);
        // Only once: a second answer changes nothing.
        edits.accept_sent();
        assert_eq!(edits.base(), &words(&["x", "b"]));
    }

    #[test]
    fn summarize_describes_against_the_state_before_each_change() {
        let mut edits = Edits::new(words(&["a", "b"]));
        edits.edit(&Set(1, "y".into())).unwrap();
        let (values, lines) = edits.summarize();
        assert_eq!(values, vec![json!({"op": "set", "at": 1, "word": "y"})]);
        assert_eq!(lines, vec!["b → y".to_string()]);
        let cmd = edits.event_command("ui-1", 4, Some("thanks"));
        let value: Value =
            serde_json::from_str(cmd.strip_prefix("/gentle-ui:event ").unwrap()).unwrap();
        assert_eq!(value["spec"], json!(["a", "y"]));
        assert_eq!(value["version"], 4);
        assert_eq!(value["note"], "thanks");
    }

    #[test]
    fn helpers() {
        let taken = |id: &str| id == "write-tests";
        assert_eq!(fresh_id("Write tests!", "item", taken), "write-tests-2");
        assert_eq!(fresh_id("  ", "item", |_| false), "item");
        assert_eq!(number(&json!("1,200")), Some(1200.));
        assert_eq!(number(&json!("abc")), None);
        assert_eq!(format_number(2.), "2");
        assert_eq!(format_number(2.50), "2.5");
        assert_eq!(format_number(1. / 3.), "0.3333");
        let names: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
        assert_eq!(list_names(&names[..1], 3), "a");
        assert_eq!(list_names(&names[..3], 3), "a, b and c");
        assert_eq!(list_names(&names, 2), "a, b and 2 more");
    }
}
