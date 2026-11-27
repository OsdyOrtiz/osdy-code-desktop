//! The erd kind: tables with typed columns (primary and foreign keys,
//! nullable, unique) and the relations between their columns with a
//! cardinality, laid out with the diagram's layered layout. The user moves
//! and renames tables, adds and edits columns and relates columns.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use super::diagram::{
    Diagram, Direction, Edge, EdgeStyle, Errors, MAX_LABEL, MAX_TEXT, Node, NodeKind, array, id_of,
    text, truncate, wire_enum,
};
use super::edits::{Editable, fresh_id, plural};
use super::layout::{self, Cubic, EdgePath, Point, Rect};
use super::refs::CodeRef;
use super::style::{self, Tint};

pub const MAX_TABLES: usize = 60;
pub const MAX_COLUMNS: usize = 80;
pub const MAX_RELATIONS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Cardinality {
    OneOne,
    #[default]
    OneMany,
    ManyOne,
    ManyMany,
}

wire_enum!(Cardinality, 4, [
    OneOne => "1-1",
    OneMany => "1-n",
    ManyOne => "n-1",
    ManyMany => "n-n",
]);

impl Cardinality {
    /// Whether the `from` and the `to` end are "many".
    pub fn many(self) -> (bool, bool) {
        match self {
            Cardinality::OneOne => (false, false),
            Cardinality::OneMany => (false, true),
            Cardinality::ManyOne => (true, false),
            Cardinality::ManyMany => (true, true),
        }
    }
}

/// A column of a table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct End {
    pub table: String,
    pub column: String,
}

impl End {
    pub fn new(table: &str, column: &str) -> Self {
        Self {
            table: table.into(),
            column: column.into(),
        }
    }

    fn wire(&self) -> String {
        format!("{}.{}", self.table, self.column)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub ty: Option<String>,
    pub pk: bool,
    pub fk: Option<End>,
    pub nullable: bool,
    pub unique: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub id: String,
    pub name: String,
    pub columns: Vec<Column>,
    pub note: Option<String>,
    pub code_ref: Option<CodeRef>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    /// Overrides the table's color and icon (`color`, `icon`).
    pub color: Option<Tint>,
    pub icon: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    pub id: String,
    pub from: End,
    pub to: End,
    pub cardinality: Cardinality,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Erd {
    pub tables: Vec<Table>,
    pub relations: Vec<Relation>,
}

impl Column {
    pub fn new(name: impl Into<String>, ty: Option<String>) -> Self {
        Self {
            name: name.into(),
            ty,
            pk: false,
            fk: None,
            nullable: false,
            unique: false,
            note: None,
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("name".into(), json!(self.name));
        if let Some(ty) = &self.ty {
            map.insert("type".into(), json!(ty));
        }
        for (key, on) in [
            ("pk", self.pk),
            ("nullable", self.nullable),
            ("unique", self.unique),
        ] {
            if on {
                map.insert(key.into(), json!(true));
            }
        }
        if let Some(fk) = &self.fk {
            map.insert("fk".into(), json!({"table": fk.table, "column": fk.column}));
        }
        if let Some(note) = &self.note {
            map.insert("note".into(), json!(note));
        }
        Value::Object(map)
    }
}

impl Table {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            columns: Vec::new(),
            note: None,
            code_ref: None,
            x: None,
            y: None,
            color: None,
            icon: None,
        }
    }

    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    fn column_mut(&mut self, name: &str) -> Option<&mut Column> {
        self.columns.iter_mut().find(|c| c.name == name)
    }

    pub fn fresh_column(&self, base: &str) -> String {
        if self.column(base).is_none() {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base}_{n}"))
            .find(|name| self.column(name).is_none())
            .expect("an unused suffix exists")
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("name".into(), json!(self.name));
        map.insert(
            "columns".into(),
            self.columns.iter().map(Column::to_value).collect(),
        );
        if let Some(note) = &self.note {
            map.insert("note".into(), json!(note));
        }
        if let Some(code_ref) = &self.code_ref {
            map.insert("ref".into(), code_ref.to_value());
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

impl Relation {
    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("from".into(), json!(self.from.wire()));
        map.insert("to".into(), json!(self.to.wire()));
        map.insert("cardinality".into(), json!(self.cardinality.name()));
        if let Some(label) = &self.label {
            map.insert("label".into(), json!(label));
        }
        Value::Object(map)
    }
}

impl Erd {
    pub fn parse(spec: &Value) -> Result<Erd, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(raw_tables) = spec.get("tables").and_then(Value::as_array) else {
            return Err(vec!["\"tables\" must be an array".into()]);
        };
        let raw_relations = array(spec.get("relations"));
        let mut errors = Errors::default();
        for (what, count, max) in [
            ("tables", raw_tables.len(), MAX_TABLES),
            ("relations", raw_relations.len(), MAX_RELATIONS),
        ] {
            if count > max {
                errors.push(format!("too many {what} ({count}, at most {max})"));
            }
        }
        let mut erd = Erd::default();
        // Foreign keys as written, checked once every table is known.
        let mut fks: Vec<(usize, usize, Value)> = Vec::new();
        for (i, raw) in raw_tables.iter().take(MAX_TABLES).enumerate() {
            let name = text(raw.get("name").or_else(|| raw.get("label")), MAX_LABEL);
            let Some(id) = raw.get("id").and_then(id_of).or_else(|| name.clone()) else {
                errors.push(format!("table #{} has no id", i + 1));
                continue;
            };
            if erd.table(&id).is_some() {
                errors.push(format!("duplicate table id \"{id}\""));
                continue;
            }
            let raw_columns = array(raw.get("columns"));
            if raw_columns.len() > MAX_COLUMNS {
                errors.push(format!(
                    "table \"{id}\": too many columns ({}, at most {MAX_COLUMNS})",
                    raw_columns.len()
                ));
            }
            let mut table = Table::new(id.clone(), name.unwrap_or_else(|| id.clone()));
            for (j, raw_column) in raw_columns.iter().take(MAX_COLUMNS).enumerate() {
                let name = match raw_column {
                    Value::String(_) => id_of(raw_column),
                    _ => raw_column.get("name").and_then(id_of),
                };
                let Some(name) = name else {
                    errors.push(format!("table \"{id}\": column #{} has no name", j + 1));
                    continue;
                };
                if table.column(&name).is_some() {
                    errors.push(format!("table \"{id}\": duplicate column \"{name}\""));
                    continue;
                }
                let flag = |key: &str| raw_column.get(key).and_then(Value::as_bool) == Some(true);
                if let Some(fk) = raw_column.get("fk").filter(|v| !v.is_null()) {
                    fks.push((erd.tables.len(), table.columns.len(), fk.clone()));
                }
                table.columns.push(Column {
                    ty: text(raw_column.get("type"), MAX_LABEL),
                    pk: flag("pk"),
                    fk: None,
                    nullable: flag("nullable"),
                    unique: flag("unique"),
                    note: text(raw_column.get("note"), MAX_TEXT),
                    name,
                });
            }
            table.note = text(raw.get("note"), MAX_TEXT);
            table.code_ref = match CodeRef::field(raw) {
                Ok(code_ref) => code_ref,
                Err(e) => {
                    errors.push(format!("table \"{id}\": ref {e}"));
                    None
                }
            };
            let number = |key: &str| {
                raw.get(key)
                    .and_then(Value::as_f64)
                    .map(|n| n as f32)
                    .filter(|n| n.is_finite())
            };
            if let (Some(x), Some(y)) = (number("x"), number("y")) {
                table.x = Some(x);
                table.y = Some(y);
            }
            table.color = Tint::field(raw);
            table.icon = style::icon_field(raw);
            erd.tables.push(table);
        }

        for (t, c, raw) in fks {
            let owner = format!("{}.{}", erd.tables[t].id, erd.tables[t].columns[c].name);
            match erd.read_end(&raw, true) {
                Ok(end) => erd.tables[t].columns[c].fk = Some(end),
                Err(e) => errors.push(format!("column {owner}: fk {e}")),
            }
        }

        for (i, raw) in raw_relations.iter().take(MAX_RELATIONS).enumerate() {
            let what = format!("relation #{}", i + 1);
            let ends = (
                raw.get("from").map(|v| erd.read_end(v, false)),
                raw.get("to").map(|v| erd.read_end(v, false)),
            );
            let (from, to) = match ends {
                (Some(Ok(from)), Some(Ok(to))) => (from, to),
                (Some(Err(e)), _) | (_, Some(Err(e))) => {
                    errors.push(format!("{what}: {e}"));
                    continue;
                }
                _ => {
                    errors.push(format!("{what} needs \"from\" and \"to\" (table.column)"));
                    continue;
                }
            };
            let id = match raw.get("id").and_then(id_of) {
                Some(id) => id,
                None => erd.fresh_relation_id(&from, &to),
            };
            if erd.relation(&id).is_some() {
                errors.push(format!("duplicate relation id \"{id}\""));
                continue;
            }
            let cardinality = raw
                .get("cardinality")
                .and_then(Value::as_str)
                .and_then(Cardinality::parse)
                .unwrap_or_else(|| erd.implied(&from, &to));
            erd.relations.push(Relation {
                id,
                from,
                to,
                cardinality,
                label: text(raw.get("label"), MAX_LABEL),
            });
        }

        // Every foreign key is drawn: one no relation covers gets its own.
        let implied: Vec<Relation> =
            erd.tables
                .iter()
                .flat_map(|t| {
                    t.columns.iter().filter_map(|c| {
                        let fk = c.fk.as_ref()?;
                        let own = End::new(&t.id, &c.name);
                        let covered = erd.relations.iter().any(|r| {
                            (r.from == own && r.to == *fk) || (r.to == own && r.from == *fk)
                        });
                        (!covered).then(|| (own, fk.clone()))
                    })
                })
                .map(|(own, fk)| Relation {
                    id: String::new(),
                    cardinality: erd.implied(&own, &fk),
                    from: own,
                    to: fk,
                    label: None,
                })
                .collect();
        for mut relation in implied {
            if erd.relations.len() >= MAX_RELATIONS {
                break;
            }
            relation.id = erd.fresh_relation_id(&relation.from, &relation.to);
            erd.relations.push(relation);
        }

        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(erd)
    }

    /// `"table.column"` (a table id may hold dots) or `{table, column}`.
    /// For a foreign key the column may be left out: the table's primary
    /// key, else `id`.
    fn read_end(&self, value: &Value, fk: bool) -> Result<End, String> {
        let (table, column) = match value {
            Value::String(s) => {
                let s = s.trim();
                let by_prefix = self.tables.iter().find_map(|t| {
                    let rest = s.strip_prefix(t.id.as_str())?.strip_prefix('.')?;
                    Some((t.id.clone(), Some(rest.to_string())))
                });
                by_prefix.unwrap_or_else(|| match s.rsplit_once('.') {
                    Some((t, c)) => (t.to_string(), Some(c.to_string())),
                    None => (s.to_string(), None),
                })
            }
            Value::Object(map) => (
                map.get("table").and_then(id_of).unwrap_or_default(),
                map.get("column").and_then(id_of),
            ),
            _ => return Err("must be \"table.column\"".into()),
        };
        let Some(t) = self.table(&table) else {
            return Err(format!("table \"{table}\" does not exist"));
        };
        let column = match column.filter(|c| !c.is_empty()) {
            Some(column) => column,
            None if fk => t
                .columns
                .iter()
                .find(|c| c.pk)
                .map_or_else(|| "id".to_string(), |c| c.name.clone()),
            None => return Err(format!("\"{table}\" needs a column (table.column)")),
        };
        if t.column(&column).is_none() {
            return Err(format!("column \"{table}.{column}\" does not exist"));
        }
        Ok(End { table, column })
    }

    /// The cardinality the columns suggest: a foreign key is many-to-one
    /// (one-to-one when unique), a primary key the "one" side.
    pub fn implied(&self, from: &End, to: &End) -> Cardinality {
        let column = |end: &End| self.table(&end.table).and_then(|t| t.column(&end.column));
        let (Some(a), Some(b)) = (column(from), column(to)) else {
            return Cardinality::default();
        };
        let one = |c: &Column| c.pk || c.unique;
        if a.fk.as_ref() == Some(to) {
            return if one(a) {
                Cardinality::OneOne
            } else {
                Cardinality::ManyOne
            };
        }
        if b.fk.as_ref() == Some(from) {
            return if one(b) {
                Cardinality::OneOne
            } else {
                Cardinality::OneMany
            };
        }
        match (a.pk, b.pk) {
            (false, true) => Cardinality::ManyOne,
            (true, true) => Cardinality::OneOne,
            _ => Cardinality::OneMany,
        }
    }

    pub fn table(&self, id: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.id == id)
    }

    pub fn table_index(&self, id: &str) -> Option<usize> {
        self.tables.iter().position(|t| t.id == id)
    }

    fn table_mut(&mut self, id: &str) -> Result<&mut Table, String> {
        self.tables
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("unknown table \"{id}\""))
    }

    pub fn relation(&self, id: &str) -> Option<&Relation> {
        self.relations.iter().find(|r| r.id == id)
    }

    pub fn has(&self, end: &End) -> bool {
        self.table(&end.table)
            .is_some_and(|t| t.column(&end.column).is_some())
    }

    pub fn fresh_table_id(&self, name: &str) -> String {
        fresh_id(name, "table", |id| self.table(id).is_some())
    }

    pub fn fresh_relation_id(&self, from: &End, to: &End) -> String {
        let base = format!("{}->{}", from.wire(), to.wire());
        if self.relation(&base).is_none() {
            return base;
        }
        (2..)
            .map(|n| format!("{base}#{n}"))
            .find(|id| self.relation(id).is_none())
            .expect("an unused suffix exists")
    }

    fn name(&self, table: &str) -> String {
        self.table(table)
            .map_or_else(|| table.to_string(), |t| format!("“{}”", t.name))
    }

    fn column_name(&self, end: &End) -> String {
        let table = self
            .table(&end.table)
            .map_or(end.table.as_str(), |t| &t.name);
        format!("{table}.{}", end.column)
    }

    fn arrow(&self, relation: &Relation) -> String {
        format!(
            "{} → {}",
            self.column_name(&relation.from),
            self.column_name(&relation.to)
        )
    }

    /// Foreign keys pointing at a column that no longer is (or moved).
    fn retarget(&mut self, old: &End, new: Option<&End>) {
        for table in &mut self.tables {
            for column in &mut table.columns {
                if column.fk.as_ref() == Some(old) {
                    column.fk = new.cloned();
                }
            }
        }
        match new {
            Some(new) => {
                for r in &mut self.relations {
                    for end in [&mut r.from, &mut r.to] {
                        if end == old {
                            *end = new.clone();
                        }
                    }
                }
            }
            None => self.relations.retain(|r| r.from != *old && r.to != *old),
        }
    }
}

/// None = unchanged; `Some(None)` clears.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColumnPatch {
    pub name: Option<String>,
    pub ty: Option<Option<String>>,
    pub pk: Option<bool>,
    pub nullable: Option<bool>,
    pub unique: Option<bool>,
    pub fk: Option<Option<End>>,
    pub note: Option<Option<String>>,
}

impl ColumnPatch {
    fn between(a: &Column, b: &Column) -> ColumnPatch {
        ColumnPatch {
            name: (a.name != b.name).then(|| b.name.clone()),
            ty: (a.ty != b.ty).then(|| b.ty.clone()),
            pk: (a.pk != b.pk).then_some(b.pk),
            nullable: (a.nullable != b.nullable).then_some(b.nullable),
            unique: (a.unique != b.unique).then_some(b.unique),
            fk: (a.fk != b.fk).then(|| b.fk.clone()),
            note: (a.note != b.note).then(|| b.note.clone()),
        }
    }

    fn is_empty(&self) -> bool {
        *self == ColumnPatch::default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Move {
        table: String,
        x: f32,
        y: f32,
    },
    AddTable {
        table: Table,
    },
    /// Its relations and the foreign keys to it go too.
    RemoveTable {
        table: String,
    },
    /// `None` = unchanged; a note of `Some(None)` is cleared.
    UpdateTable {
        table: String,
        name: Option<String>,
        note: Option<Option<String>>,
    },
    /// Appended.
    AddColumn {
        table: String,
        column: Column,
    },
    RemoveColumn {
        table: String,
        column: String,
    },
    /// A rename follows into relations and foreign keys.
    UpdateColumn {
        table: String,
        column: String,
        patch: ColumnPatch,
    },
    OrderColumns {
        table: String,
        columns: Vec<String>,
    },
    AddRelation {
        relation: Relation,
    },
    /// Clears the foreign key it stood for.
    RemoveRelation {
        relation: String,
    },
    UpdateRelation {
        relation: String,
        cardinality: Option<Cardinality>,
        label: Option<Option<String>>,
    },
}

fn clean(s: &str, max: usize) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| truncate(s, max))
}

impl Editable for Erd {
    type Change = Change;

    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut sim = old.clone();
        let mut push = |change: Change, sim: &mut Erd| {
            let _ = sim.apply(&change);
            changes.push(change);
        };
        for r in &old.relations {
            if new.relation(&r.id).is_none() && sim.relation(&r.id).is_some() {
                push(
                    Change::RemoveRelation {
                        relation: r.id.clone(),
                    },
                    &mut sim,
                );
            }
        }
        for t in &old.tables {
            if new.table(&t.id).is_none() {
                push(
                    Change::RemoveTable {
                        table: t.id.clone(),
                    },
                    &mut sim,
                );
            }
        }
        for t in &new.tables {
            let Some(before) = sim.table(&t.id).cloned() else {
                push(Change::AddTable { table: t.clone() }, &mut sim);
                continue;
            };
            if before.name != t.name || before.note != t.note {
                push(
                    Change::UpdateTable {
                        table: t.id.clone(),
                        name: (before.name != t.name).then(|| t.name.clone()),
                        note: (before.note != t.note).then(|| t.note.clone()),
                    },
                    &mut sim,
                );
            }
            // A column in the same place under a new name was renamed.
            for (a, b) in before.columns.iter().zip(&t.columns) {
                if a.name != b.name
                    && t.column(&a.name).is_none()
                    && before.column(&b.name).is_none()
                {
                    push(
                        Change::UpdateColumn {
                            table: t.id.clone(),
                            column: a.name.clone(),
                            patch: ColumnPatch {
                                name: Some(b.name.clone()),
                                ..ColumnPatch::default()
                            },
                        },
                        &mut sim,
                    );
                }
            }
            let now = |sim: &Erd| sim.table(&t.id).cloned().unwrap_or_else(|| before.clone());
            for c in now(&sim).columns {
                if t.column(&c.name).is_none() {
                    push(
                        Change::RemoveColumn {
                            table: t.id.clone(),
                            column: c.name.clone(),
                        },
                        &mut sim,
                    );
                }
            }
            for c in &t.columns {
                if now(&sim).column(&c.name).is_none() {
                    push(
                        Change::AddColumn {
                            table: t.id.clone(),
                            column: c.clone(),
                        },
                        &mut sim,
                    );
                }
            }
            let names =
                |t: &Table| -> Vec<String> { t.columns.iter().map(|c| c.name.clone()).collect() };
            if names(&now(&sim)) != names(t) {
                push(
                    Change::OrderColumns {
                        table: t.id.clone(),
                        columns: names(t),
                    },
                    &mut sim,
                );
            }
            for c in &t.columns {
                let current = now(&sim);
                let Some(was) = current.column(&c.name) else {
                    continue;
                };
                let patch = ColumnPatch::between(was, c);
                if !patch.is_empty() {
                    push(
                        Change::UpdateColumn {
                            table: t.id.clone(),
                            column: c.name.clone(),
                            patch,
                        },
                        &mut sim,
                    );
                }
            }
            let at = now(&sim);
            if (at.x, at.y) != (t.x, t.y)
                && let (Some(x), Some(y)) = (t.x, t.y)
            {
                push(
                    Change::Move {
                        table: t.id.clone(),
                        x,
                        y,
                    },
                    &mut sim,
                );
            }
        }
        for r in &new.relations {
            match sim.relation(&r.id) {
                Some(was) if was.from != r.from || was.to != r.to => {
                    push(
                        Change::RemoveRelation {
                            relation: r.id.clone(),
                        },
                        &mut sim,
                    );
                    push(
                        Change::AddRelation {
                            relation: r.clone(),
                        },
                        &mut sim,
                    );
                }
                None => push(
                    Change::AddRelation {
                        relation: r.clone(),
                    },
                    &mut sim,
                ),
                Some(was) if was != r => {
                    let change = Change::UpdateRelation {
                        relation: r.id.clone(),
                        cardinality: (was.cardinality != r.cardinality).then_some(r.cardinality),
                        label: (was.label != r.label).then(|| r.label.clone()),
                    };
                    push(change, &mut sim);
                }
                Some(_) => {}
            }
        }
        // A relation taken out above may have cleared a key the new version keeps.
        for t in &new.tables {
            for c in &t.columns {
                let differs = sim
                    .table(&t.id)
                    .and_then(|s| s.column(&c.name))
                    .is_some_and(|was| was.fk != c.fk);
                if differs {
                    let patch = ColumnPatch {
                        fk: Some(c.fk.clone()),
                        ..ColumnPatch::default()
                    };
                    push(
                        Change::UpdateColumn {
                            table: t.id.clone(),
                            column: c.name.clone(),
                            patch,
                        },
                        &mut sim,
                    );
                }
            }
        }
        changes
    }

    fn apply(&mut self, change: &Change) -> Result<(), String> {
        match change {
            Change::Move { table, x, y } => {
                let t = self.table_mut(table)?;
                t.x = Some(*x);
                t.y = Some(*y);
            }
            Change::AddTable { table } => {
                if table.id.trim().is_empty() || table.name.trim().is_empty() {
                    return Err("a new table needs an id and a name".into());
                }
                if self.table(&table.id).is_some() {
                    return Err(format!("table \"{}\" already exists", table.id));
                }
                if self.tables.len() >= MAX_TABLES {
                    return Err(format!("too many tables (at most {MAX_TABLES})"));
                }
                self.tables.push(table.clone());
            }
            Change::RemoveTable { table } => {
                let at = self
                    .table_index(table)
                    .ok_or_else(|| format!("unknown table \"{table}\""))?;
                let removed = self.tables.remove(at);
                for c in &removed.columns {
                    self.retarget(&End::new(table, &c.name), None);
                }
                self.relations
                    .retain(|r| r.from.table != *table && r.to.table != *table);
            }
            Change::UpdateTable { table, name, note } => {
                let t = self.table_mut(table)?;
                if let Some(name) = name {
                    t.name = clean(name, MAX_LABEL).ok_or("a table needs a name")?;
                }
                if let Some(note) = note {
                    t.note = note.as_deref().and_then(|n| clean(n, MAX_TEXT));
                }
            }
            Change::AddColumn { table, column } => {
                let t = self.table_mut(table)?;
                if column.name.trim().is_empty() {
                    return Err("a column needs a name".into());
                }
                if t.column(&column.name).is_some() {
                    return Err(format!("“{}” already has a column {}", t.name, column.name));
                }
                if t.columns.len() >= MAX_COLUMNS {
                    return Err(format!("too many columns (at most {MAX_COLUMNS})"));
                }
                t.columns.push(column.clone());
            }
            Change::RemoveColumn { table, column } => {
                let t = self.table_mut(table)?;
                let at = t
                    .columns
                    .iter()
                    .position(|c| c.name == *column)
                    .ok_or_else(|| format!("unknown column \"{table}.{column}\""))?;
                t.columns.remove(at);
                self.retarget(&End::new(table, column), None);
            }
            Change::UpdateColumn {
                table,
                column,
                patch,
            } => {
                if let Some(Some(fk)) = &patch.fk
                    && !self.has(fk)
                {
                    return Err(format!("unknown column \"{}\"", fk.wire()));
                }
                let t = self.table_mut(table)?;
                let name = match &patch.name {
                    Some(name) => {
                        let name =
                            clean(name, super::diagram::MAX_ID).ok_or("a column needs a name")?;
                        if name != *column && t.column(&name).is_some() {
                            return Err(format!("“{}” already has a column {name}", t.name));
                        }
                        Some(name)
                    }
                    None => None,
                };
                let c = t
                    .column_mut(column)
                    .ok_or_else(|| format!("unknown column \"{table}.{column}\""))?;
                if let Some(ty) = &patch.ty {
                    c.ty = ty.as_deref().and_then(|t| clean(t, MAX_LABEL));
                }
                if let Some(pk) = patch.pk {
                    c.pk = pk;
                }
                if let Some(nullable) = patch.nullable {
                    c.nullable = nullable;
                }
                if let Some(unique) = patch.unique {
                    c.unique = unique;
                }
                if let Some(fk) = &patch.fk {
                    c.fk = fk.clone();
                }
                if let Some(note) = &patch.note {
                    c.note = note.as_deref().and_then(|n| clean(n, MAX_TEXT));
                }
                if let Some(name) = name.filter(|n| n != column) {
                    c.name = name.clone();
                    self.retarget(&End::new(table, column), Some(&End::new(table, &name)));
                }
            }
            Change::OrderColumns { table, columns } => {
                let t = self.table_mut(table)?;
                if let Some(missing) = columns.iter().find(|c| t.column(c).is_none()) {
                    return Err(format!("unknown column \"{table}.{missing}\""));
                }
                let mut next: Vec<Column> = columns
                    .iter()
                    .filter_map(|name| t.column(name).cloned())
                    .collect();
                next.dedup_by(|a, b| a.name == b.name);
                for c in &t.columns {
                    if !columns.contains(&c.name) {
                        next.push(c.clone());
                    }
                }
                t.columns = next;
            }
            Change::AddRelation { relation } => {
                for end in [&relation.from, &relation.to] {
                    if !self.has(end) {
                        return Err(format!("unknown column \"{}\"", end.wire()));
                    }
                }
                if relation.id.trim().is_empty() {
                    return Err("a new relation needs an id".into());
                }
                if self.relation(&relation.id).is_some() {
                    return Err(format!("relation \"{}\" already exists", relation.id));
                }
                if self.relations.len() >= MAX_RELATIONS {
                    return Err(format!("too many relations (at most {MAX_RELATIONS})"));
                }
                self.relations.push(relation.clone());
            }
            Change::RemoveRelation { relation } => {
                let at = self
                    .relations
                    .iter()
                    .position(|r| r.id == *relation)
                    .ok_or_else(|| format!("unknown relation \"{relation}\""))?;
                let r = self.relations.remove(at);
                for (own, other) in [(&r.from, &r.to), (&r.to, &r.from)] {
                    if let Ok(t) = self.table_mut(&own.table)
                        && let Some(c) = t.column_mut(&own.column)
                        && c.fk.as_ref() == Some(other)
                    {
                        c.fk = None;
                    }
                }
            }
            Change::UpdateRelation {
                relation,
                cardinality,
                label,
            } => {
                let r = self
                    .relations
                    .iter_mut()
                    .find(|r| r.id == *relation)
                    .ok_or_else(|| format!("unknown relation \"{relation}\""))?;
                if let Some(cardinality) = cardinality {
                    r.cardinality = *cardinality;
                }
                if let Some(label) = label {
                    r.label = label.as_deref().and_then(|l| clean(l, MAX_LABEL));
                }
            }
        }
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        match change {
            Change::Move { table, x, y } => json!({
                "op": "move", "table": table, "x": x.round() as i64, "y": y.round() as i64,
            }),
            Change::AddTable { table } => json!({"op": "add_table", "table": table.to_value()}),
            Change::RemoveTable { table } => json!({"op": "remove_table", "table": table}),
            Change::UpdateTable { table, name, note } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_table"));
                map.insert("table".into(), json!(table));
                if let Some(name) = name {
                    map.insert("name".into(), json!(name));
                }
                if let Some(note) = note {
                    map.insert("note".into(), json!(note));
                }
                Value::Object(map)
            }
            Change::AddColumn { table, column } => {
                json!({"op": "add_column", "table": table, "column": column.to_value()})
            }
            Change::RemoveColumn { table, column } => {
                json!({"op": "remove_column", "table": table, "column": column})
            }
            Change::UpdateColumn {
                table,
                column,
                patch,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_column"));
                map.insert("table".into(), json!(table));
                map.insert("column".into(), json!(column));
                if let Some(name) = &patch.name {
                    map.insert("name".into(), json!(name));
                }
                if let Some(ty) = &patch.ty {
                    map.insert("type".into(), json!(ty));
                }
                for (key, value) in [
                    ("pk", patch.pk),
                    ("nullable", patch.nullable),
                    ("unique", patch.unique),
                ] {
                    if let Some(value) = value {
                        map.insert(key.into(), json!(value));
                    }
                }
                if let Some(fk) = &patch.fk {
                    map.insert(
                        "fk".into(),
                        fk.as_ref().map_or(
                            Value::Null,
                            |fk| json!({"table": fk.table, "column": fk.column}),
                        ),
                    );
                }
                if let Some(note) = &patch.note {
                    map.insert("note".into(), json!(note));
                }
                Value::Object(map)
            }
            Change::OrderColumns { table, columns } => {
                json!({"op": "order_columns", "table": table, "columns": columns})
            }
            Change::AddRelation { relation } => {
                json!({"op": "add_relation", "relation": relation.to_value()})
            }
            Change::RemoveRelation { relation } => {
                json!({"op": "remove_relation", "relation": relation})
            }
            Change::UpdateRelation {
                relation,
                cardinality,
                label,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_relation"));
                map.insert("relation".into(), json!(relation));
                if let Some(cardinality) = cardinality {
                    map.insert("cardinality".into(), json!(cardinality.name()));
                }
                if let Some(label) = label {
                    map.insert("label".into(), json!(label));
                }
                Value::Object(map)
            }
        }
    }

    fn describe(change: &Change, before: &Self, after: &Self) -> String {
        match change {
            Change::Move { table, .. } => format!("moved table {}", after.name(table)),
            Change::AddTable { table } => format!(
                "added table “{}” ({})",
                table.name,
                plural(table.columns.len(), "column")
            ),
            Change::RemoveTable { table } => {
                let gone = before
                    .relations
                    .iter()
                    .filter(|r| r.from.table == *table || r.to.table == *table)
                    .count();
                if gone == 0 {
                    format!("removed table {}", before.name(table))
                } else {
                    format!(
                        "removed table {} (and its {})",
                        before.name(table),
                        plural(gone, "relation")
                    )
                }
            }
            Change::UpdateTable { table, name, note } => {
                let mut parts = Vec::new();
                if let Some(name) = name {
                    parts.push(format!("renamed table {} to “{name}”", before.name(table)));
                }
                match note {
                    Some(Some(_)) => {
                        parts.push(format!("updated the note on {}", after.name(table)))
                    }
                    Some(None) => parts.push(format!("cleared the note on {}", after.name(table))),
                    None => {}
                }
                parts.join(", ")
            }
            Change::AddColumn { table, column } => {
                let at = after.column_name(&End::new(table, &column.name));
                match &column.ty {
                    Some(ty) => format!("added column {at} ({ty})"),
                    None => format!("added column {at}"),
                }
            }
            Change::RemoveColumn { table, column } => {
                format!(
                    "removed column {}",
                    before.column_name(&End::new(table, column))
                )
            }
            Change::UpdateColumn {
                table,
                column,
                patch,
            } => {
                let was = before.column_name(&End::new(table, column));
                let now =
                    after.column_name(&End::new(table, patch.name.as_deref().unwrap_or(column)));
                let mut parts = Vec::new();
                if let Some(name) = &patch.name {
                    parts.push(format!("renamed column {was} to {name}"));
                }
                if let Some(ty) = &patch.ty {
                    parts.push(match ty {
                        Some(ty) => format!("made {now} a {ty}"),
                        None => format!("cleared the type of {now}"),
                    });
                }
                let flags = [
                    (patch.pk, "a primary key", "not a primary key"),
                    (patch.nullable, "nullable", "required"),
                    (patch.unique, "unique", "not unique"),
                ];
                for (value, on, off) in flags {
                    if let Some(value) = value {
                        parts.push(format!("made {now} {}", if value { on } else { off }));
                    }
                }
                if let Some(fk) = &patch.fk {
                    parts.push(match fk {
                        Some(fk) => format!("pointed {now} at {}", after.column_name(fk)),
                        None => format!("dropped the foreign key on {now}"),
                    });
                }
                if patch.note.is_some() {
                    parts.push(format!("changed the note on {now}"));
                }
                parts.join(", ")
            }
            Change::OrderColumns { table, .. } => {
                format!("reordered the columns of {}", after.name(table))
            }
            Change::AddRelation { relation } => format!(
                "related {} ({})",
                after.arrow(relation),
                relation.cardinality.name()
            ),
            Change::RemoveRelation { relation } => match before.relation(relation) {
                Some(r) => format!("removed the relation {}", before.arrow(r)),
                None => format!("removed relation {relation}"),
            },
            Change::UpdateRelation {
                relation,
                cardinality,
                label,
            } => {
                let Some(r) = after.relation(relation) else {
                    return format!("changed relation {relation}");
                };
                let mut parts = Vec::new();
                if let Some(cardinality) = cardinality {
                    parts.push(format!("made {} {}", after.arrow(r), cardinality.name()));
                }
                match label {
                    Some(Some(label)) => {
                        parts.push(format!("labeled {} “{label}”", after.arrow(r)))
                    }
                    Some(None) => parts.push(format!("cleared the label of {}", after.arrow(r))),
                    None => {}
                }
                parts.join(", ")
            }
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "tables": self.tables.iter().map(Table::to_value).collect::<Vec<_>>(),
            "relations": self.relations.iter().map(Relation::to_value).collect::<Vec<_>>(),
        })
    }
}

// ── Layout ─────────────────────────────────────────────────────────────────

pub const TABLE_W: f32 = 236.;
pub const HEADER: f32 = 38.;
pub const COLUMN_ROW: f32 = 26.;
const FOOT: f32 = 8.;
/// Between ranks (columns of tables) and between stacked tables.
const RANK_GAP: f32 = 120.;
const STACK_GAP: f32 = 40.;

#[derive(Debug, Clone, PartialEq)]
pub struct RelationGeom {
    pub path: EdgePath,
    /// Unit vectors pointing out of the tables at each end (the markers' axis).
    pub from_dir: Point,
    pub to_dir: Point,
}

/// `tables` and `relations` are index-aligned with the spec's.
#[derive(Debug, Clone, PartialEq)]
pub struct ErdLayout {
    pub tables: Vec<Rect>,
    pub relations: Vec<RelationGeom>,
    pub bounds: Rect,
}

pub fn table_height(t: &Table) -> f32 {
    HEADER + t.columns.len().max(1) as f32 * COLUMN_ROW + FOOT
}

/// The middle of column `i`'s row.
pub fn column_y(rect: &Rect, i: usize) -> f32 {
    rect.y + HEADER + (i as f32 + 0.5) * COLUMN_ROW
}

impl ErdLayout {
    pub fn table_at(&self, p: Point) -> Option<usize> {
        self.tables.iter().rposition(|r| r.contains(p))
    }

    /// The table and column under `p`.
    pub fn column_at(&self, erd: &Erd, p: Point) -> Option<(usize, usize)> {
        let t = self.table_at(p)?;
        let rect = self.tables[t];
        let row = ((p.y - rect.y - HEADER) / COLUMN_ROW).floor();
        (row >= 0. && (row as usize) < erd.tables[t].columns.len()).then_some((t, row as usize))
    }
}

/// Tables ranked by the diagram's layered layout (the "one" side of each
/// relation before the "many" side), stacked in each rank in the order it
/// found with the fewest crossings, at their own heights. Pinned tables
/// keep their place.
pub fn layout(erd: &Erd) -> ErdLayout {
    let proxy = Diagram {
        nodes: erd
            .tables
            .iter()
            .map(|t| Node::new(t.id.clone(), t.name.clone(), NodeKind::Database))
            .collect(),
        edges: erd
            .relations
            .iter()
            .enumerate()
            .filter(|(_, r)| r.from.table != r.to.table)
            .map(|(i, r)| {
                let (from, to) = match r.cardinality {
                    Cardinality::ManyOne => (&r.to.table, &r.from.table),
                    _ => (&r.from.table, &r.to.table),
                };
                Edge {
                    id: format!("r{i}"),
                    from: from.clone(),
                    to: to.clone(),
                    label: None,
                    style: EdgeStyle::Sync,
                    color: None,
                }
            })
            .collect(),
        groups: Vec::new(),
        direction: Direction::LeftRight,
    };
    let ranked = layout::layout(&proxy);
    let rank_count = ranked.ranks.iter().max().map_or(0, |r| r + 1);
    let mut per_rank: Vec<Vec<usize>> = vec![Vec::new(); rank_count];
    for (i, &r) in ranked.ranks.iter().enumerate() {
        per_rank[r].push(i);
    }
    let heights: Vec<f32> = erd.tables.iter().map(table_height).collect();
    let totals: Vec<f32> = per_rank
        .iter()
        .map(|list| list.iter().map(|&i| heights[i] + STACK_GAP).sum::<f32>() - STACK_GAP)
        .collect();
    let tallest = totals.iter().copied().fold(0., f32::max);
    let mut tables = vec![Rect::default(); erd.tables.len()];
    for (rank, list) in per_rank.iter_mut().enumerate() {
        list.sort_by(|a, b| ranked.nodes[*a].y.total_cmp(&ranked.nodes[*b].y));
        let mut y = (tallest - totals[rank]) / 2.;
        for &i in list.iter() {
            tables[i] = Rect {
                x: rank as f32 * (TABLE_W + RANK_GAP),
                y,
                w: TABLE_W,
                h: heights[i],
            };
            y += heights[i] + STACK_GAP;
        }
    }
    for (rect, t) in tables.iter_mut().zip(&erd.tables) {
        if let (Some(x), Some(y)) = (t.x, t.y) {
            rect.x = x;
            rect.y = y;
        }
    }
    let relations = routes(erd, &tables);
    let bounds = tables
        .iter()
        .copied()
        .chain(relations.iter().map(|g| g.path.bounds()))
        .reduce(|a, b| a.union(&b))
        .unwrap_or_default();
    ErdLayout {
        tables,
        relations,
        bounds,
    }
}

/// How far a relation runs straight out of a table before turning: room
/// for the crow's-foot marker and a corner.
const MARKER_REACH: f32 = 40.;

/// Square out of one column's row side and into the other's: across facing
/// sides, a horizontal run, a vertical one in a channel of the gap next to
/// the target (every relation turning in a gap gets its own) and a
/// horizontal one in; when the tables overlap across, a bracket out of the
/// same side of both.
fn routes(erd: &Erd, tables: &[Rect]) -> Vec<RelationGeom> {
    let ends: Vec<Option<(Point, Point, f32, f32)>> = erd
        .relations
        .iter()
        .map(|r| relation_ends(erd, tables, r))
        .collect();
    // Facing relations that turn, by the gap they turn in (bounds rounded to
    // half units so the same gap computed two ways matches), as jogs from
    // left to right.
    type Gap = (f32, f32, Vec<(usize, f32, f32)>);
    let mut gaps: HashMap<(i32, i32), Gap> = HashMap::new();
    for (i, end) in ends.iter().enumerate() {
        let Some((from, to, da, db)) = *end else {
            continue;
        };
        if da == db || (from.y - to.y).abs() < layout::ALIGNED {
            continue;
        }
        let (lo, hi, enter, leave) = if da > 0. {
            ((to.x - RANK_GAP).max(from.x), to.x, from.y, to.y)
        } else {
            (to.x, (to.x + RANK_GAP).min(from.x), to.y, from.y)
        };
        let key = ((lo * 2.).round() as i32, (hi * 2.).round() as i32);
        gaps.entry(key)
            .or_insert_with(|| (lo, hi, Vec::new()))
            .2
            .push((i, enter, leave));
    }
    let mut channel = vec![0.; ends.len()];
    for (lo, hi, jogs) in gaps.into_values() {
        // Pinned tables too close for both markers share the middle.
        let inset = MARKER_REACH.min((hi - lo) / 2.);
        for (i, x) in layout::channels(jogs, lo + inset, hi - inset) {
            channel[i] = x;
        }
    }
    ends.iter()
        .zip(channel)
        .map(|(end, x)| {
            let Some((from, to, da, db)) = *end else {
                let p = Point::default();
                return RelationGeom {
                    path: EdgePath {
                        segments: vec![Cubic {
                            from: p,
                            c1: p,
                            c2: p,
                            to: p,
                        }],
                    },
                    from_dir: Point { x: 1., y: 0. },
                    to_dir: Point { x: -1., y: 0. },
                };
            };
            let x = if da == db {
                if da > 0. {
                    from.x.max(to.x) + MARKER_REACH
                } else {
                    from.x.min(to.x) - MARKER_REACH
                }
            } else {
                x
            };
            let path = if da != db && (from.y - to.y).abs() < layout::ALIGNED {
                layout::orthogonal(&[from, to])
            } else {
                layout::orthogonal(&[from, Point { x, y: from.y }, Point { x, y: to.y }, to])
            };
            RelationGeom {
                path,
                from_dir: Point { x: da, y: 0. },
                to_dir: Point { x: db, y: 0. },
            }
        })
        .collect()
}

/// Where a relation leaves and enters (its columns' row middles, on the
/// sides it uses) and which way each side faces; None when a column is gone.
fn relation_ends(erd: &Erd, tables: &[Rect], r: &Relation) -> Option<(Point, Point, f32, f32)> {
    let place = |end: &End| -> Option<(Rect, f32)> {
        let t = erd.table_index(&end.table)?;
        let c = erd.tables[t]
            .columns
            .iter()
            .position(|c| c.name == end.column)?;
        Some((tables[t], column_y(&tables[t], c)))
    };
    let (a, ya) = place(&r.from)?;
    let (b, yb) = place(&r.to)?;
    let (da, db) = if r.from.table == r.to.table {
        (1., 1.)
    } else if b.x > a.right() + 24. {
        (1., -1.)
    } else if b.right() < a.x - 24. {
        (-1., 1.)
    } else {
        (1., 1.)
    };
    let side = |rect: &Rect, d: f32| if d > 0. { rect.right() } else { rect.x };
    Some((
        Point {
            x: side(&a, da),
            y: ya,
        },
        Point {
            x: side(&b, db),
            y: yb,
        },
        da,
        db,
    ))
}

/// The header's counts.
pub fn counts(erd: &Erd) -> String {
    counts_in(erd, crate::i18n::Language::English)
}

pub fn counts_in(erd: &Erd, language: crate::i18n::Language) -> String {
    use super::edits::count_in;
    format!(
        "{} · {}",
        count_in(
            language,
            erd.tables.len(),
            "{count} table",
            "{count} tables"
        ),
        count_in(
            language,
            erd.relations.len(),
            "{count} relation",
            "{count} relations"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn sample() -> Erd {
        Erd::parse(&json!({
            "tables": [
                {"id": "users", "name": "users", "ref": "db/migrations/001_users.sql:3-12", "columns": [
                    {"name": "id", "type": "uuid", "pk": true},
                    {"name": "email", "type": "text", "unique": true}
                ]},
                {"id": "orders", "name": "orders", "columns": [
                    {"name": "id", "type": "uuid", "pk": true},
                    {"name": "user_id", "type": "uuid", "fk": "users.id"},
                    {"name": "total", "type": "numeric", "nullable": true}
                ]},
                {"id": "items", "columns": ["id", "order_id", "sku"]}
            ],
            "relations": [
                {"from": "orders.id", "to": "items.order_id", "label": "has"}
            ]
        }))
        .unwrap()
    }

    fn round_trip(old: &Erd, new: &Erd) {
        let mut replay = old.clone();
        for change in Erd::diff(old, new) {
            replay
                .apply(&change)
                .unwrap_or_else(|e| panic!("{change:?}: {e}"));
        }
        assert_eq!(&replay, new);
    }

    #[test]
    fn parses_keys_refs_and_implied_relations() {
        let erd = sample();
        let users = erd.table("users").unwrap();
        assert_eq!(
            users.code_ref,
            Some(CodeRef {
                path: "db/migrations/001_users.sql".into(),
                line: Some(3),
                end_line: Some(12),
            })
        );
        assert_eq!(erd.table("items").unwrap().name, "items");
        assert_eq!(erd.relations.len(), 2);
        assert_eq!(erd.relations[0].cardinality, Cardinality::OneMany);
        let fk = &erd.relations[1];
        assert_eq!(fk.id, "orders.user_id->users.id");
        assert_eq!(fk.cardinality, Cardinality::ManyOne);
        assert_eq!(Erd::parse(&erd.to_value()).unwrap(), erd);
    }

    #[test]
    fn readable_errors() {
        let errors = Erd::parse(&json!({
            "tables": [
                {"id": "a", "columns": ["x", "x", {"name": "y", "fk": "b.id"}]},
                {"name": "a"},
                {"columns": []}
            ],
            "relations": [{"from": "a.x"}, {"from": "a", "to": "a.y"}, {"from": "a.z", "to": "a.x"}]
        }))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "table \"a\": duplicate column \"x\"",
                "duplicate table id \"a\"",
                "table #3 has no id",
                "column a.y: fk table \"b\" does not exist",
                "relation #1 needs \"from\" and \"to\" (table.column)",
                "relation #2: \"a\" needs a column (table.column)",
                "relation #3: column \"a.z\" does not exist",
            ]
        );
    }

    #[test]
    fn layout_puts_the_one_side_first_and_rows_at_their_columns() {
        let erd = sample();
        let l = layout(&erd);
        let x = |id: &str| l.tables[erd.table_index(id).unwrap()].x;
        assert!(x("users") < x("orders") && x("orders") < x("items"));
        // The foreign key leaves orders.user_id's row on the left, toward users.
        let fk = &l.relations[1];
        let orders = l.tables[1];
        assert_eq!(
            fk.path.start(),
            Point {
                x: orders.x,
                y: column_y(&orders, 1)
            }
        );
        assert_eq!(fk.from_dir.x, -1.);
        assert_eq!(
            l.column_at(
                &erd,
                Point {
                    x: orders.x + 10.,
                    y: column_y(&orders, 2)
                }
            ),
            Some((1, 2))
        );
        assert_eq!(l.table_at(Point { x: -500., y: 0. }), None);
    }

    #[test]
    fn edits_describe_and_round_trip() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        edits
            .edit(&Change::Move {
                table: "items".into(),
                x: 40.,
                y: 400.,
            })
            .unwrap();
        edits
            .edit(&Change::UpdateColumn {
                table: "users".into(),
                column: "id".into(),
                patch: ColumnPatch {
                    name: Some("user_id".into()),
                    ..ColumnPatch::default()
                },
            })
            .unwrap();
        edits
            .edit(&Change::AddColumn {
                table: "orders".into(),
                column: Column::new("status", Some("text".into())),
            })
            .unwrap();
        let relation = Relation {
            id: base.fresh_relation_id(&End::new("items", "sku"), &End::new("orders", "status")),
            from: End::new("items", "sku"),
            to: End::new("orders", "status"),
            cardinality: Cardinality::ManyMany,
            label: None,
        };
        edits.edit(&Change::AddRelation { relation }).unwrap();
        edits
            .edit(&Change::UpdateTable {
                table: "items".into(),
                name: Some("order_items".into()),
                note: None,
            })
            .unwrap();
        // The renamed key follows into the foreign key and its relation.
        let working = edits.working();
        assert_eq!(
            working
                .table("orders")
                .unwrap()
                .column("user_id")
                .unwrap()
                .fk,
            Some(End::new("users", "user_id"))
        );
        assert_eq!(working.relations[1].to, End::new("users", "user_id"));
        let (_, lines) = edits.summarize();
        assert_eq!(
            lines,
            vec![
                "renamed column users.id to user_id",
                "added column orders.status (text)",
                "renamed table “items” to “order_items”",
                "moved table “order_items”",
                "related order_items.sku → orders.status (n-n)",
            ]
        );
        round_trip(&base, edits.working());
    }

    #[test]
    fn removing_a_relation_drops_its_foreign_key() {
        let base = sample();
        let mut erd = base.clone();
        erd.apply(&Change::RemoveRelation {
            relation: "orders.user_id->users.id".into(),
        })
        .unwrap();
        assert_eq!(
            erd.table("orders").unwrap().column("user_id").unwrap().fk,
            None
        );
        // Parsed again, it stays gone.
        assert_eq!(Erd::parse(&erd.to_value()).unwrap(), erd);
        round_trip(&base, &erd);
        // A table takes its relations and the keys pointing at it along.
        let mut erd = base.clone();
        erd.apply(&Change::RemoveTable {
            table: "users".into(),
        })
        .unwrap();
        assert_eq!(erd.relations.len(), 1);
        assert_eq!(
            erd.table("orders").unwrap().column("user_id").unwrap().fk,
            None
        );
        round_trip(&base, &erd);
    }
}
