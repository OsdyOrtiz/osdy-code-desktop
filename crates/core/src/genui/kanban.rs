//! The kanban kind: columns of cards the user drags between and within
//! columns, adds, edits and removes.

use serde_json::{Map, Value, json};

use super::diagram::{Errors, MAX_LABEL, MAX_TEXT, array, id_of, text, truncate};
use super::edits::{Editable, fresh_id, list_names};
use super::refs::CodeRef;

pub const MAX_COLUMNS: usize = 12;
pub const MAX_CARDS: usize = 300;
pub const MAX_TAGS: usize = 8;
pub const MAX_TAG: usize = 40;
pub const MAX_TITLE: usize = 120;

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub id: String,
    pub label: String,
    /// Work-in-progress limit: more cards are allowed but flagged.
    pub limit: Option<u32>,
    /// A card dropped here asks the agent to dispatch a subagent for it.
    pub starts: bool,
}

/// Who works a card: the subagent the agent should dispatch for it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CardAgent {
    /// The subagent to use (the agent picks one when absent).
    pub name: Option<String>,
    /// What to tell it (default: the card's title and description).
    pub task: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub column: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub assignee: Option<String>,
    /// The code this card is about (`ref`).
    pub code_ref: Option<CodeRef>,
    pub agent: Option<CardAgent>,
    /// The dispatched subagent's id, as the agent reported it: the card
    /// follows that subagent's live status.
    pub agent_id: Option<String>,
}

impl Card {
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("title".into(), json!(self.title));
        map.insert("column".into(), json!(self.column));
        if let Some(description) = &self.description {
            map.insert("description".into(), json!(description));
        }
        if !self.tags.is_empty() {
            map.insert("tags".into(), json!(self.tags));
        }
        if let Some(assignee) = &self.assignee {
            map.insert("assignee".into(), json!(assignee));
        }
        if let Some(code_ref) = &self.code_ref {
            map.insert("ref".into(), code_ref.to_value());
        }
        if let Some(agent) = &self.agent {
            let mut a = Map::new();
            if let Some(name) = &agent.name {
                a.insert("name".into(), json!(name));
            }
            if let Some(task) = &agent.task {
                a.insert("task".into(), json!(task));
            }
            map.insert("agent".into(), Value::Object(a));
        }
        if let Some(id) = &self.agent_id {
            map.insert("agentId".into(), json!(id));
        }
        Value::Object(map)
    }
}

/// Cards are kept grouped by column, in column order; within a column,
/// their order is the board's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Board {
    pub columns: Vec<Column>,
    pub cards: Vec<Card>,
}

/// `agent: {name?, task?}` (a string is the name); `None` when it names nothing.
fn card_agent(raw: Option<&Value>) -> Option<CardAgent> {
    let agent = match raw? {
        Value::String(_) => CardAgent {
            name: text(raw, MAX_LABEL),
            task: None,
        },
        Value::Object(o) => CardAgent {
            name: text(o.get("name"), MAX_LABEL),
            task: text(o.get("task"), MAX_TEXT),
        },
        _ => return None,
    };
    (agent != CardAgent::default()).then_some(agent)
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = truncate(tag.trim(), MAX_TAG);
        if !tag.is_empty() && !out.contains(&tag) && out.len() < MAX_TAGS {
            out.push(tag);
        }
    }
    out
}

impl Board {
    pub fn parse(spec: &Value) -> Result<Board, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(columns_raw) = spec.get("columns").and_then(Value::as_array) else {
            return Err(vec!["\"columns\" must be an array".into()]);
        };
        let cards_raw = match spec.get("cards") {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(cards)) => cards.as_slice(),
            Some(_) => return Err(vec!["\"cards\" must be an array".into()]),
        };
        let mut errors = Errors::default();
        if columns_raw.is_empty() {
            errors.push("a board needs at least one column".into());
        }
        if columns_raw.len() > MAX_COLUMNS {
            errors.push(format!(
                "too many columns ({}, at most {MAX_COLUMNS})",
                columns_raw.len()
            ));
        }
        if cards_raw.len() > MAX_CARDS {
            errors.push(format!(
                "too many cards ({}, at most {MAX_CARDS})",
                cards_raw.len()
            ));
        }
        let mut columns: Vec<Column> = Vec::new();
        for (i, raw) in columns_raw.iter().enumerate().take(MAX_COLUMNS) {
            // "columns": ["To do", "Done"] is a shorthand for labels.
            let (id, label) = match raw {
                Value::String(s) => (
                    Some(fresh_id(s, "column", |id| {
                        columns.iter().any(|c| c.id == id)
                    })),
                    text(Some(raw), MAX_LABEL),
                ),
                _ => (
                    raw.get("id").and_then(id_of),
                    text(raw.get("label"), MAX_LABEL),
                ),
            };
            let Some(id) = id else {
                errors.push(format!("column #{} has no id", i + 1));
                continue;
            };
            if columns.iter().any(|c| c.id == id) {
                errors.push(format!("duplicate column id \"{id}\""));
                continue;
            }
            columns.push(Column {
                label: label.unwrap_or_else(|| id.clone()),
                limit: raw
                    .get("limit")
                    .and_then(Value::as_u64)
                    .filter(|l| *l > 0)
                    .map(|l| l.min(999) as u32),
                starts: raw.get("starts").and_then(Value::as_bool) == Some(true),
                id,
            });
        }
        let mut cards: Vec<Card> = Vec::new();
        for (i, raw) in cards_raw.iter().enumerate().take(MAX_CARDS) {
            let title = text(raw.get("title").or_else(|| raw.get("label")), MAX_TITLE);
            let id = raw.get("id").and_then(id_of);
            let Some(title) = title.or_else(|| id.clone()) else {
                errors.push(format!("card #{} has no title", i + 1));
                continue;
            };
            let id = id
                .unwrap_or_else(|| fresh_id(&title, "card", |id| cards.iter().any(|c| c.id == id)));
            if cards.iter().any(|c| c.id == id) {
                errors.push(format!("duplicate card id \"{id}\""));
                continue;
            }
            let column = match raw.get("column").and_then(id_of) {
                Some(column) if columns.iter().any(|c| c.id == column) => column,
                Some(column) => {
                    errors.push(format!(
                        "card \"{id}\" is in column \"{column}\", which is not in columns"
                    ));
                    continue;
                }
                // No column: the first one.
                None => match columns.first() {
                    Some(first) => first.id.clone(),
                    None => continue,
                },
            };
            let tags: Vec<String> = array(raw.get("tags"))
                .iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect();
            let code_ref = CodeRef::field(raw).unwrap_or_else(|error| {
                errors.push(format!("card \"{id}\": ref {error}"));
                None
            });
            cards.push(Card {
                id,
                title,
                column,
                description: text(raw.get("description"), MAX_TEXT),
                tags: clean_tags(&tags),
                assignee: text(raw.get("assignee"), MAX_LABEL),
                code_ref,
                agent: card_agent(raw.get("agent")),
                agent_id: raw.get("agentId").and_then(id_of),
            });
        }
        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        let mut board = Board { columns, cards };
        board.group();
        Ok(board)
    }

    /// Cards grouped by column in column order (stable within a column).
    fn group(&mut self) {
        let columns = &self.columns;
        self.cards
            .sort_by_key(|card| columns.iter().position(|c| c.id == card.column));
    }

    /// Cards `working` has in a `starts` column that `base` had elsewhere
    /// (or didn't have) and that have no subagent yet: what dropping them
    /// there asks the agent to dispatch.
    pub fn dispatches(base: &Board, working: &Board) -> Vec<String> {
        let starts = |board: &Board, column: &str| board.column(column).is_some_and(|c| c.starts);
        working
            .cards
            .iter()
            .filter(|card| card.agent_id.is_none() && starts(working, &card.column))
            .filter(|card| {
                base.card(&card.id)
                    .is_none_or(|before| !starts(base, &before.column))
            })
            .map(|card| card.id.clone())
            .collect()
    }

    pub fn column(&self, id: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.id == id)
    }

    pub fn card(&self, id: &str) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// Ids of `column`'s cards, top to bottom.
    pub fn ids(&self, column: &str) -> Vec<String> {
        self.cards
            .iter()
            .filter(|c| c.column == column)
            .map(|c| c.id.clone())
            .collect()
    }

    pub fn fresh_id(&self, title: &str) -> String {
        fresh_id(title, "card", |id| self.card(id).is_some())
    }

    /// `column`'s order with `card` dropped at `index` (an index among the
    /// column's other cards): what a drop sends as [`Change::Order`].
    pub fn dropped(&self, card: &str, column: &str, index: usize) -> Vec<String> {
        let mut ids: Vec<String> = self
            .ids(column)
            .into_iter()
            .filter(|id| id != card)
            .collect();
        ids.insert(index.min(ids.len()), card.to_string());
        ids
    }

    fn label_of(&self, column: &str) -> String {
        self.column(column)
            .map_or_else(|| column.to_string(), |c| c.label.clone())
    }
}

/// Where a card dragged over a column lands: the index among the column's
/// other cards whose middle is below `y` (card tops and heights, in the
/// column's coordinates, without the dragged card).
pub fn drop_index(y: f32, slots: &[(f32, f32)]) -> usize {
    slots
        .iter()
        .position(|(top, height)| y < top + height / 2.)
        .unwrap_or(slots.len())
}

/// `None` = unchanged; a cleared optional is `Some(None)`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CardPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub assignee: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Appended at the bottom of its column.
    AddCard {
        card: Card,
    },
    UpdateCard {
        card: String,
        patch: CardPatch,
    },
    RemoveCard {
        card: String,
    },
    /// `column`'s cards are now `cards`, top to bottom (cards from other
    /// columns move into it); cards it held that aren't listed stay below.
    Order {
        column: String,
        cards: Vec<String>,
    },
}

impl Editable for Board {
    type Change = Change;

    /// Adds, orders (by column), updates, removes.
    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut sim = old.clone();
        for card in &new.cards {
            if old.card(&card.id).is_none() {
                let change = Change::AddCard { card: card.clone() };
                let _ = sim.apply(&change);
                changes.push(change);
            }
        }
        for column in &new.columns {
            let wanted = new.ids(&column.id);
            let now: Vec<String> = sim
                .ids(&column.id)
                .into_iter()
                .filter(|id| new.card(id).is_some_and(|c| c.column == column.id))
                .collect();
            if now != wanted {
                let change = Change::Order {
                    column: column.id.clone(),
                    cards: wanted,
                };
                let _ = sim.apply(&change);
                changes.push(change);
            }
        }
        for card in &new.cards {
            let Some(o) = old.card(&card.id) else {
                continue;
            };
            let patch = CardPatch {
                title: (o.title != card.title).then(|| card.title.clone()),
                description: (o.description != card.description).then(|| card.description.clone()),
                tags: (o.tags != card.tags).then(|| card.tags.clone()),
                assignee: (o.assignee != card.assignee).then(|| card.assignee.clone()),
            };
            if patch != CardPatch::default() {
                changes.push(Change::UpdateCard {
                    card: card.id.clone(),
                    patch,
                });
            }
        }
        for card in &old.cards {
            if new.card(&card.id).is_none() {
                changes.push(Change::RemoveCard {
                    card: card.id.clone(),
                });
            }
        }
        changes
    }

    fn apply(&mut self, change: &Change) -> Result<(), String> {
        let unknown = |id: &str| format!("unknown card \"{id}\"");
        match change {
            Change::AddCard { card } => {
                if card.id.trim().is_empty() || card.title.trim().is_empty() {
                    return Err("a new card needs an id and a title".into());
                }
                if self.card(&card.id).is_some() {
                    return Err(format!("card \"{}\" already exists", card.id));
                }
                if self.column(&card.column).is_none() {
                    return Err(format!("unknown column \"{}\"", card.column));
                }
                if self.cards.len() >= MAX_CARDS {
                    return Err(format!("too many cards (at most {MAX_CARDS})"));
                }
                let mut card = card.clone();
                card.title = truncate(card.title.trim(), MAX_TITLE);
                card.tags = clean_tags(&card.tags);
                // After the column's last card.
                let at = self
                    .cards
                    .iter()
                    .rposition(|c| c.column == card.column)
                    .map(|i| i + 1)
                    .unwrap_or_else(|| {
                        let before = self.columns.iter().position(|c| c.id == card.column);
                        self.cards
                            .iter()
                            .position(|c| {
                                self.columns.iter().position(|col| col.id == c.column) > before
                            })
                            .unwrap_or(self.cards.len())
                    });
                self.cards.insert(at, card);
            }
            Change::UpdateCard { card, patch } => {
                let c = self
                    .cards
                    .iter_mut()
                    .find(|c| &c.id == card)
                    .ok_or_else(|| unknown(card))?;
                if let Some(title) = &patch.title {
                    let title = title.trim();
                    if title.is_empty() {
                        return Err("a card needs a title".into());
                    }
                    c.title = truncate(title, MAX_TITLE);
                }
                let clean = |s: &Option<String>, max| {
                    s.as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| truncate(s, max))
                };
                if let Some(description) = &patch.description {
                    c.description = clean(description, MAX_TEXT);
                }
                if let Some(tags) = &patch.tags {
                    c.tags = clean_tags(tags);
                }
                if let Some(assignee) = &patch.assignee {
                    c.assignee = clean(assignee, MAX_LABEL);
                }
            }
            Change::RemoveCard { card } => {
                let i = self
                    .cards
                    .iter()
                    .position(|c| &c.id == card)
                    .ok_or_else(|| unknown(card))?;
                self.cards.remove(i);
            }
            Change::Order { column, cards } => {
                if self.column(column).is_none() {
                    return Err(format!("unknown column \"{column}\""));
                }
                if let Some(missing) = cards.iter().find(|id| self.card(id).is_none()) {
                    return Err(unknown(missing));
                }
                let mut moved: Vec<Card> = Vec::new();
                for id in cards {
                    if let Some(i) = self.cards.iter().position(|c| &c.id == id) {
                        let mut card = self.cards.remove(i);
                        card.column = column.clone();
                        moved.push(card);
                    }
                }
                // Listed first, then the column's others, at its place.
                let at = self
                    .cards
                    .iter()
                    .position(|c| &c.column == column)
                    .unwrap_or(self.cards.len());
                for (offset, card) in moved.into_iter().enumerate() {
                    self.cards.insert(at + offset, card);
                }
                self.group();
            }
        }
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        match change {
            Change::AddCard { card } => json!({"op": "add_card", "card": card.to_value()}),
            Change::UpdateCard { card, patch } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_card"));
                map.insert("card".into(), json!(card));
                if let Some(title) = &patch.title {
                    map.insert("title".into(), json!(title));
                }
                if let Some(description) = &patch.description {
                    map.insert("description".into(), json!(description));
                }
                if let Some(tags) = &patch.tags {
                    map.insert("tags".into(), json!(tags));
                }
                if let Some(assignee) = &patch.assignee {
                    map.insert("assignee".into(), json!(assignee));
                }
                Value::Object(map)
            }
            Change::RemoveCard { card } => json!({"op": "remove_card", "card": card}),
            Change::Order { column, cards } => {
                json!({"op": "order", "column": column, "cards": cards})
            }
        }
    }

    fn describe(change: &Change, before: &Self, after: &Self) -> String {
        let name = |id: &str| {
            after.card(id).or_else(|| before.card(id)).map_or_else(
                || id.to_string(),
                |c| format!("“{}”", truncate(&c.title, 60)),
            )
        };
        match change {
            Change::AddCard { card } => {
                format!(
                    "added {} to {}",
                    name(&card.id),
                    after.label_of(&card.column)
                )
            }
            Change::RemoveCard { card } => format!("removed {}", name(card)),
            Change::UpdateCard { card, patch } => {
                let mut parts = Vec::new();
                if let Some(title) = &patch.title {
                    let old = before.card(card).map_or(card.as_str(), |c| &c.title);
                    parts.push(format!("renamed “{old}” to “{title}”"));
                }
                let n = name(card);
                match &patch.description {
                    Some(Some(_)) => parts.push(format!("updated the description of {n}")),
                    Some(None) => parts.push(format!("cleared the description of {n}")),
                    None => {}
                }
                match &patch.tags {
                    Some(tags) if tags.is_empty() => parts.push(format!("cleared the tags of {n}")),
                    Some(tags) => parts.push(format!("tagged {n} {}", tags.join(", "))),
                    None => {}
                }
                match &patch.assignee {
                    Some(Some(who)) => parts.push(format!("assigned {n} to {who}")),
                    Some(None) => parts.push(format!("unassigned {n}")),
                    None => {}
                }
                parts.join(", ")
            }
            Change::Order { column, cards } => {
                let target = after.label_of(column);
                let moved: Vec<String> = cards
                    .iter()
                    .filter_map(|id| {
                        let from = &before.card(id)?.column;
                        (from != column)
                            .then(|| format!("{} from {}", name(id), before.label_of(from)))
                    })
                    .collect();
                if moved.is_empty() {
                    format!("reordered {target}")
                } else {
                    format!("moved {} to {target}", list_names(&moved, 5))
                }
            }
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "columns": self.columns.iter().map(|c| {
                let mut map = Map::new();
                map.insert("id".into(), json!(c.id));
                map.insert("label".into(), json!(c.label));
                if let Some(limit) = c.limit {
                    map.insert("limit".into(), json!(limit));
                }
                if c.starts {
                    map.insert("starts".into(), json!(true));
                }
                Value::Object(map)
            }).collect::<Vec<_>>(),
            "cards": self.cards.iter().map(Card::to_value).collect::<Vec<_>>(),
        })
    }
}

/// "5 cards in 3 columns".
pub fn card_count(board: &Board) -> String {
    card_count_in(board, crate::i18n::Language::English)
}

pub fn card_count_in(board: &Board, language: crate::i18n::Language) -> String {
    use super::edits::count_in;
    crate::i18n::tr(language, "{cards} in {columns}")
        .replace(
            "{cards}",
            &count_in(language, board.cards.len(), "{count} card", "{count} cards"),
        )
        .replace(
            "{columns}",
            &count_in(
                language,
                board.columns.len(),
                "{count} column",
                "{count} columns",
            ),
        )
}

/// `{op: "dispatch", card, column, agent?}` and its sentence: the change
/// an event adds for each card dropped into a `starts` column (the agent
/// dispatches a subagent with its own tool; the app can't start one).
pub fn dispatch(board: &Board, card: &str) -> Option<(Value, String)> {
    let c = board.card(card)?;
    let mut map = Map::new();
    map.insert("op".into(), json!("dispatch"));
    map.insert("card".into(), json!(c.id));
    map.insert("column".into(), json!(c.column));
    if c.agent.is_some() {
        map.insert("agent".into(), c.to_value()["agent"].clone());
    }
    let who = c
        .agent
        .as_ref()
        .and_then(|a| a.name.as_deref())
        .map_or_else(
            || "a subagent".to_string(),
            |name| format!("the {name} subagent"),
        );
    let line = format!(
        "asked the agent to dispatch {who} for “{}” ({})",
        truncate(&c.title, 60),
        board.label_of(&c.column)
    );
    Some((Value::Object(map), line))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn sample() -> Board {
        Board::parse(&json!({
            "columns": [{"id": "todo", "label": "To do"}, {"id": "doing", "label": "Doing", "limit": 2}, "Done"],
            "cards": [
                {"id": "c1", "title": "Login", "column": "doing", "tags": ["auth", " auth ", ""]},
                {"id": "c2", "title": "Signup", "column": "todo", "assignee": "Ada"},
                {"title": "Docs"},
                {"id": "c4", "title": "Deploy", "column": "done"}
            ]
        }))
        .unwrap()
    }

    fn round_trip(old: &Board, new: &Board) {
        let mut replay = old.clone();
        for change in Board::diff(old, new) {
            replay
                .apply(&change)
                .unwrap_or_else(|e| panic!("{change:?}: {e}"));
        }
        assert_eq!(&replay, new);
    }

    #[test]
    fn parses_and_groups_cards_by_column() {
        let b = sample();
        assert_eq!(b.columns[2].id, "done");
        assert_eq!(b.columns[1].limit, Some(2));
        assert_eq!(b.ids("todo"), vec!["c2", "docs"]);
        let order: Vec<&str> = b.cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(order, vec!["c2", "docs", "c1", "c4"]);
        assert_eq!(b.card("c1").unwrap().tags, vec!["auth"]);
        assert_eq!(Board::parse(&b.to_value()).unwrap(), b);
        assert_eq!(card_count(&b), "4 cards in 3 columns");
    }

    #[test]
    fn readable_errors() {
        let errors = Board::parse(&json!({
            "columns": [{"id": "a"}, {"id": "a"}],
            "cards": [{"id": "x", "column": "zz"}, {"column": "a"}, {"id": "x"}, {"id": "x"}]
        }))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "duplicate column id \"a\"",
                "card \"x\" is in column \"zz\", which is not in columns",
                "card #2 has no title",
                "duplicate card id \"x\"",
            ]
        );
    }

    #[test]
    fn drops_reorder_and_move_between_columns() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        // Drag "Login" to the top of Done, then "Docs" above "Signup".
        let order = edits.working().dropped("c1", "done", 0);
        edits
            .edit(&Change::Order {
                column: "done".into(),
                cards: order,
            })
            .unwrap();
        let order = edits.working().dropped("docs", "todo", 0);
        edits
            .edit(&Change::Order {
                column: "todo".into(),
                cards: order,
            })
            .unwrap();
        let card = Card {
            id: base.fresh_id("Write tests"),
            title: "Write tests".into(),
            column: "doing".into(),
            description: None,
            tags: vec![],
            assignee: None,
            code_ref: None,
            agent: None,
            agent_id: None,
        };
        edits.edit(&Change::AddCard { card }).unwrap();
        edits
            .edit(&Change::UpdateCard {
                card: "c2".into(),
                patch: CardPatch {
                    title: Some("Sign up".into()),
                    assignee: Some(None),
                    tags: Some(vec!["ux".into()]),
                    ..CardPatch::default()
                },
            })
            .unwrap();
        edits
            .edit(&Change::RemoveCard { card: "c4".into() })
            .unwrap();
        let w = edits.working();
        assert_eq!(w.ids("todo"), vec!["docs", "c2"]);
        assert_eq!(w.ids("doing"), vec!["write-tests"]);
        assert_eq!(w.ids("done"), vec!["c1"]);
        let (_, lines) = edits.summarize();
        assert_eq!(
            lines,
            vec![
                "added “Write tests” to Doing",
                "reordered To do",
                "moved “Login” from Doing to Done",
                "renamed “Signup” to “Sign up”, tagged “Sign up” ux, unassigned “Sign up”",
                "removed “Deploy”",
            ]
        );
        round_trip(&base, edits.working());
    }

    #[test]
    fn drop_index_uses_card_middles() {
        let slots = [(0., 80.), (88., 80.), (176., 40.)];
        assert_eq!(drop_index(-5., &slots), 0);
        assert_eq!(drop_index(39., &slots), 0);
        assert_eq!(drop_index(41., &slots), 1);
        assert_eq!(drop_index(190., &slots), 2);
        assert_eq!(drop_index(500., &slots), 3);
        assert_eq!(drop_index(0., &[]), 0);
    }

    #[test]
    fn cards_keep_their_code_ref() {
        let board = Board::parse(&json!({
            "columns": ["To do"],
            "cards": [{"id": "c1", "title": "Fix", "ref": {"path": "src/lib.rs", "line": 4, "endLine": 9}}]
        }))
        .unwrap();
        let code_ref = board.cards[0].code_ref.as_ref().unwrap();
        assert_eq!((code_ref.line, code_ref.end_line), (Some(4), Some(9)));
        assert_eq!(Board::parse(&board.to_value()).unwrap(), board);
        assert!(
            Board::parse(&json!({"columns": ["A"], "cards": [{"title": "x", "ref": "../../y"}]}))
                .is_err()
        );
    }

    #[test]
    fn a_drop_into_a_starts_column_asks_for_a_subagent() {
        let base = Board::parse(&json!({
            "columns": [
                {"id": "todo", "label": "To do"},
                {"id": "doing", "label": "In progress", "starts": true},
                {"id": "review", "label": "Review", "starts": true}
            ],
            "cards": [
                {"id": "a", "title": "Login", "agent": {"name": "worker", "task": "Build the login form"}},
                {"id": "b", "title": "Signup", "agent": "worker"},
                {"id": "c", "title": "Docs", "column": "doing", "agentId": "sa-1"},
                {"id": "d", "title": "Tests", "agent": {}}
            ]
        }))
        .unwrap();
        assert!(base.columns[1].starts && !base.columns[0].starts);
        assert_eq!(
            base.card("b")
                .unwrap()
                .agent
                .as_ref()
                .unwrap()
                .name
                .as_deref(),
            Some("worker")
        );
        assert_eq!(base.card("c").unwrap().agent_id.as_deref(), Some("sa-1"));
        assert_eq!(
            base.card("d").unwrap().agent,
            None,
            "an empty agent names nothing"
        );
        assert_eq!(Board::parse(&base.to_value()).unwrap(), base, "round trip");

        let mut working = base.clone();
        for (card, column) in [("a", "doing"), ("c", "review"), ("d", "doing")] {
            let cards = working.dropped(card, column, 0);
            working
                .apply(&Change::Order {
                    column: column.into(),
                    cards,
                })
                .unwrap();
        }
        // "c" already has a subagent; a move between starts columns isn't a start.
        assert_eq!(Board::dispatches(&base, &working), vec!["d", "a"]);
        let (value, line) = dispatch(&working, "a").unwrap();
        assert_eq!(
            value,
            json!({"op": "dispatch", "card": "a", "column": "doing", "agent": {"name": "worker", "task": "Build the login form"}})
        );
        assert_eq!(
            line,
            "asked the agent to dispatch the worker subagent for “Login” (In progress)"
        );
        assert_eq!(
            dispatch(&working, "d").unwrap().1,
            "asked the agent to dispatch a subagent for “Tests” (In progress)"
        );
        assert!(Board::dispatches(&working, &working).is_empty());
    }
}
