//! The review kind: a change set the agent proposes before writing it
//! (files and their hunks), which the user reviews hunk by hunk: accepted,
//! rejected and commented hunks go back as decisions, and the agent marks
//! the hunks it applied. The hunks arrive already computed (the extension
//! turns a file's `before`/`after` into hunks), so nothing here diffs text.

use serde_json::{Map, Value, json};

use super::diagram::{Errors, MAX_TEXT, array, id_of, text, truncate};
use super::edits::{Editable, list_names};

pub const MAX_FILES: usize = 50;
pub const MAX_HUNKS: usize = 300;
pub const MAX_HUNK_LINES: usize = 1000;
pub const MAX_LINES: usize = 6000;
pub const MAX_LINE: usize = 1000;
pub const MAX_PATH: usize = 300;
pub const MAX_HEADER: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Added => "added",
            Status::Modified => "modified",
            Status::Deleted => "deleted",
            Status::Renamed => "renamed",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        Some(match s.trim().to_lowercase().as_str() {
            "added" | "add" | "new" | "created" => Status::Added,
            "modified" | "modify" | "changed" | "edit" | "edited" => Status::Modified,
            "deleted" | "delete" | "removed" => Status::Deleted,
            "renamed" | "rename" | "moved" => Status::Renamed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Del,
}

impl LineKind {
    pub fn name(self) -> &'static str {
        match self {
            LineKind::Context => "context",
            LineKind::Add => "add",
            LineKind::Del => "del",
        }
    }

    fn parse(s: &str) -> Option<LineKind> {
        Some(match s.trim().to_lowercase().as_str() {
            "context" | "ctx" | " " | "" | "same" | "unchanged" => LineKind::Context,
            "add" | "added" | "+" | "insert" | "ins" => LineKind::Add,
            "del" | "delete" | "deleted" | "remove" | "removed" | "-" => LineKind::Del,
            _ => return None,
        })
    }

    /// The unified diff prefix.
    pub fn prefix(self) -> char {
        match self {
            LineKind::Context => ' ',
            LineKind::Add => '+',
            LineKind::Del => '-',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Accepted,
    Rejected,
}

impl Decision {
    pub fn name(self) -> &'static str {
        match self {
            Decision::Accepted => "accepted",
            Decision::Rejected => "rejected",
        }
    }

    pub fn parse(s: &str) -> Option<Decision> {
        match s.trim().to_lowercase().as_str() {
            "accepted" | "accept" => Some(Decision::Accepted),
            "rejected" | "reject" => Some(Decision::Rejected),
            _ => None,
        }
    }
}

/// Which side of the change a line number belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Old,
    New,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hunk {
    pub id: String,
    /// What follows `@@ … @@` (the enclosing function, say).
    pub header: Option<String>,
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<Line>,
    /// The agent's note about this hunk.
    pub note: Option<String>,
    pub decision: Option<Decision>,
    pub comment: Option<String>,
    /// The agent wrote it.
    pub applied: bool,
}

impl Hunk {
    pub fn old_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind != LineKind::Add)
            .count()
    }

    pub fn new_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind != LineKind::Del)
            .count()
    }

    pub fn additions(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == LineKind::Add)
            .count()
    }

    pub fn deletions(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == LineKind::Del)
            .count()
    }

    /// Each line's side and number, in order (context lines on the new side).
    pub fn positions(&self) -> Vec<(Side, usize)> {
        let (mut old, mut new) = (self.old_start.max(1), self.new_start.max(1));
        self.lines
            .iter()
            .map(|line| match line.kind {
                LineKind::Context => {
                    old += 1;
                    new += 1;
                    (Side::New, new - 1)
                }
                LineKind::Add => {
                    new += 1;
                    (Side::New, new - 1)
                }
                LineKind::Del => {
                    old += 1;
                    (Side::Old, old - 1)
                }
            })
            .collect()
    }

    /// The line a unified view shows last: the last line, unless the hunk
    /// ends in a block of changes, which shows its removed lines before its
    /// added ones.
    pub fn last_shown(&self) -> (Side, usize) {
        let positions = self.positions();
        let tail = self
            .lines
            .iter()
            .rposition(|l| l.kind == LineKind::Context)
            .map_or(0, |i| i + 1);
        if tail == self.lines.len() {
            return positions[positions.len() - 1];
        }
        let block = tail..self.lines.len();
        let last_add = block
            .clone()
            .rev()
            .find(|&i| self.lines[i].kind == LineKind::Add);
        positions[last_add.unwrap_or(block.end - 1)]
    }

    /// The line a unified view shows first (see [`Self::last_shown`]).
    pub fn first_shown(&self) -> (Side, usize) {
        let positions = self.positions();
        let end = self
            .lines
            .iter()
            .position(|l| l.kind == LineKind::Context)
            .unwrap_or(self.lines.len());
        let first_del = (0..end).find(|&i| self.lines[i].kind == LineKind::Del);
        positions[first_del.unwrap_or(0)]
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        if let Some(header) = &self.header {
            map.insert("header".into(), json!(header));
        }
        map.insert("oldStart".into(), json!(self.old_start));
        map.insert("newStart".into(), json!(self.new_start));
        map.insert(
            "lines".into(),
            self.lines
                .iter()
                .map(|l| json!({"kind": l.kind.name(), "text": l.text}))
                .collect(),
        );
        if let Some(note) = &self.note {
            map.insert("note".into(), json!(note));
        }
        if let Some(decision) = self.decision {
            map.insert("decision".into(), json!(decision.name()));
        }
        if let Some(comment) = &self.comment {
            map.insert("comment".into(), json!(comment));
        }
        if self.applied {
            map.insert("applied".into(), json!(true));
        }
        Value::Object(map)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub path: String,
    pub status: Status,
    pub old_path: Option<String>,
    /// A highlighter language name ("rust"); else detected from the path.
    pub language: Option<String>,
    pub hunks: Vec<Hunk>,
}

impl File {
    pub fn additions(&self) -> usize {
        self.hunks.iter().map(Hunk::additions).sum()
    }

    pub fn deletions(&self) -> usize {
        self.hunks.iter().map(Hunk::deletions).sum()
    }

    pub fn decided(&self) -> usize {
        self.hunks.iter().filter(|h| h.decision.is_some()).count()
    }

    /// The file as a Git patch, for a diff viewer.
    pub fn patch(&self) -> String {
        let old = self.old_path.as_deref().unwrap_or(&self.path);
        let mut out = format!("diff --git a/{old} b/{}\n", self.path);
        let (from, to) = match self.status {
            Status::Added => {
                out.push_str("new file mode 100644\n");
                ("/dev/null".to_string(), format!("b/{}", self.path))
            }
            Status::Deleted => {
                out.push_str("deleted file mode 100644\n");
                (format!("a/{}", self.path), "/dev/null".to_string())
            }
            Status::Renamed => {
                out.push_str(&format!("rename from {old}\nrename to {}\n", self.path));
                (format!("a/{old}"), format!("b/{}", self.path))
            }
            Status::Modified => (format!("a/{}", self.path), format!("b/{}", self.path)),
        };
        if self.hunks.is_empty() {
            return out;
        }
        out.push_str(&format!("--- {from}\n+++ {to}\n"));
        for hunk in &self.hunks {
            out.push_str(&format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start,
                hunk.old_count(),
                hunk.new_start,
                hunk.new_count()
            ));
            if let Some(header) = &hunk.header {
                out.push(' ');
                out.push_str(header);
            }
            out.push('\n');
            for line in &hunk.lines {
                out.push(line.kind.prefix());
                out.push_str(&line.text);
                out.push('\n');
            }
        }
        out
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("path".into(), json!(self.path));
        map.insert("status".into(), json!(self.status.name()));
        if let Some(old) = &self.old_path {
            map.insert("oldPath".into(), json!(old));
        }
        if let Some(language) = &self.language {
            map.insert("language".into(), json!(language));
        }
        map.insert(
            "hunks".into(),
            self.hunks.iter().map(Hunk::to_value).collect(),
        );
        Value::Object(map)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Review {
    pub summary: Option<String>,
    pub files: Vec<File>,
}

/// A non-negative whole line number.
fn line_number(value: Option<&Value>) -> Option<usize> {
    let n = value?.as_f64()?;
    (n.is_finite() && n >= 0. && n.fract() == 0.).then_some(n as usize)
}

/// A path: trimmed, `\` as `/`, without a leading `./`, `a/` or `b/`.
fn path_of(value: Option<&Value>) -> Option<String> {
    let path = value?.as_str()?.trim().replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path).to_string();
    (!path.is_empty()).then_some(path)
}

/// A hunk's lines: `{kind, text}` objects or strings with a unified diff
/// prefix (`+`, `-`, space; none is context). Text with newlines is split.
fn read_lines(raw: &[Value], hunk: &str, errors: &mut Errors) -> Vec<Line> {
    let mut lines = Vec::new();
    for (i, item) in raw.iter().enumerate() {
        let (kind, text) = match item {
            Value::String(s) => match s.chars().next() {
                Some('+') => (LineKind::Add, &s[1..]),
                Some('-') => (LineKind::Del, &s[1..]),
                Some(' ') => (LineKind::Context, &s[1..]),
                _ => (LineKind::Context, s.as_str()),
            },
            Value::Object(o) => {
                let kind = o.get("kind").or_else(|| o.get("type"));
                let Some(kind) = kind.and_then(Value::as_str).and_then(LineKind::parse) else {
                    errors.push(format!(
                        "line {} of hunk \"{hunk}\" needs kind context, add or del",
                        i + 1
                    ));
                    continue;
                };
                (kind, o.get("text").and_then(Value::as_str).unwrap_or(""))
            }
            _ => {
                errors.push(format!(
                    "line {} of hunk \"{hunk}\" must be a string or {{kind, text}}",
                    i + 1
                ));
                continue;
            }
        };
        let text = text.strip_suffix('\n').unwrap_or(text);
        for part in text.split('\n') {
            lines.push(Line {
                kind,
                text: truncate(part.strip_suffix('\r').unwrap_or(part), MAX_LINE),
            });
        }
    }
    lines
}

impl Review {
    pub fn parse(spec: &Value) -> Result<Review, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(raw_files) = spec.get("files").and_then(Value::as_array) else {
            return Err(vec!["\"files\" must be an array".into()]);
        };
        let mut errors = Errors::default();
        if raw_files.is_empty() {
            errors.push("a review needs at least one file".into());
        }
        if raw_files.len() > MAX_FILES {
            errors.push(format!(
                "too many files ({}, at most {MAX_FILES})",
                raw_files.len()
            ));
        }
        let raw_hunks: Vec<&Value> = raw_files
            .iter()
            .flat_map(|f| array(f.get("hunks")))
            .collect();
        if raw_hunks.len() > MAX_HUNKS {
            errors.push(format!(
                "too many hunks ({}, at most {MAX_HUNKS})",
                raw_hunks.len()
            ));
        }
        // Given ids first, so a generated one never takes an id a later hunk names.
        let given: Vec<String> = raw_hunks
            .iter()
            .filter_map(|h| h.get("id").and_then(id_of))
            .collect();
        let mut seen: Vec<String> = Vec::new();
        let mut counter = 0;
        let mut total_lines = 0;
        let mut review = Review {
            summary: text(spec.get("summary"), MAX_TEXT),
            files: Vec::new(),
        };
        for (fi, raw) in raw_files.iter().take(MAX_FILES).enumerate() {
            let Some(path) = path_of(raw.get("path")) else {
                errors.push(format!("file {} has no path", fi + 1));
                continue;
            };
            if path.chars().count() > MAX_PATH {
                errors.push(format!(
                    "the path of file {} is longer than {MAX_PATH} characters",
                    fi + 1
                ));
                continue;
            }
            if review.files.iter().any(|f| f.path == path) {
                errors.push(format!("duplicate file \"{path}\""));
                continue;
            }
            let old_path = path_of(raw.get("oldPath")).filter(|old| *old != path);
            let status = match raw.get("status").and_then(Value::as_str) {
                Some(s) => Status::parse(s).unwrap_or(Status::Modified),
                None if old_path.is_some() => Status::Renamed,
                None => Status::Modified,
            };
            if status == Status::Renamed && old_path.is_none() {
                errors.push(format!("renamed file \"{path}\" needs oldPath"));
            }
            if raw.get("hunks").is_some_and(|h| !h.is_array()) {
                errors.push(format!("the hunks of \"{path}\" must be an array"));
            }
            let mut file = File {
                path: path.clone(),
                status,
                old_path: old_path.filter(|_| status == Status::Renamed),
                language: text(raw.get("language"), 40).map(|l| l.to_lowercase()),
                hunks: Vec::new(),
            };
            for raw_hunk in array(raw.get("hunks")) {
                counter += 1;
                let id = match raw_hunk.get("id").and_then(id_of) {
                    Some(id) => id,
                    None => (counter..)
                        .map(|n| format!("h{n}"))
                        .find(|id| !given.contains(id) && !seen.contains(id))
                        .expect("an unused id exists"),
                };
                if seen.contains(&id) {
                    errors.push(format!("duplicate hunk id \"{id}\""));
                    continue;
                }
                seen.push(id.clone());
                let raw_lines = array(raw_hunk.get("lines"));
                let lines = read_lines(raw_lines, &id, &mut errors);
                total_lines += lines.len();
                if lines.len() > MAX_HUNK_LINES {
                    errors.push(format!(
                        "hunk \"{id}\" has too many lines ({}, at most {MAX_HUNK_LINES})",
                        lines.len()
                    ));
                }
                if !lines.iter().any(|l| l.kind != LineKind::Context) {
                    errors.push(format!("hunk \"{id}\" changes nothing"));
                }
                let wrong = match status {
                    Status::Added => lines.iter().any(|l| l.kind != LineKind::Add),
                    Status::Deleted => lines.iter().any(|l| l.kind != LineKind::Del),
                    _ => false,
                };
                if wrong {
                    errors.push(format!(
                        "hunk \"{id}\" of {} file \"{path}\" can only {} lines",
                        status.name(),
                        if status == Status::Added {
                            "add"
                        } else {
                            "remove"
                        }
                    ));
                }
                let (default_old, default_new) = match status {
                    Status::Added => (Some(0), Some(1)),
                    Status::Deleted => (Some(1), Some(0)),
                    _ => (None, None),
                };
                let old_start = line_number(raw_hunk.get("oldStart")).or(default_old);
                let new_start = line_number(raw_hunk.get("newStart")).or(default_new);
                let (Some(old_start), Some(new_start)) = (old_start, new_start) else {
                    errors.push(format!(
                        "hunk \"{id}\" needs oldStart and newStart (1-based line numbers)"
                    ));
                    continue;
                };
                let hunk = Hunk {
                    id: id.clone(),
                    header: text(raw_hunk.get("header"), MAX_HEADER),
                    old_start,
                    new_start,
                    lines,
                    note: text(raw_hunk.get("note"), MAX_TEXT),
                    decision: raw_hunk
                        .get("decision")
                        .and_then(Value::as_str)
                        .and_then(Decision::parse),
                    comment: text(raw_hunk.get("comment"), MAX_TEXT),
                    applied: raw_hunk.get("applied").and_then(Value::as_bool) == Some(true),
                };
                if (hunk.old_count() > 0 && old_start == 0)
                    || (hunk.new_count() > 0 && new_start == 0)
                {
                    errors.push(format!("hunk \"{id}\" starts at line 0"));
                }
                file.hunks.push(hunk);
            }
            // In file order; overlapping hunks can't both apply.
            file.hunks.sort_by_key(|h| (h.old_start, h.new_start));
            for pair in file.hunks.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if b.old_start < a.old_start + a.old_count()
                    || b.new_start < a.new_start + a.new_count()
                {
                    errors.push(format!("hunks \"{}\" and \"{}\" overlap", a.id, b.id));
                }
            }
            if array(raw.get("hunks")).is_empty() && status != Status::Renamed {
                errors.push(format!("file \"{path}\" has no hunks"));
            }
            review.files.push(file);
        }
        if total_lines > MAX_LINES {
            errors.push(format!(
                "too many lines ({total_lines}, at most {MAX_LINES})"
            ));
        }
        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(review)
    }

    pub fn hunks(&self) -> impl Iterator<Item = (&File, &Hunk)> {
        self.files
            .iter()
            .flat_map(|f| f.hunks.iter().map(move |h| (f, h)))
    }

    pub fn hunk_count(&self) -> usize {
        self.files.iter().map(|f| f.hunks.len()).sum()
    }

    pub fn decided(&self) -> usize {
        self.files.iter().map(File::decided).sum()
    }

    pub fn hunk(&self, id: &str) -> Option<&Hunk> {
        self.hunks().map(|(_, h)| h).find(|h| h.id == id)
    }

    fn hunk_mut(&mut self, id: &str) -> Option<&mut Hunk> {
        self.files
            .iter_mut()
            .flat_map(|f| f.hunks.iter_mut())
            .find(|h| h.id == id)
    }

    /// Hunk ids in reading order.
    pub fn hunk_ids(&self) -> Vec<String> {
        self.hunks().map(|(_, h)| h.id.clone()).collect()
    }

    /// "hunk 2 of src/lib.rs" (only "src/lib.rs" for a file's only hunk).
    pub fn hunk_name(&self, id: &str) -> String {
        self.files
            .iter()
            .find_map(|f| {
                let at = f.hunks.iter().position(|h| h.id == id)?;
                Some(if f.hunks.len() == 1 {
                    f.path.clone()
                } else {
                    format!("hunk {} of {}", at + 1, f.path)
                })
            })
            .unwrap_or_else(|| format!("hunk \"{id}\""))
    }

    /// "5 of 9 hunks decided".
    pub fn progress(&self) -> String {
        self.progress_in(crate::i18n::Language::English)
    }

    pub fn progress_in(&self, language: crate::i18n::Language) -> String {
        crate::i18n::tr(language, "{decided} of {hunks} decided")
            .replace("{decided}", &self.decided().to_string())
            .replace(
                "{hunks}",
                &super::edits::count_in(
                    language,
                    self.hunk_count(),
                    "{count} hunk",
                    "{count} hunks",
                ),
            )
    }

    /// The header's counts: files, lines added and removed, progress.
    pub fn stats(&self) -> String {
        self.stats_in(crate::i18n::Language::English)
    }

    pub fn stats_in(&self, language: crate::i18n::Language) -> String {
        let (add, del): (usize, usize) = self
            .files
            .iter()
            .map(|f| (f.additions(), f.deletions()))
            .fold((0, 0), |(a, d), (fa, fd)| (a + fa, d + fd));
        format!(
            "{} · +{add} −{del} · {}",
            super::edits::count_in(language, self.files.len(), "{count} file", "{count} files"),
            self.progress_in(language)
        )
    }

    /// The same files and hunks, ignoring decisions, comments and applied
    /// marks (whether a diff viewer needs new content).
    pub fn same_content(&self, other: &Review) -> bool {
        self.files.len() == other.files.len()
            && self.files.iter().zip(&other.files).all(|(a, b)| {
                a.path == b.path
                    && a.status == b.status
                    && a.old_path == b.old_path
                    && a.language == b.language
                    && a.hunks.len() == b.hunks.len()
                    && a.hunks.iter().zip(&b.hunks).all(|(x, y)| {
                        x.id == y.id
                            && x.header == y.header
                            && x.old_start == y.old_start
                            && x.new_start == y.new_start
                            && x.lines == y.lines
                    })
            })
    }
}

/// A trimmed comment, cut to [`MAX_TEXT`]; empty is none.
fn clean_comment(comment: Option<&str>) -> Option<String> {
    comment
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| truncate(c, MAX_TEXT))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// One hunk's decision (`None`: undecided) and comment as they are now:
    /// on the wire, one entry of the decisions the agent gets.
    Decide {
        hunk: String,
        decision: Option<Decision>,
        comment: Option<String>,
    },
    /// Accept all / Reject all: every listed hunk not applied yet gets
    /// `decision`; comments stay. Local only (the diff lists each hunk).
    DecideAll {
        hunks: Vec<String>,
        decision: Option<Decision>,
    },
}

impl Editable for Review {
    type Change = Change;

    /// One `Decide` per hunk whose decision or comment changed, in order.
    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        new.hunks()
            .filter_map(|(_, hunk)| {
                let before = old.hunk(&hunk.id)?;
                (before.decision != hunk.decision || before.comment != hunk.comment).then(|| {
                    Change::Decide {
                        hunk: hunk.id.clone(),
                        decision: hunk.decision,
                        comment: hunk.comment.clone(),
                    }
                })
            })
            .collect()
    }

    fn apply(&mut self, change: &Change) -> Result<(), String> {
        match change {
            Change::Decide {
                hunk,
                decision,
                comment,
            } => {
                let name = self.hunk_name(hunk);
                let h = self
                    .hunk_mut(hunk)
                    .ok_or_else(|| format!("unknown hunk \"{hunk}\""))?;
                if h.applied {
                    return Err(format!("{name} is already applied"));
                }
                h.decision = *decision;
                h.comment = clean_comment(comment.as_deref());
            }
            Change::DecideAll { hunks, decision } => {
                if let Some(missing) = hunks.iter().find(|id| self.hunk(id).is_none()) {
                    return Err(format!("unknown hunk \"{missing}\""));
                }
                for id in hunks {
                    if let Some(h) = self.hunk_mut(id).filter(|h| !h.applied) {
                        h.decision = *decision;
                    }
                }
            }
        }
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        match change {
            Change::Decide {
                hunk,
                decision,
                comment,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("decide"));
                map.insert("hunk".into(), json!(hunk));
                map.insert("decision".into(), json!(decision.map(Decision::name)));
                if let Some(comment) = comment {
                    map.insert("comment".into(), json!(comment));
                }
                Value::Object(map)
            }
            Change::DecideAll { hunks, decision } => json!({
                "op": "decide_all",
                "hunks": hunks,
                "decision": decision.map(Decision::name),
            }),
        }
    }

    fn describe(change: &Change, before: &Self, after: &Self) -> String {
        match change {
            Change::Decide {
                hunk,
                decision,
                comment,
            } => {
                let name = after.hunk_name(hunk);
                let old = before.hunk(hunk);
                let mut parts = Vec::new();
                if old.map(|h| h.decision) != Some(*decision) {
                    parts.push(match decision {
                        Some(Decision::Accepted) => format!("accepted {name}"),
                        Some(Decision::Rejected) => format!("rejected {name}"),
                        None => format!("left {name} undecided"),
                    });
                }
                let comment = clean_comment(comment.as_deref());
                if old.map(|h| &h.comment) != Some(&comment) {
                    match (&comment, parts.is_empty()) {
                        (Some(c), true) => {
                            parts.push(format!("commented on {name}: “{}”", truncate(c, 80)))
                        }
                        (Some(c), false) => parts.push(format!("commenting “{}”", truncate(c, 80))),
                        (None, _) => parts.push(format!("removed the comment on {name}")),
                    }
                }
                parts.join(", ")
            }
            Change::DecideAll { hunks, decision } => {
                let verb = match decision {
                    Some(Decision::Accepted) => "accepted",
                    Some(Decision::Rejected) => "rejected",
                    None => "cleared the decisions on",
                };
                let names: Vec<String> = hunks.iter().map(|id| after.hunk_name(id)).collect();
                format!("{verb} {}", list_names(&names, 5))
            }
        }
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        if let Some(summary) = &self.summary {
            map.insert("summary".into(), json!(summary));
        }
        map.insert(
            "files".into(),
            self.files.iter().map(File::to_value).collect(),
        );
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn sample() -> Review {
        Review::parse(&json!({
            "summary": "Rename the login handler",
            "files": [
                {"path": "src/auth.rs", "hunks": [
                    {"id": "sig", "header": "fn login", "oldStart": 10, "newStart": 10, "lines": [
                        " fn main() {",
                        "-    login(user);",
                        "+    sign_in(user);",
                        " }"
                    ]},
                    {"oldStart": 40, "newStart": 40, "lines": [
                        {"kind": "del", "text": "old()"},
                        {"kind": "add", "text": "new()\nmore()"}
                    ], "note": "keeps the API"}
                ]},
                {"path": "./docs/auth.md", "status": "added", "hunks": [
                    {"lines": ["+# Auth"]}
                ]}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn parses_hunks_lines_and_defaults() {
        let review = sample();
        assert_eq!(review.files.len(), 2);
        let auth = &review.files[0];
        assert_eq!(auth.status, Status::Modified);
        assert_eq!(auth.hunks[1].id, "h2");
        assert_eq!(auth.hunks[1].lines.len(), 3, "text with a newline splits");
        assert_eq!((auth.additions(), auth.deletions()), (3, 2));
        let docs = &review.files[1];
        assert_eq!(docs.path, "docs/auth.md");
        assert_eq!((docs.hunks[0].old_start, docs.hunks[0].new_start), (0, 1));
        assert_eq!(review.hunk_ids(), vec!["sig", "h2", "h3"]);
        assert_eq!(review.stats(), "2 files · +4 −2 · 0 of 3 hunks decided");
        assert_eq!(Review::parse(&review.to_value()).unwrap(), review);
    }

    #[test]
    fn readable_errors() {
        let errors = Review::parse(&json!({"files": [
            {"path": "a.rs", "hunks": [{"id": "x", "lines": ["+a"]}]},
            {"path": "a.rs", "hunks": []},
            {"hunks": []},
            {"path": "b.rs", "hunks": [
                {"id": "x", "oldStart": 1, "newStart": 1, "lines": [" same"]},
            ]},
            {"path": "c.rs", "status": "added", "hunks": [{"id": "c1", "oldStart": 1, "newStart": 1, "lines": [" ctx", "+new"]}]},
            {"path": "d.rs", "status": "renamed", "hunks": []},
            {"path": "e.rs", "hunks": [
                {"id": "e1", "oldStart": 1, "newStart": 1, "lines": ["-a", "-b", "+c"]},
                {"id": "e2", "oldStart": 2, "newStart": 2, "lines": ["-b"]}
            ]}
        ]}))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "hunk \"x\" needs oldStart and newStart (1-based line numbers)",
                "duplicate file \"a.rs\"",
                "file 3 has no path",
                "duplicate hunk id \"x\"",
                "hunk \"c1\" of added file \"c.rs\" can only add lines",
                "renamed file \"d.rs\" needs oldPath",
                "hunks \"e1\" and \"e2\" overlap",
            ]
        );
        assert_eq!(
            Review::parse(&json!({"files": []})).unwrap_err(),
            vec!["a review needs at least one file"]
        );
    }

    #[test]
    fn positions_follow_a_unified_view() {
        let review = sample();
        let sig = review.hunk("sig").unwrap();
        assert_eq!(
            sig.positions(),
            vec![
                (Side::New, 10),
                (Side::Old, 11),
                (Side::New, 11),
                (Side::New, 12)
            ]
        );
        assert_eq!(sig.last_shown(), (Side::New, 12));
        assert_eq!(sig.first_shown(), (Side::New, 10));
        // Ends in changes: removed lines show before added ones.
        let h2 = review.hunk("h2").unwrap();
        assert_eq!(h2.first_shown(), (Side::Old, 40));
        assert_eq!(h2.last_shown(), (Side::New, 41));
    }

    #[test]
    fn patch_is_a_git_diff() {
        let review = sample();
        assert_eq!(
            review.files[0].patch(),
            "diff --git a/src/auth.rs b/src/auth.rs\n--- a/src/auth.rs\n+++ b/src/auth.rs\n\
             @@ -10,3 +10,3 @@ fn login\n fn main() {\n-    login(user);\n+    sign_in(user);\n }\n\
             @@ -40,1 +40,2 @@\n-old()\n+new()\n+more()\n"
        );
        assert!(review.files[1].patch().contains(
            "new file mode 100644\n--- /dev/null\n+++ b/docs/auth.md\n@@ -0,0 +1,1 @@\n+# Auth\n"
        ));
    }

    #[test]
    fn decisions_describe_and_round_trip() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        edits
            .edit(&Change::DecideAll {
                hunks: base.hunk_ids(),
                decision: Some(Decision::Accepted),
            })
            .unwrap();
        edits
            .edit(&Change::Decide {
                hunk: "h2".into(),
                decision: Some(Decision::Rejected),
                comment: Some("  keep old() for now ".into()),
            })
            .unwrap();
        edits
            .edit(&Change::Decide {
                hunk: "h3".into(),
                decision: None,
                comment: Some("why a new doc?".into()),
            })
            .unwrap();
        assert_eq!(edits.working().progress(), "2 of 3 hunks decided");
        let (values, lines) = edits.summarize();
        assert_eq!(
            values,
            vec![
                json!({"op": "decide", "hunk": "sig", "decision": "accepted"}),
                json!({"op": "decide", "hunk": "h2", "decision": "rejected", "comment": "keep old() for now"}),
                json!({"op": "decide", "hunk": "h3", "decision": null, "comment": "why a new doc?"}),
            ]
        );
        assert_eq!(
            lines,
            vec![
                "accepted hunk 1 of src/auth.rs",
                "rejected hunk 2 of src/auth.rs, commenting “keep old() for now”",
                "commented on docs/auth.md: “why a new doc?”",
            ]
        );
        let mut replay = base.clone();
        for change in Review::diff(&base, edits.working()) {
            replay.apply(&change).unwrap();
        }
        assert_eq!(&replay, edits.working());
    }

    #[test]
    fn applied_hunks_are_final() {
        let mut spec = sample().to_value();
        spec["files"][0]["hunks"][0]["applied"] = json!(true);
        spec["files"][0]["hunks"][0]["decision"] = json!("accepted");
        let mut review = Review::parse(&spec).unwrap();
        assert_eq!(
            review
                .apply(&Change::Decide {
                    hunk: "sig".into(),
                    decision: Some(Decision::Rejected),
                    comment: None
                })
                .unwrap_err(),
            "hunk 1 of src/auth.rs is already applied"
        );
        review
            .apply(&Change::DecideAll {
                hunks: review.hunk_ids(),
                decision: Some(Decision::Rejected),
            })
            .unwrap();
        assert_eq!(
            review.hunk("sig").unwrap().decision,
            Some(Decision::Accepted)
        );
        assert_eq!(
            review.hunk("h2").unwrap().decision,
            Some(Decision::Rejected)
        );
        assert!(review.same_content(&sample()));
    }
}
