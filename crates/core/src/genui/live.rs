//! Live data for the panel kinds (table, chart, checklist): a spec's
//! `live: { source, interval?, command?, map? }` names where its rows,
//! values or items come from, and `map` picks them out of the data with
//! JSONPath-lite paths ([`Path`]). The app reads the source (its own jobs,
//! subagents, todo list and git state, or a command the user allowed:
//! [`Gate`]) and [`fill`]s the spec with it.

use serde_json::{Map, Value, json};

use super::checklist::{MAX_ITEMS, Status};
use super::diagram::{Errors, truncate};
use super::edits::{format_number, fresh_id, number};

/// Seconds between reads of a git source, and of a command, by default.
pub const DEFAULT_GIT_INTERVAL: u32 = 10;
pub const DEFAULT_COMMAND_INTERVAL: u32 = 30;
/// The shortest and longest interval a read source may ask for (seconds).
pub const MIN_INTERVAL: u32 = 5;
pub const MAX_INTERVAL: u32 = 3600;
pub const MAX_COMMAND: usize = 500;
pub const MAX_PATH: usize = 200;
/// A command's output past this many bytes is an error (and the command is stopped).
pub const OUTPUT_CAP: usize = 1024 * 1024;
/// A command still running after this many seconds is stopped.
pub const COMMAND_TIMEOUT: u64 = 30;
/// At most this many records are mapped (a table's row limit).
pub const MAX_RECORDS: usize = 500;

/// Where a live surface's data comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// The session's background jobs (`list_jobs` / `jobs_update`).
    Jobs,
    /// The session's subagents (`list_subagents` / `agents_update`).
    Subagents,
    /// The workspace's changed files.
    GitStatus,
    /// The workspace's latest commits.
    GitLog,
    /// The agent's todo list (the `todo` tool).
    Todo,
    /// A command the user allowed ([`Gate`]).
    Command,
}

impl Source {
    pub const ALL: [Source; 6] = [
        Source::Jobs,
        Source::Subagents,
        Source::GitStatus,
        Source::GitLog,
        Source::Todo,
        Source::Command,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Source::Jobs => "jobs",
            Source::Subagents => "subagents",
            Source::GitStatus => "git:status",
            Source::GitLog => "git:log",
            Source::Todo => "todo",
            Source::Command => "command",
        }
    }

    pub fn parse(name: &str) -> Option<Source> {
        let name = name.trim().to_ascii_lowercase();
        Source::ALL.into_iter().find(|s| s.name() == name)
    }

    /// The app hands these over whenever they change; the others are read
    /// every interval while the surface is on screen.
    pub fn pushed(self) -> bool {
        matches!(self, Source::Jobs | Source::Subagents | Source::Todo)
    }

    fn default_interval(self) -> u32 {
        match self {
            Source::GitStatus | Source::GitLog => DEFAULT_GIT_INTERVAL,
            Source::Command => DEFAULT_COMMAND_INTERVAL,
            _ => 0,
        }
    }
}

/// How a command's output is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    /// One record per non-empty line (a string).
    Lines,
    /// A header row, then one record (an object) per row.
    Csv,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Lines => "lines",
            Format::Csv => "csv",
        }
    }

    fn parse(name: &str) -> Option<Format> {
        [Format::Json, Format::Lines, Format::Csv]
            .into_iter()
            .find(|f| f.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// `live.command`: run in the workspace (or `cwd` inside it) by the
/// user's shell, only once the user allows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub cmd: String,
    /// A folder inside the workspace, relative to it.
    pub cwd: Option<String>,
    pub parse: Format,
}

impl Command {
    /// What consent is given for: the command line and where it runs.
    pub fn key(&self) -> String {
        match &self.cwd {
            Some(cwd) => format!("{} (in {cwd})", self.cmd),
            None => self.cmd.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Segment {
    Key(String),
    Index(usize),
    /// `[*]` or `.*`: every element or value.
    All,
}

/// A JSONPath-lite pick: `$` (the record itself), `.key`, `["key"]`,
/// `[0]`, `[*]` / `.*`; a path without the leading `$` starts at the
/// record (`status` is `$.status`).
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    text: String,
    segments: Vec<Segment>,
}

impl Path {
    pub fn parse(text: &str) -> Result<Path, String> {
        let text = text.trim();
        let bad = |why: &str| format!("bad path \"{}\": {why}", truncate(text, 40));
        if text.is_empty() {
            return Err("a path can't be empty".into());
        }
        if text.chars().count() > MAX_PATH {
            return Err(bad(&format!("longer than {MAX_PATH} characters")));
        }
        let relative = format!(".{text}");
        let mut rest = match text.strip_prefix('$') {
            Some(rest) => rest,
            None if text.starts_with('[') => text,
            None => relative.as_str(),
        };
        let mut segments = Vec::new();
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('.') {
                if let Some(after) = after.strip_prefix('*') {
                    segments.push(Segment::All);
                    rest = after;
                    continue;
                }
                let end = after.find(['.', '[']).unwrap_or(after.len());
                let name = &after[..end];
                if name.is_empty() || name.contains([']', '"', '\'']) {
                    return Err(bad("expected a key after \".\""));
                }
                segments.push(Segment::Key(name.to_string()));
                rest = &after[end..];
            } else if let Some(after) = rest.strip_prefix('[') {
                if let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    let body = &after[1..];
                    let close = body.find(quote).ok_or_else(|| bad("unclosed quote"))?;
                    segments.push(Segment::Key(body[..close].to_string()));
                    rest = body[close + 1..]
                        .strip_prefix(']')
                        .ok_or_else(|| bad("expected \"]\""))?;
                } else {
                    let close = after.find(']').ok_or_else(|| bad("expected \"]\""))?;
                    let inner = after[..close].trim();
                    segments.push(if inner == "*" {
                        Segment::All
                    } else {
                        Segment::Index(
                            inner
                                .parse()
                                .map_err(|_| bad("an index is a number, * or a quoted key"))?,
                        )
                    });
                    rest = &after[close + 1..];
                }
            } else {
                return Err(bad("expected \".\" or \"[\""));
            }
        }
        Ok(Path {
            text: text.to_string(),
            segments,
        })
    }

    /// `$.<key>`, for a default pick.
    pub fn key(key: &str) -> Path {
        Path {
            text: key.to_string(),
            segments: vec![Segment::Key(key.to_string())],
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Every value the path reaches in `value` (wildcards reach several).
    pub fn eval<'a>(&self, value: &'a Value) -> Vec<&'a Value> {
        let mut now = vec![value];
        for segment in &self.segments {
            let mut next = Vec::new();
            for v in now {
                match segment {
                    Segment::Key(key) => next.extend(v.get(key.as_str())),
                    Segment::Index(i) => next.extend(v.as_array().and_then(|a| a.get(*i))),
                    Segment::All => match v {
                        Value::Array(items) => next.extend(items),
                        Value::Object(map) => next.extend(map.values()),
                        _ => {}
                    },
                }
            }
            now = next;
        }
        now
    }

    pub fn first<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        self.eval(value).into_iter().next()
    }
}

/// `live.map`: where the records are and what each kind takes from them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mapping {
    /// The records: the array it reaches (or every value a wildcard
    /// reaches). Default: the data itself.
    pub items: Option<Path>,
    /// A record's id (default its `id`, else a number or a label slug).
    pub id: Option<Path>,
    /// Table: column key → pick (default: the column's key).
    pub columns: Vec<(String, Path)>,
    /// Checklist item label, or chart x label.
    pub label: Option<Path>,
    /// Checklist item status (default `status`), read by [`status_word`].
    pub status: Option<Path>,
    /// Checklist item note (default `note`).
    pub note: Option<Path>,
    /// Chart: series name → pick (default: each series' name).
    pub series: Vec<(String, Path)>,
}

impl Mapping {
    fn parse(raw: &Value, errors: &mut Errors) -> Mapping {
        let Some(raw) = raw.as_object() else {
            errors.push("live.map must be an object".into());
            return Mapping::default();
        };
        let mut path = |key: &str| -> Option<Path> {
            let value = raw.get(key)?;
            match value.as_str().map(Path::parse) {
                Some(Ok(path)) => Some(path),
                Some(Err(error)) => {
                    errors.push(format!("live.map.{key}: {error}"));
                    None
                }
                None => {
                    errors.push(format!("live.map.{key} must be a path string"));
                    None
                }
            }
        };
        let (items, id, label, status, note) = (
            path("items"),
            path("id"),
            path("label"),
            path("status"),
            path("note"),
        );
        let mut picks = |key: &str| -> Vec<(String, Path)> {
            let Some(value) = raw.get(key) else {
                return Vec::new();
            };
            let Some(map) = value.as_object() else {
                errors.push(format!("live.map.{key} must be an object of paths"));
                return Vec::new();
            };
            map.iter()
                .filter_map(|(name, path)| {
                    match path.as_str().ok_or_else(|| "not a string".to_string()) {
                        Ok(text) => match Path::parse(text) {
                            Ok(path) => Some((name.clone(), path)),
                            Err(error) => {
                                errors.push(format!("live.map.{key}.{name}: {error}"));
                                None
                            }
                        },
                        Err(error) => {
                            errors.push(format!("live.map.{key}.{name}: {error}"));
                            None
                        }
                    }
                })
                .collect()
        };
        let (columns, series) = (picks("columns"), picks("series"));
        Mapping {
            items,
            id,
            columns,
            label,
            status,
            note,
            series,
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        for (key, path) in [
            ("items", &self.items),
            ("id", &self.id),
            ("label", &self.label),
            ("status", &self.status),
            ("note", &self.note),
        ] {
            if let Some(path) = path {
                map.insert(key.into(), json!(path.as_str()));
            }
        }
        for (key, picks) in [("columns", &self.columns), ("series", &self.series)] {
            if !picks.is_empty() {
                let picks: Map<String, Value> = picks
                    .iter()
                    .map(|(name, path)| (name.clone(), json!(path.as_str())))
                    .collect();
                map.insert(key.into(), Value::Object(picks));
            }
        }
        Value::Object(map)
    }

    fn is_empty(&self) -> bool {
        self == &Mapping::default()
    }
}

/// A spec's `live` field.
#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    pub source: Source,
    /// Only with [`Source::Command`].
    pub command: Option<Command>,
    /// Seconds between reads; 0 for pushed sources.
    pub interval: u32,
    pub map: Mapping,
}

impl Live {
    /// `"jobs"` is a shorthand for `{source: "jobs"}`; `{command: …}`
    /// without a source is a command source.
    pub fn parse(raw: &Value) -> Result<Live, Vec<String>> {
        let mut errors = Errors::default();
        let object = match raw {
            Value::String(_) => None,
            Value::Object(map) => Some(map),
            _ => return Err(vec!["live must be an object or a source name".into()]),
        };
        let named = match raw {
            Value::String(name) => Some(name.as_str()),
            _ => object.and_then(|o| o.get("source")).and_then(Value::as_str),
        };
        let has_command = object.is_some_and(|o| o.get("command").is_some_and(|c| !c.is_null()));
        let source = match named {
            Some(name) => Source::parse(name).ok_or_else(|| {
                let names: Vec<&str> = Source::ALL.iter().map(|s| s.name()).collect();
                vec![format!(
                    "unknown live source \"{}\" (one of {})",
                    truncate(name, 40),
                    names.join(", ")
                )]
            })?,
            None if has_command => Source::Command,
            None => return Err(vec!["live needs a source".into()]),
        };
        let command = match (source, object.and_then(|o| o.get("command"))) {
            (Source::Command, Some(raw)) => command(raw, &mut errors),
            (Source::Command, None) => {
                errors.push("a command source needs live.command: {cmd, cwd?, parse?}".into());
                None
            }
            (_, Some(c)) if !c.is_null() => {
                errors.push(format!(
                    "live.command only goes with source \"command\", not \"{}\"",
                    source.name()
                ));
                None
            }
            _ => None,
        };
        let interval = if source.pushed() {
            0
        } else {
            object
                .and_then(|o| o.get("interval").or_else(|| command_interval(o)))
                .and_then(number)
                .map_or(source.default_interval(), |n| {
                    (n.round().max(0.) as u32).clamp(MIN_INTERVAL, MAX_INTERVAL)
                })
        };
        let map = match object.and_then(|o| o.get("map")) {
            None | Some(Value::Null) => Mapping::default(),
            Some(raw) => Mapping::parse(raw, &mut errors),
        };
        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(Live {
            source,
            command,
            interval,
            map,
        })
    }

    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("source".into(), json!(self.source.name()));
        if !self.source.pushed() {
            map.insert("interval".into(), json!(self.interval));
        }
        if let Some(command) = &self.command {
            let mut c = Map::new();
            c.insert("cmd".into(), json!(command.cmd));
            if let Some(cwd) = &command.cwd {
                c.insert("cwd".into(), json!(cwd));
            }
            c.insert("parse".into(), json!(command.parse.name()));
            map.insert("command".into(), Value::Object(c));
        }
        if !self.map.is_empty() {
            map.insert("map".into(), self.map.to_value());
        }
        Value::Object(map)
    }

    /// A table's live map may only name its columns.
    pub fn check_columns(&self, keys: &[&str]) -> Vec<String> {
        self.map
            .columns
            .iter()
            .filter(|(key, _)| !keys.contains(&key.as_str()))
            .map(|(key, _)| format!("live.map.columns: \"{key}\" is not a column"))
            .collect()
    }
}

/// `live.command.interval` is accepted for `live.interval`.
fn command_interval(live: &Map<String, Value>) -> Option<&Value> {
    live.get("command")?.get("interval")
}

fn command(raw: &Value, errors: &mut Errors) -> Option<Command> {
    let (cmd, cwd, parse) = match raw {
        Value::String(cmd) => (Some(cmd.as_str()), None, None),
        Value::Object(o) => (
            o.get("cmd").and_then(Value::as_str),
            o.get("cwd").filter(|c| !c.is_null()),
            o.get("parse").filter(|p| !p.is_null()),
        ),
        _ => {
            errors.push("live.command must be {cmd, cwd?, parse?}".into());
            return None;
        }
    };
    let cmd = cmd.map(str::trim).unwrap_or_default();
    if cmd.is_empty() {
        errors.push("live.command.cmd must be a non-empty string".into());
        return None;
    }
    if cmd.chars().count() > MAX_COMMAND {
        errors.push(format!(
            "live.command.cmd is longer than {MAX_COMMAND} characters"
        ));
        return None;
    }
    if cmd.chars().any(char::is_control) {
        errors.push("live.command.cmd must be one line".into());
        return None;
    }
    let cwd = match cwd {
        None => None,
        Some(Value::String(dir)) => match inside(dir) {
            Ok(dir) => dir,
            Err(error) => {
                errors.push(error);
                return None;
            }
        },
        Some(_) => {
            errors.push("live.command.cwd must be a folder path".into());
            return None;
        }
    };
    let parse = match parse {
        None => Format::Lines,
        Some(p) => match p.as_str().and_then(Format::parse) {
            Some(format) => format,
            None => {
                errors.push("live.command.parse must be json, lines or csv".into());
                return None;
            }
        },
    };
    Some(Command {
        cmd: cmd.to_string(),
        cwd,
        parse,
    })
}

/// A folder relative to the workspace and inside it (`None`: the workspace).
fn inside(dir: &str) -> Result<Option<String>, String> {
    let dir = dir.trim().trim_end_matches(['/', '\\']);
    if dir.is_empty() || dir == "." {
        return Ok(None);
    }
    let absolute =
        dir.starts_with(['/', '\\']) || dir.chars().nth(1) == Some(':') || dir.starts_with('~');
    if absolute || dir.split(['/', '\\']).any(|part| part == "..") {
        return Err(format!(
            "live.command.cwd \"{}\" must be a folder inside the workspace (relative, without \"..\")",
            truncate(dir, 60)
        ));
    }
    Ok(Some(dir.to_string()))
}

/// The records `map` reaches in `data`: the elements of the one array its
/// `items` reaches, or every value it reaches (at most [`MAX_RECORDS`]).
pub fn records<'a>(map: &Mapping, data: &'a Value) -> Vec<&'a Value> {
    let found = match &map.items {
        Some(path) => path.eval(data),
        None => vec![data],
    };
    let list: Vec<&Value> = match found.as_slice() {
        [Value::Array(items)] => items.iter().collect(),
        [Value::Null] => Vec::new(),
        _ => found,
    };
    list.into_iter().take(MAX_RECORDS).collect()
}

/// A picked value as text (`None` when it is missing or null).
fn as_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Null => None,
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.as_f64().map_or_else(|| n.to_string(), format_number)),
        Value::Bool(b) => Some(if *b { "yes" } else { "no" }.to_string()),
        other => Some(other.to_string()),
    }
}

/// A picked value as a table cell: scalars as they are, links kept, any
/// other object or array as its JSON text.
fn as_cell(value: Option<&Value>) -> Value {
    match value {
        None | Some(Value::Null) => Value::Null,
        Some(v @ (Value::String(_) | Value::Number(_) | Value::Bool(_))) => v.clone(),
        Some(v) if v.get("url").is_some_and(Value::is_string) => v.clone(),
        Some(v) => json!(truncate(&v.to_string(), 500)),
    }
}

/// `pick` in `record`, else the first of `keys` it has, else the record
/// itself when it is a scalar.
fn pick_text(record: &Value, pick: Option<&Path>, keys: &[&str]) -> Option<String> {
    if let Some(path) = pick {
        return as_text(path.first(record));
    }
    keys.iter()
        .find_map(|key| as_text(record.get(*key)))
        .or_else(|| (!record.is_object() && !record.is_array()).then(|| as_text(Some(record)))?)
}

/// Record ids: the picked or own `id`, else `fallback(n, label)`; repeats
/// get "-2", "-3"….
fn ids(records: &[&Value], map: &Mapping, fallback: impl Fn(usize) -> String) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for (n, record) in records.iter().enumerate() {
        let id = pick_text(record, map.id.as_ref(), &["id"])
            .filter(|_| map.id.is_some() || record.get("id").is_some())
            .map(|id| id.chars().take(60).collect::<String>())
            .unwrap_or_else(|| fallback(n));
        let id = fresh_id(&id, "row", |taken| seen.iter().any(|s| s == taken));
        seen.push(id);
    }
    seen
}

/// `spec` (a table, chart or checklist's wire JSON) with what `data` holds
/// in place of its rows, values or items; other kinds are returned as they are.
pub fn fill(kind: &str, spec: &Value, live: &Live, data: &Value) -> Value {
    let mut out = spec.clone();
    let records = records(&live.map, data);
    let Some(map) = out.as_object_mut() else {
        return out;
    };
    match kind {
        "table" => fill_table(map, &live.map, &records),
        "chart" => fill_chart(map, &live.map, &records),
        "checklist" => fill_checklist(map, &live.map, &records),
        _ => {}
    }
    out
}

fn fill_table(spec: &mut Map<String, Value>, map: &Mapping, records: &[&Value]) {
    let keys: Vec<String> = spec
        .get("columns")
        .and_then(Value::as_array)
        .map(|columns| {
            columns
                .iter()
                .filter_map(|c| c.get("key").or(Some(c)).and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let ids = ids(records, map, |n| format!("row-{}", n + 1));
    let rows: Vec<Value> = records
        .iter()
        .zip(ids)
        .map(|(record, id)| {
            let cells: Map<String, Value> = keys
                .iter()
                .map(|key| {
                    let value = match map.columns.iter().find(|(k, _)| k == key) {
                        Some((_, path)) => as_cell(path.first(record)),
                        None => as_cell(record.get(key.as_str())),
                    };
                    (key.clone(), value)
                })
                .filter(|(_, v)| !v.is_null())
                .collect();
            json!({"id": id, "cells": cells})
        })
        .collect();
    spec.insert("rows".into(), Value::Array(rows));
    // Rows the data owns aren't the user's to edit.
    spec.insert("editable".into(), json!(false));
}

fn fill_chart(spec: &mut Map<String, Value>, map: &Mapping, records: &[&Value]) {
    if spec.get("type").and_then(Value::as_str) == Some("scatter") {
        return;
    }
    let labels: Vec<String> = records
        .iter()
        .enumerate()
        .map(|(n, record)| {
            pick_text(record, map.label.as_ref(), &["label", "name", "title"])
                .map_or_else(|| (n + 1).to_string(), |l| truncate(&l, 80))
        })
        .collect();
    let picks: Vec<(String, Path)> = if map.series.is_empty() {
        spec.get("series")
            .and_then(Value::as_array)
            .map(|series| {
                series
                    .iter()
                    .filter_map(|s| s.get("name").and_then(Value::as_str))
                    .map(|name| (name.to_string(), Path::key(name)))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        map.series.clone()
    };
    let series: Vec<Value> = picks
        .iter()
        .map(|(name, path)| {
            let values: Vec<f64> = records
                .iter()
                .map(|record| path.first(record).and_then(number).unwrap_or(0.))
                .collect();
            json!({"name": name, "values": values})
        })
        .collect();
    spec.insert("labels".into(), json!(labels));
    spec.insert("series".into(), Value::Array(series));
}

fn fill_checklist(spec: &mut Map<String, Value>, map: &Mapping, records: &[&Value]) {
    let records = &records[..records.len().min(MAX_ITEMS)];
    let labels: Vec<String> = records
        .iter()
        .enumerate()
        .map(|(n, record)| {
            pick_text(
                record,
                map.label.as_ref(),
                &["title", "label", "subject", "name", "path", "agent", "id"],
            )
            .unwrap_or_else(|| format!("Item {}", n + 1))
        })
        .collect();
    let ids = ids(records, map, |n| fresh_id(&labels[n], "item", |_| false));
    let items: Vec<Value> = records
        .iter()
        .zip(ids)
        .zip(&labels)
        .map(|((record, id), label)| {
            let status = pick_text(record, map.status.as_ref(), &["status"])
                .map_or(Status::Pending, |s| status_word(&s));
            let mut item = Map::new();
            item.insert("id".into(), json!(id));
            item.insert("label".into(), json!(truncate(label, 200)));
            item.insert("done".into(), json!(status == Status::Done));
            item.insert("status".into(), json!(status.name()));
            if let Some(note) = pick_text(record, map.note.as_ref(), &["note"]) {
                item.insert("note".into(), json!(note));
            }
            Value::Object(item)
        })
        .collect();
    spec.insert("items".into(), Value::Array(items));
}

/// A status word from any source as a checklist status: done, completed,
/// passed… are done; running, in_progress, active… running; failed,
/// error, timed_out… failed; skipped, cancelled… skipped; anything else
/// pending.
pub fn status_word(word: &str) -> Status {
    let word = word.trim().to_lowercase().replace([' ', '-'], "_");
    match word.as_str() {
        "done" | "completed" | "complete" | "succeeded" | "success" | "passed" | "pass" | "ok"
        | "finished" | "true" | "x" | "merged" => Status::Done,
        "running" | "in_progress" | "active" | "started" | "starting" | "working" | "writing"
        | "busy" => Status::Running,
        "failed" | "failure" | "fail" | "error" | "errored" | "failing" | "timed_out"
        | "timeout" | "interrupted" | "broken" => Status::Failed,
        "skipped" | "skip" | "cancelled" | "canceled" | "dismissed" | "aborted" | "dropped" => {
            Status::Skipped
        }
        _ => Status::Pending,
    }
}

/// Data for the UI to render a parse failure without changing its English representation.
#[derive(Debug)]
pub struct OutputError {
    pub error: serde_json::Error,
    pub preview: String,
}

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the output isn't JSON ({}): {}",
            self.error, self.preview
        )
    }
}

/// A command's output as data.
pub fn parse_output(format: Format, text: &str) -> Result<Value, String> {
    parse_output_with_details(format, text).map_err(|error| error.to_string())
}

pub fn parse_output_with_details(format: Format, text: &str) -> Result<Value, OutputError> {
    match format {
        Format::Json => serde_json::from_str(text.trim()).map_err(|error| OutputError {
            error,
            preview: truncate(text.trim(), 80),
        }),
        Format::Lines => Ok(Value::Array(
            text.lines()
                .map(str::trim_end)
                .filter(|l| !l.trim().is_empty())
                .take(MAX_RECORDS)
                .map(|l| json!(l))
                .collect(),
        )),
        Format::Csv => {
            let mut rows = csv_rows(text).into_iter();
            let header = rows.next().unwrap_or_default();
            Ok(Value::Array(
                rows.take(MAX_RECORDS)
                    .map(|row| {
                        let record: Map<String, Value> = header
                            .iter()
                            .zip(row.into_iter().chain(std::iter::repeat(String::new())))
                            .map(|(key, value)| (key.trim().to_string(), json!(value)))
                            .collect();
                        Value::Object(record)
                    })
                    .collect(),
            ))
        }
    }
}

/// RFC 4180-ish rows: commas, double quotes ("" inside quotes), CRLF or LF;
/// blank lines skipped.
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let end_row = |row: &mut Vec<String>, field: &mut String, rows: &mut Vec<Vec<String>>| {
        row.push(std::mem::take(field));
        let row = std::mem::take(row);
        if !(row.len() == 1 && row[0].trim().is_empty()) {
            rows.push(row);
        }
    };
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') if field.is_empty() => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut field)),
            (false, '\r') if chars.peek() == Some(&'\n') => {}
            (false, '\n') => end_row(&mut row, &mut field, &mut rows),
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        end_row(&mut row, &mut field, &mut rows);
    }
    rows
}

/// "Live · updated 5 s ago": how long ago, in steps of 5 s, then minutes
/// and hours (so the label changes at most every 5 s).
pub fn ago(seconds: u64) -> String {
    ago_in(seconds, crate::i18n::Language::English)
}

pub fn ago_in(seconds: u64, language: crate::i18n::Language) -> String {
    let (key, count) = match seconds {
        0..5 => return crate::i18n::tr(language, "just now").into(),
        5..60 => ("{count} s ago", seconds / 5 * 5),
        60..3600 => ("{count} min ago", seconds / 60),
        _ => ("{count} h ago", seconds / 3600),
    };
    crate::i18n::tr(language, key).replace("{count}", &count.to_string())
}

/// What the user said about a command source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// Not asked yet: the card asks, and nothing runs.
    Ask,
    /// Allowed for this card (until its command changes or the app restarts).
    Once,
    /// Allowed for every card with this command in the conversation.
    Session,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Once,
    Session,
    Deny,
}

/// One command source's consent: a command line never runs until the user
/// allows it, and a different command is asked about again.
#[derive(Debug, Clone, PartialEq)]
pub struct Gate {
    key: String,
    consent: Consent,
}

impl Gate {
    /// `session_allows`: the user already allowed this command for the
    /// session (on another card).
    pub fn new(key: impl Into<String>, session_allows: bool) -> Gate {
        Gate {
            key: key.into(),
            consent: if session_allows {
                Consent::Session
            } else {
                Consent::Ask
            },
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn consent(&self) -> Consent {
        self.consent
    }

    pub fn may_run(&self) -> bool {
        matches!(self.consent, Consent::Once | Consent::Session)
    }

    /// The agent sent a new version: the same command keeps its answer, a
    /// different one starts over.
    pub fn retarget(&mut self, key: &str, session_allows: bool) {
        if key != self.key {
            *self = Gate::new(key, session_allows);
        } else if session_allows {
            self.session_allowed();
        }
    }

    pub fn decide(&mut self, decision: Decision) {
        self.consent = match decision {
            Decision::Once => Consent::Once,
            Decision::Session => Consent::Session,
            Decision::Deny => Consent::Denied,
        };
    }

    /// Another card allowed this command for the session: a card still
    /// asking takes it; a denial stands.
    pub fn session_allowed(&mut self) {
        if self.consent == Consent::Ask {
            self.consent = Consent::Session;
        }
    }

    /// "Ask again" after a denial.
    pub fn ask_again(&mut self) {
        if self.consent == Consent::Denied {
            self.consent = Consent::Ask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_pick_keys_indexes_and_wildcards() {
        let data = json!({"suites": [
            {"name": "api", "stats": {"passed": 3, "failed": 1}, "tags": ["a", "b"]},
            {"name": "web", "stats": {"passed": 5, "failed": 0}, "weird key": 7}
        ]});
        let eval = |p: &str| {
            Path::parse(p)
                .unwrap()
                .eval(&data)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(eval("$.suites[1].name"), vec![json!("web")]);
        assert_eq!(eval("suites[0].stats.failed"), vec![json!(1)]);
        assert_eq!(eval("$.suites[*].name"), vec![json!("api"), json!("web")]);
        assert_eq!(eval("$.suites[0].stats.*"), vec![json!(3), json!(1)]);
        assert_eq!(eval("$.suites[1]['weird key']"), vec![json!(7)]);
        assert_eq!(eval("$.suites[0][\"tags\"][1]"), vec![json!("b")]);
        assert_eq!(eval("$.nope.deeper"), Vec::<Value>::new());
        assert_eq!(eval("$"), vec![data.clone()]);
        for bad in ["", "$.", "$[x]", "$.a[1", "$['a]", "$..a", "$a"] {
            assert!(Path::parse(bad).is_err(), "{bad} is not a path");
        }
    }

    #[test]
    fn live_parses_sources_commands_intervals_and_maps() {
        let jobs = Live::parse(&json!("jobs")).unwrap();
        assert_eq!(jobs.source, Source::Jobs);
        assert_eq!(jobs.interval, 0, "pushed sources have no interval");
        assert_eq!(jobs.to_value(), json!({"source": "jobs"}));

        let git = Live::parse(&json!({"source": "git:log", "interval": 1})).unwrap();
        assert_eq!(git.interval, MIN_INTERVAL, "clamped to the minimum");

        let cmd = Live::parse(&json!({
            "command": {"cmd": " npm test -- --json ", "cwd": "web/", "parse": "json"},
            "interval": 30,
            "map": {"items": "$.testResults", "columns": {"name": "$.name"}}
        }))
        .unwrap();
        assert_eq!(cmd.source, Source::Command);
        let command = cmd.command.as_ref().unwrap();
        assert_eq!(command.cmd, "npm test -- --json");
        assert_eq!(command.cwd.as_deref(), Some("web"));
        assert_eq!(command.parse, Format::Json);
        assert_eq!(command.key(), "npm test -- --json (in web)");
        assert_eq!(Live::parse(&cmd.to_value()).unwrap(), cmd, "round trip");
        assert_eq!(
            cmd.check_columns(&["status"]),
            vec!["live.map.columns: \"name\" is not a column".to_string()]
        );

        let errors = |raw: Value| Live::parse(&raw).unwrap_err().join("\n");
        assert!(errors(json!({"source": "files"})).contains("unknown live source"));
        assert!(errors(json!({"source": "command"})).contains("needs live.command"));
        assert!(errors(json!({"source": "jobs", "command": "ls"})).contains("only goes with"));
        assert!(
            errors(json!({"command": {"cmd": "ls", "cwd": "../x"}}))
                .contains("inside the workspace")
        );
        assert!(
            errors(json!({"command": {"cmd": "ls", "cwd": "/etc"}}))
                .contains("inside the workspace")
        );
        assert!(errors(json!({"command": "ls\nrm -rf x"})).contains("one line"));
        assert!(
            errors(json!({"command": {"cmd": "ls", "parse": "xml"}}))
                .contains("json, lines or csv")
        );
        assert!(
            errors(json!({"source": "todo", "map": {"label": "$["}})).contains("live.map.label")
        );
    }

    #[test]
    fn fills_a_table_from_records() {
        let live = Live::parse(&json!({
            "source": "command",
            "command": {"cmd": "x", "parse": "json"},
            "map": {"items": "$.results", "id": "$.file", "columns": {"fails": "$.stats.failed"}}
        }))
        .unwrap();
        let spec = json!({"columns": [{"key": "file"}, {"key": "fails", "type": "number"}], "rows": [], "editable": true});
        let data = json!({"results": [
            {"file": "a.rs", "stats": {"failed": 2}},
            {"file": "a.rs", "stats": {"failed": 0}},
            {"file": "b.rs", "stats": {"failed": {"n": 1}}}
        ]});
        let filled = fill("table", &spec, &live, &data);
        assert_eq!(filled["editable"], json!(false));
        let rows = filled["rows"].as_array().unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["a-rs", "a-rs-2", "b-rs"]);
        assert_eq!(rows[0]["cells"], json!({"file": "a.rs", "fails": 2}));
        assert_eq!(rows[2]["cells"]["fails"], json!("{\"n\":1}"));
        let table = crate::genui::table::Table::parse(&filled).unwrap();
        assert_eq!(table.rows.len(), 3);
    }

    #[test]
    fn fills_a_chart_and_a_checklist() {
        let live = Live::parse(&json!({"source": "git:log", "map": {"label": "short", "series": {"Files": "$.files"}}})).unwrap();
        let spec = json!({"type": "bar", "series": [{"name": "Files", "values": []}]});
        let data = json!([{"short": "abc", "files": 3}, {"short": "def", "files": "x"}]);
        let filled = fill("chart", &spec, &live, &data);
        assert_eq!(filled["labels"], json!(["abc", "def"]));
        assert_eq!(
            filled["series"],
            json!([{"name": "Files", "values": [3.0, 0.0]}])
        );

        let todo = Live::parse(&json!("todo")).unwrap();
        let data = json!([
            {"title": "Write tests", "status": "done"},
            {"title": "Ship", "status": "in_progress", "note": "on it"},
            {"title": "Ship", "status": "blocked"}
        ]);
        let filled = fill("checklist", &json!({"items": []}), &todo, &data);
        let list = crate::genui::checklist::Checklist::parse(&filled).unwrap();
        let shown: Vec<(&str, Status, bool)> = list
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.status, i.done))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("write-tests", Status::Done, true),
                ("ship", Status::Running, false),
                ("ship-2", Status::Pending, false),
            ]
        );
        assert_eq!(list.items[1].note.as_deref(), Some("on it"));
    }

    #[test]
    fn status_words_map_to_checklist_statuses() {
        for (word, status) in [
            ("Completed", Status::Done),
            ("in-progress", Status::Running),
            ("timed out", Status::Failed),
            ("cancelled", Status::Skipped),
            ("queued", Status::Pending),
            ("", Status::Pending),
        ] {
            assert_eq!(status_word(word), status, "{word}");
        }
    }

    #[test]
    fn command_output_parses_as_json_lines_or_csv() {
        assert_eq!(
            parse_output(Format::Json, " {\"a\": 1}\n").unwrap(),
            json!({"a": 1})
        );
        assert!(
            parse_output(Format::Json, "npm WARN\n{")
                .unwrap_err()
                .contains("isn't JSON")
        );
        assert_eq!(
            parse_output(Format::Lines, "a\n\n b \r\n").unwrap(),
            json!(["a", " b"])
        );
        assert_eq!(
            parse_output(
                Format::Csv,
                "name,note\r\napi,\"says \"\"hi\"\", ok\"\n\nweb\n"
            )
            .unwrap(),
            json!([{"name": "api", "note": "says \"hi\", ok"}, {"name": "web", "note": ""}])
        );
    }

    #[test]
    fn consent_starts_asking_and_follows_the_user() {
        let mut gate = Gate::new("npm test", false);
        assert_eq!(gate.consent(), Consent::Ask);
        assert!(!gate.may_run(), "nothing runs before the user answers");
        gate.decide(Decision::Deny);
        assert!(!gate.may_run());
        gate.session_allowed();
        assert_eq!(gate.consent(), Consent::Denied, "a denial stands");
        gate.ask_again();
        assert_eq!(gate.consent(), Consent::Ask);
        gate.decide(Decision::Once);
        assert!(gate.may_run());
        gate.retarget("npm test", false);
        assert_eq!(
            gate.consent(),
            Consent::Once,
            "the same command keeps its answer"
        );
        gate.retarget("rm -rf /", false);
        assert_eq!(
            gate.consent(),
            Consent::Ask,
            "a new command is asked about again"
        );
        gate.retarget("rm -rf /", true);
        assert_eq!(gate.consent(), Consent::Session);
        assert_eq!(Gate::new("ls", true).consent(), Consent::Session);
    }

    #[test]
    fn ago_steps_by_five_seconds_then_minutes() {
        assert_eq!(ago(3), "just now");
        assert_eq!(ago(7), "5 s ago");
        assert_eq!(ago(59), "55 s ago");
        assert_eq!(ago(61), "1 min ago");
        assert_eq!(ago(7300), "2 h ago");
    }
}
