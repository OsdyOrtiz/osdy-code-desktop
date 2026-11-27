//! The timeline kind: a Gantt chart. Tasks with start and end dates (or a
//! duration), progress, groups, dependencies and milestones on a date
//! axis. The user drags tasks to move or resize them, links dependencies,
//! renames, adds and removes tasks; the critical path and dependencies
//! that start too early are worked out here.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use super::diagram::{Errors, MAX_LABEL, array, id_of, text, truncate};
use super::edits::{Editable, fresh_id, list_names, plural};
use super::form::is_date;
use super::layout::{self, EdgePath, Point, Rect};
use super::style::Tint;
use crate::usage::time::{civil_from_days, days_from_civil};

pub const MAX_TASKS: usize = 200;
/// The longest a task (or the whole timeline) may run, in days.
pub const MAX_DAYS: i64 = 3660;

/// Days since 1970-01-01 of `YYYY-MM-DD` (a time after it is ignored).
pub fn day_of(s: &str) -> Option<i64> {
    let date = s.trim().get(..10)?;
    if !is_date(date) {
        return None;
    }
    let part = |r: std::ops::Range<usize>| date[r].parse::<i64>().ok();
    Some(days_from_civil(part(0..4)?, part(5..7)?, part(8..10)?))
}

/// `YYYY-MM-DD` of a day number.
pub fn date_of(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "Mar 4".
pub fn short_date(day: i64) -> String {
    short_date_in(day, crate::i18n::Language::English)
}

pub fn short_date_in(day: i64, language: crate::i18n::Language) -> String {
    use crate::i18n::tr;
    let (_, m, d) = civil_from_days(day);
    tr(language, "{month} {day}")
        .replace("{month}", tr(language, MONTHS[(m - 1) as usize]))
        .replace("{day}", &d.to_string())
}

/// "Mar 2026".
pub fn month_label(day: i64) -> String {
    month_label_in(day, crate::i18n::Language::English)
}

pub fn month_label_in(day: i64, language: crate::i18n::Language) -> String {
    let (y, m, _) = civil_from_days(day);
    format!(
        "{} {y}",
        crate::i18n::tr(language, MONTHS[(m - 1) as usize])
    )
}

/// The day of the month (1-based) and whether `day` is a Monday.
pub fn day_parts(day: i64) -> (u32, bool) {
    // 1970-01-01 was a Thursday.
    (civil_from_days(day).2, (day + 3).rem_euclid(7) == 0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    pub label: String,
    /// Day numbers; `end` is the last day (inclusive).
    pub start: i64,
    pub end: i64,
    /// Percent done.
    pub progress: Option<u8>,
    pub group: Option<String>,
    pub depends_on: Vec<String>,
    /// A point in time (`end == start`), drawn as a diamond.
    pub milestone: bool,
    /// The bar's color (`color`); its group's otherwise.
    pub color: Option<Tint>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    pub tasks: Vec<Task>,
}

impl Task {
    pub fn new(id: impl Into<String>, label: impl Into<String>, start: i64, end: i64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            start,
            end,
            progress: None,
            group: None,
            depends_on: Vec::new(),
            milestone: false,
            color: None,
        }
    }

    pub fn days(&self) -> i64 {
        self.end - self.start + 1
    }

    fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("label".into(), json!(self.label));
        map.insert("start".into(), json!(date_of(self.start)));
        map.insert("end".into(), json!(date_of(self.end)));
        if let Some(progress) = self.progress {
            map.insert("progress".into(), json!(progress));
        }
        if let Some(group) = &self.group {
            map.insert("group".into(), json!(group));
        }
        if !self.depends_on.is_empty() {
            map.insert("dependsOn".into(), json!(self.depends_on));
        }
        if self.milestone {
            map.insert("milestone".into(), json!(true));
        }
        if let Some(color) = self.color {
            map.insert("color".into(), json!(color.name()));
        }
        Value::Object(map)
    }
}

/// 0–1 is a fraction, above that a percent; clamped to 0–100.
fn percent(value: &Value) -> Option<u8> {
    let n = super::edits::number(value)?;
    let n = if n <= 1. { n * 100. } else { n };
    Some(n.clamp(0., 100.).round() as u8)
}

impl Timeline {
    pub fn parse(spec: &Value) -> Result<Timeline, Vec<String>> {
        let Some(spec) = spec.as_object() else {
            return Err(vec!["the spec must be a JSON object".into()]);
        };
        let Some(raw_tasks) = spec.get("tasks").and_then(Value::as_array) else {
            return Err(vec!["\"tasks\" must be an array".into()]);
        };
        let mut errors = Errors::default();
        if raw_tasks.len() > MAX_TASKS {
            errors.push(format!(
                "too many tasks ({}, at most {MAX_TASKS})",
                raw_tasks.len()
            ));
        }
        let mut timeline = Timeline::default();
        let mut deps: Vec<Vec<String>> = Vec::new();
        for (i, raw) in raw_tasks.iter().take(MAX_TASKS).enumerate() {
            let label = text(raw.get("label").or_else(|| raw.get("name")), MAX_LABEL);
            let id = raw.get("id").and_then(id_of);
            let Some(label) = label.or_else(|| id.clone()) else {
                errors.push(format!("task #{} has no label", i + 1));
                continue;
            };
            let id = id.unwrap_or_else(|| timeline.fresh_task_id(&label));
            if timeline.task(&id).is_some() {
                errors.push(format!("duplicate task id \"{id}\""));
                continue;
            }
            let date = |key: &str| raw.get(key).and_then(Value::as_str).map(|s| (s, day_of(s)));
            let Some((given, start)) = date("start") else {
                errors.push(format!("task \"{id}\" needs a \"start\" date (YYYY-MM-DD)"));
                continue;
            };
            let Some(start) = start else {
                errors.push(format!(
                    "task \"{id}\": start \"{given}\" is not a date (YYYY-MM-DD)"
                ));
                continue;
            };
            let milestone = raw.get("milestone").and_then(Value::as_bool) == Some(true);
            let duration = raw
                .get("duration")
                .and_then(super::edits::number)
                .map(|d| d.round() as i64);
            let end = match (date("end"), duration) {
                _ if milestone => start,
                (Some((_, Some(end))), _) => end,
                (Some((given, None)), _) => {
                    errors.push(format!(
                        "task \"{id}\": end \"{given}\" is not a date (YYYY-MM-DD)"
                    ));
                    continue;
                }
                (None, Some(days)) if days >= 1 => start + days - 1,
                (None, Some(_)) => {
                    errors.push(format!("task \"{id}\": duration must be at least 1 day"));
                    continue;
                }
                (None, None) => {
                    errors.push(format!(
                        "task \"{id}\" needs an \"end\" date or a \"duration\" in days"
                    ));
                    continue;
                }
            };
            if end < start {
                errors.push(format!("task \"{id}\" ends before it starts"));
                continue;
            }
            if end - start >= MAX_DAYS {
                errors.push(format!("task \"{id}\" is longer than {MAX_DAYS} days"));
                continue;
            }
            let wanted = raw
                .get("dependsOn")
                .or_else(|| raw.get("depends_on"))
                .or_else(|| raw.get("after"));
            let wanted: Vec<String> = match wanted {
                Some(Value::Array(_)) => array(wanted).iter().filter_map(id_of).collect(),
                Some(single) => id_of(single).into_iter().collect(),
                None => Vec::new(),
            };
            deps.push(wanted);
            timeline.tasks.push(Task {
                label,
                start,
                end,
                progress: raw.get("progress").and_then(percent),
                group: text(raw.get("group"), MAX_LABEL),
                depends_on: Vec::new(),
                milestone,
                color: Tint::field(raw),
                id,
            });
        }
        for (i, wanted) in deps.into_iter().enumerate() {
            let id = timeline.tasks[i].id.clone();
            let mut list: Vec<String> = Vec::new();
            for dep in wanted {
                if dep == id {
                    errors.push(format!("task \"{id}\" can't depend on itself"));
                } else if timeline.task(&dep).is_none() {
                    errors.push(format!(
                        "task \"{id}\": dependency \"{dep}\" does not exist"
                    ));
                } else if !list.contains(&dep) {
                    list.push(dep);
                }
            }
            timeline.tasks[i].depends_on = list;
        }
        if errors.0.is_empty()
            && let Some(cycle) = timeline.cycle()
        {
            errors.push(format!("dependency cycle: {}", cycle.join(" → ")));
        }
        let first = timeline.tasks.iter().map(|t| t.start).min();
        let last = timeline.tasks.iter().map(|t| t.end).max();
        if let (Some(first), Some(last)) = (first, last)
            && last - first >= MAX_DAYS
        {
            errors.push(format!("the timeline spans more than {MAX_DAYS} days"));
        }
        if !errors.0.is_empty() {
            return Err(errors.0);
        }
        Ok(timeline)
    }

    pub fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    fn task_mut(&mut self, id: &str) -> Result<&mut Task, String> {
        self.tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("unknown task \"{id}\""))
    }

    pub fn fresh_task_id(&self, label: &str) -> String {
        fresh_id(label, "task", |id| self.task(id).is_some())
    }

    /// A dependency loop, as ids from and back to its first task.
    fn cycle(&self) -> Option<Vec<String>> {
        // 0 unvisited, 1 on the path, 2 done.
        let mut mark: HashMap<&str, u8> = HashMap::new();
        fn visit<'a>(
            t: &'a Timeline,
            id: &'a str,
            mark: &mut HashMap<&'a str, u8>,
            path: &mut Vec<&'a str>,
        ) -> Option<Vec<String>> {
            match mark.get(id) {
                Some(2) => return None,
                Some(1) => {
                    let from = path.iter().position(|p| *p == id).unwrap_or(0);
                    let mut cycle: Vec<String> =
                        path[from..].iter().map(|s| s.to_string()).collect();
                    cycle.push(id.to_string());
                    return Some(cycle);
                }
                _ => {}
            }
            mark.insert(id, 1);
            path.push(id);
            for dep in &t.task(id)?.depends_on {
                if let Some(cycle) = visit(t, dep, mark, path) {
                    return Some(cycle);
                }
            }
            path.pop();
            mark.insert(id, 2);
            None
        }
        let mut path = Vec::new();
        self.tasks
            .iter()
            .find_map(|task| visit(self, &task.id, &mut mark, &mut path))
    }

    /// Group names in order of first appearance (`None`: ungrouped).
    pub fn groups(&self) -> Vec<Option<&str>> {
        let mut out: Vec<Option<&str>> = Vec::new();
        for t in &self.tasks {
            let g = t.group.as_deref();
            if !out.contains(&g) {
                out.push(g);
            }
        }
        out
    }

    /// Dependencies a task starts before they finish: (dependency, task).
    pub fn late(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (ti, t) in self.tasks.iter().enumerate() {
            for dep in &t.depends_on {
                let Some(di) = self.index(dep) else { continue };
                let d = &self.tasks[di];
                let ready = d.end + i64::from(!d.milestone);
                if t.start < ready {
                    out.push((di, ti));
                }
            }
        }
        out
    }

    /// The chain of dependencies that ends last: from the task finishing
    /// last back through the dependency finishing last each time.
    pub fn critical_path(&self) -> Vec<String> {
        let Some(mut at) = self
            .tasks
            .iter()
            .enumerate()
            .max_by_key(|(i, t)| (t.end, std::cmp::Reverse(*i)))
            .map(|(i, _)| i)
        else {
            return Vec::new();
        };
        let mut chain = vec![self.tasks[at].id.clone()];
        while let Some(prev) = self.tasks[at]
            .depends_on
            .iter()
            .filter_map(|d| self.index(d))
            .max_by_key(|&i| (self.tasks[i].end, std::cmp::Reverse(i)))
        {
            if chain.contains(&self.tasks[prev].id) {
                break;
            }
            chain.push(self.tasks[prev].id.clone());
            at = prev;
        }
        chain.reverse();
        chain
    }

    /// Whether `task` may depend on `on` (no loop through it).
    pub fn may_depend(&self, task: &str, on: &str) -> bool {
        if task == on {
            return false;
        }
        let mut seen = vec![on.to_string()];
        let mut stack = vec![on.to_string()];
        while let Some(id) = stack.pop() {
            if id == task {
                return false;
            }
            for dep in self
                .task(&id)
                .map(|t| t.depends_on.clone())
                .unwrap_or_default()
            {
                if !seen.contains(&dep) {
                    seen.push(dep.clone());
                    stack.push(dep);
                }
            }
        }
        true
    }

    fn name(&self, id: &str) -> String {
        self.task(id).map_or_else(
            || id.to_string(),
            |t| format!("“{}”", truncate(&t.label, 60)),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Day numbers (dates on the wire); `end` inclusive.
    SetDates {
        task: String,
        start: i64,
        end: i64,
    },
    /// Appended, without dependencies (they come as `AddDependency`).
    AddTask {
        task: Task,
    },
    /// Others stop depending on it.
    RemoveTask {
        task: String,
    },
    /// `None` = unchanged; `Some(None)` clears.
    UpdateTask {
        task: String,
        label: Option<String>,
        progress: Option<Option<u8>>,
        group: Option<Option<String>>,
    },
    AddDependency {
        task: String,
        on: String,
    },
    RemoveDependency {
        task: String,
        on: String,
    },
}

impl Change {
    pub fn rename(task: &str, label: String) -> Change {
        Change::UpdateTask {
            task: task.into(),
            label: Some(label),
            progress: None,
            group: None,
        }
    }
}

impl Editable for Timeline {
    type Change = Change;

    /// Removed dependencies and tasks, added tasks, dates, other fields,
    /// then added dependencies.
    fn diff(old: &Self, new: &Self) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut sim = old.clone();
        let mut push = |change: Change, sim: &mut Timeline| {
            let _ = sim.apply(&change);
            changes.push(change);
        };
        for t in &old.tasks {
            let Some(now) = new.task(&t.id) else {
                continue;
            };
            for dep in &t.depends_on {
                if !now.depends_on.contains(dep) && new.task(dep).is_some() {
                    push(
                        Change::RemoveDependency {
                            task: t.id.clone(),
                            on: dep.clone(),
                        },
                        &mut sim,
                    );
                }
            }
        }
        for t in &old.tasks {
            if new.task(&t.id).is_none() {
                push(Change::RemoveTask { task: t.id.clone() }, &mut sim);
            }
        }
        for t in &new.tasks {
            let Some(was) = sim.task(&t.id).cloned() else {
                let task = Task {
                    depends_on: Vec::new(),
                    ..t.clone()
                };
                push(Change::AddTask { task }, &mut sim);
                continue;
            };
            if (was.start, was.end) != (t.start, t.end) {
                push(
                    Change::SetDates {
                        task: t.id.clone(),
                        start: t.start,
                        end: t.end,
                    },
                    &mut sim,
                );
            }
            if was.label != t.label || was.progress != t.progress || was.group != t.group {
                push(
                    Change::UpdateTask {
                        task: t.id.clone(),
                        label: (was.label != t.label).then(|| t.label.clone()),
                        progress: (was.progress != t.progress).then_some(t.progress),
                        group: (was.group != t.group).then(|| t.group.clone()),
                    },
                    &mut sim,
                );
            }
        }
        for t in &new.tasks {
            for dep in &t.depends_on {
                let has = sim.task(&t.id).is_some_and(|s| s.depends_on.contains(dep));
                if !has {
                    push(
                        Change::AddDependency {
                            task: t.id.clone(),
                            on: dep.clone(),
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
            Change::SetDates { task, start, end } => {
                let t = self.task_mut(task)?;
                let end = if t.milestone { *start } else { *end };
                if end < *start {
                    return Err("a task can't end before it starts".into());
                }
                if end - start >= MAX_DAYS {
                    return Err(format!("a task can't run longer than {MAX_DAYS} days"));
                }
                t.start = *start;
                t.end = end;
            }
            Change::AddTask { task } => {
                if task.id.trim().is_empty() || task.label.trim().is_empty() {
                    return Err("a new task needs an id and a label".into());
                }
                if self.task(&task.id).is_some() {
                    return Err(format!("task \"{}\" already exists", task.id));
                }
                if task.end < task.start {
                    return Err("a task can't end before it starts".into());
                }
                if self.tasks.len() >= MAX_TASKS {
                    return Err(format!("too many tasks (at most {MAX_TASKS})"));
                }
                if let Some(missing) = task.depends_on.iter().find(|d| self.task(d).is_none()) {
                    return Err(format!("unknown task \"{missing}\""));
                }
                self.tasks.push(task.clone());
            }
            Change::RemoveTask { task } => {
                let at = self
                    .index(task)
                    .ok_or_else(|| format!("unknown task \"{task}\""))?;
                self.tasks.remove(at);
                for t in &mut self.tasks {
                    t.depends_on.retain(|d| d != task);
                }
            }
            Change::UpdateTask {
                task,
                label,
                progress,
                group,
            } => {
                let t = self.task_mut(task)?;
                if let Some(label) = label {
                    let label = label.trim();
                    if label.is_empty() {
                        return Err("a task needs a label".into());
                    }
                    t.label = truncate(label, MAX_LABEL);
                }
                if let Some(progress) = progress {
                    t.progress = progress.map(|p| p.min(100));
                }
                if let Some(group) = group {
                    t.group = group
                        .as_deref()
                        .map(str::trim)
                        .filter(|g| !g.is_empty())
                        .map(|g| truncate(g, MAX_LABEL));
                }
            }
            Change::AddDependency { task, on } => {
                if self.task(on).is_none() {
                    return Err(format!("unknown task \"{on}\""));
                }
                if !self.may_depend(task, on) {
                    return Err(format!(
                        "{} can't wait for {}: that would make a loop",
                        self.name(task),
                        self.name(on)
                    ));
                }
                let t = self.task_mut(task)?;
                if !t.depends_on.contains(on) {
                    t.depends_on.push(on.clone());
                }
            }
            Change::RemoveDependency { task, on } => {
                let t = self.task_mut(task)?;
                let at = t
                    .depends_on
                    .iter()
                    .position(|d| d == on)
                    .ok_or_else(|| format!("\"{task}\" doesn't depend on \"{on}\""))?;
                t.depends_on.remove(at);
            }
        }
        Ok(())
    }

    fn change_value(change: &Change) -> Value {
        match change {
            Change::SetDates { task, start, end } => json!({
                "op": "set_dates", "task": task, "start": date_of(*start), "end": date_of(*end),
            }),
            Change::AddTask { task } => json!({"op": "add_task", "task": task.to_value()}),
            Change::RemoveTask { task } => json!({"op": "remove_task", "task": task}),
            Change::UpdateTask {
                task,
                label,
                progress,
                group,
            } => {
                let mut map = Map::new();
                map.insert("op".into(), json!("update_task"));
                map.insert("task".into(), json!(task));
                if let Some(label) = label {
                    map.insert("label".into(), json!(label));
                }
                if let Some(progress) = progress {
                    map.insert("progress".into(), json!(progress));
                }
                if let Some(group) = group {
                    map.insert("group".into(), json!(group));
                }
                Value::Object(map)
            }
            Change::AddDependency { task, on } => {
                json!({"op": "add_dependency", "task": task, "on": on})
            }
            Change::RemoveDependency { task, on } => {
                json!({"op": "remove_dependency", "task": task, "on": on})
            }
        }
    }

    fn describe(change: &Change, before: &Self, after: &Self) -> String {
        let span = |t: &Task| {
            if t.start == t.end {
                short_date(t.start)
            } else {
                format!("{} – {}", short_date(t.start), short_date(t.end))
            }
        };
        match change {
            Change::SetDates { task, .. } => {
                let (Some(was), Some(now)) = (before.task(task), after.task(task)) else {
                    return format!("changed the dates of {task}");
                };
                let name = after.name(task);
                if was.days() == now.days() {
                    let by = now.start - was.start;
                    let days = plural(by.unsigned_abs() as usize, "day");
                    let way = if by > 0 { "later" } else { "earlier" };
                    format!("moved {name} {days} {way} ({})", span(now))
                } else {
                    format!(
                        "made {name} {} ({})",
                        plural(now.days() as usize, "day"),
                        span(now)
                    )
                }
            }
            Change::AddTask { task } => {
                let what = if task.milestone { "milestone" } else { "task" };
                format!("added {what} “{}” ({})", task.label, span(task))
            }
            Change::RemoveTask { task } => {
                let waiting: Vec<String> = before
                    .tasks
                    .iter()
                    .filter(|t| t.depends_on.contains(task))
                    .map(|t| before.name(&t.id))
                    .collect();
                if waiting.is_empty() {
                    format!("removed {}", before.name(task))
                } else {
                    format!(
                        "removed {} ({} no longer waits for it)",
                        before.name(task),
                        list_names(&waiting, 3)
                    )
                }
            }
            Change::UpdateTask {
                task,
                label,
                progress,
                group,
            } => {
                let mut parts = Vec::new();
                if let Some(label) = label {
                    parts.push(format!("renamed {} to “{label}”", before.name(task)));
                }
                if let Some(progress) = progress {
                    parts.push(match progress {
                        Some(p) => format!("set {} to {p}% done", after.name(task)),
                        None => format!("cleared the progress of {}", after.name(task)),
                    });
                }
                if let Some(group) = group {
                    parts.push(match group {
                        Some(g) => format!("moved {} to {g}", after.name(task)),
                        None => format!("took {} out of its group", after.name(task)),
                    });
                }
                parts.join(", ")
            }
            Change::AddDependency { task, on } => {
                format!("made {} wait for {}", after.name(task), after.name(on))
            }
            Change::RemoveDependency { task, on } => {
                format!(
                    "{} no longer waits for {}",
                    after.name(task),
                    after.name(on)
                )
            }
        }
    }

    fn to_value(&self) -> Value {
        json!({"tasks": self.tasks.iter().map(Task::to_value).collect::<Vec<_>>()})
    }
}

// ── Layout ─────────────────────────────────────────────────────────────────

/// One day's width, a task row's and a group header's height.
pub const DAY: f32 = 26.;
pub const ROW: f32 = 36.;
pub const BAR_H: f32 = 20.;
pub const GROUP_ROW: f32 = 30.;
/// The task names' column, left of the first day.
pub const LABEL_W: f32 = 190.;
/// Days shown before the first task and after the last.
const MARGIN_DAYS: i64 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineLayout {
    /// The day drawn at x = 0.
    pub origin: i64,
    /// The last day shown.
    pub last: i64,
    /// Each task's bar (a milestone's diamond box), index-aligned.
    pub bars: Vec<Rect>,
    /// Each task's row band (labels and picking), index-aligned.
    pub rows: Vec<Rect>,
    /// Group headers: name and band.
    pub groups: Vec<(String, Rect)>,
    /// (dependency, task, path from its end to the task's start).
    pub links: Vec<(usize, usize, EdgePath)>,
    pub bounds: Rect,
}

impl TimelineLayout {
    pub fn x_of(&self, day: i64) -> f32 {
        (day - self.origin) as f32 * DAY
    }

    /// The day at `x` (the nearest day boundary when `round`).
    pub fn day_at(&self, x: f32, round: bool) -> i64 {
        let days = x / DAY;
        self.origin + if round { days.round() } else { days.floor() } as i64
    }

    pub fn task_at(&self, p: Point) -> Option<usize> {
        self.bars
            .iter()
            .position(|r| r.inflate(3.).contains(p))
            .or_else(|| self.rows.iter().position(|r| r.contains(p)))
    }
}

pub fn layout(timeline: &Timeline) -> TimelineLayout {
    layout_from(timeline, None)
}

/// [`layout`] with day `origin` at x = 0 (a drag keeps the axis still).
pub fn layout_from(timeline: &Timeline, origin: Option<i64>) -> TimelineLayout {
    let first = timeline.tasks.iter().map(|t| t.start).min().unwrap_or(0);
    let last = timeline
        .tasks
        .iter()
        .map(|t| t.end)
        .max()
        .unwrap_or(first + 14);
    let origin = origin.unwrap_or(first - MARGIN_DAYS);
    let last = last + MARGIN_DAYS + 1;
    let x_of = |day: i64| (day - origin) as f32 * DAY;
    let width = x_of(last + 1);
    let mut bars = vec![Rect::default(); timeline.tasks.len()];
    let mut rows = vec![Rect::default(); timeline.tasks.len()];
    let mut groups = Vec::new();
    let mut y = 0.;
    for group in timeline.groups() {
        if let Some(name) = group {
            groups.push((
                name.to_string(),
                Rect {
                    x: -LABEL_W,
                    y,
                    w: LABEL_W + width,
                    h: GROUP_ROW,
                },
            ));
            y += GROUP_ROW;
        }
        for (i, t) in timeline.tasks.iter().enumerate() {
            if t.group.as_deref() != group {
                continue;
            }
            rows[i] = Rect {
                x: -LABEL_W,
                y,
                w: LABEL_W + width,
                h: ROW,
            };
            let mid = y + ROW / 2.;
            bars[i] = if t.milestone {
                Rect {
                    x: x_of(t.start) + DAY / 2. - BAR_H / 2.,
                    y: mid - BAR_H / 2.,
                    w: BAR_H,
                    h: BAR_H,
                }
            } else {
                Rect {
                    x: x_of(t.start),
                    y: mid - BAR_H / 2.,
                    w: t.days() as f32 * DAY,
                    h: BAR_H,
                }
            };
            y += ROW;
        }
    }
    let links = timeline
        .tasks
        .iter()
        .enumerate()
        .flat_map(|(ti, t)| {
            t.depends_on
                .iter()
                .filter_map(|d| timeline.index(d))
                .map(move |di| (di, ti))
        })
        .map(|(di, ti)| (di, ti, link(&bars[di], &bars[ti])))
        .collect();
    let bounds = Rect {
        x: -LABEL_W,
        y: 0.,
        w: LABEL_W + width,
        h: y.max(ROW),
    };
    TimelineLayout {
        origin,
        last,
        bars,
        rows,
        groups,
        links,
        bounds,
    }
}

/// From a bar's end to another's start, leaving and arriving level, with
/// rounded elbows: when the target starts far enough after, the run turns
/// down (or up) just before it; otherwise it steps out, crosses on the
/// target row's edge, back past the target's start, and in.
pub fn link(from: &Rect, to: &Rect) -> EdgePath {
    // Room for a corner and the arrowhead on each straight end.
    const STUB: f32 = 20.;
    let a = Point {
        x: from.right(),
        y: from.y + from.h / 2.,
    };
    let b = Point {
        x: to.x,
        y: to.y + to.h / 2.,
    };
    let level = (a.y - b.y).abs() < layout::ALIGNED;
    if level && b.x >= a.x {
        return layout::orthogonal(&[a, b]);
    }
    if !level && b.x - a.x >= 2. * STUB {
        let x = b.x - STUB;
        return layout::orthogonal(&[a, Point { x, y: a.y }, Point { x, y: b.y }, b]);
    }
    let y = b.y + if a.y < b.y { -ROW / 2. } else { ROW / 2. };
    let (out, back) = (a.x + STUB, b.x - STUB);
    layout::orthogonal(&[
        a,
        Point { x: out, y: a.y },
        Point { x: out, y },
        Point { x: back, y },
        Point { x: back, y: b.y },
        b,
    ])
}

/// The header's counts.
pub fn counts(timeline: &Timeline) -> String {
    counts_in(timeline, crate::i18n::Language::English)
}

pub fn counts_in(timeline: &Timeline, language: crate::i18n::Language) -> String {
    let count = super::edits::count_in(
        language,
        timeline.tasks.len(),
        "{count} task",
        "{count} tasks",
    );
    let (Some(first), Some(last)) = (
        timeline.tasks.iter().map(|t| t.start).min(),
        timeline.tasks.iter().map(|t| t.end).max(),
    ) else {
        return count;
    };
    format!(
        "{count} · {} – {}",
        short_date_in(first, language),
        short_date_in(last, language)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genui::edits::Edits;

    fn day(s: &str) -> i64 {
        day_of(s).unwrap()
    }

    fn sample() -> Timeline {
        Timeline::parse(&json!({"tasks": [
            {"id": "design", "label": "Design", "start": "2026-03-02", "end": "2026-03-06", "progress": 1, "group": "Phase 1"},
            {"id": "api", "label": "API", "start": "2026-03-09", "duration": 10, "dependsOn": ["design"], "progress": 40, "group": "Phase 1"},
            {"id": "ui", "label": "UI", "start": "2026-03-09T08:00:00Z", "end": "2026-03-13", "dependsOn": "design"},
            {"id": "beta", "label": "Beta", "start": "2026-03-20", "milestone": true, "dependsOn": ["api", "ui"]}
        ]}))
        .unwrap()
    }

    fn round_trip(old: &Timeline, new: &Timeline) {
        let mut replay = old.clone();
        for change in Timeline::diff(old, new) {
            replay
                .apply(&change)
                .unwrap_or_else(|e| panic!("{change:?}: {e}"));
        }
        assert_eq!(&replay, new);
    }

    #[test]
    fn dates_round_trip() {
        assert_eq!(date_of(day("2024-02-29")), "2024-02-29");
        assert_eq!(day("1970-01-02"), 1);
        assert_eq!(day_of("2026-02-30"), None);
        assert_eq!(short_date(day("2026-03-04")), "Mar 4");
        assert_eq!(day_parts(day("2026-03-02")), (2, true));
    }

    #[test]
    fn parses_durations_progress_and_milestones() {
        let t = sample();
        assert_eq!(t.tasks[0].progress, Some(100));
        assert_eq!(t.tasks[1].end, day("2026-03-18"));
        assert_eq!(t.tasks[2].start, day("2026-03-09"));
        assert_eq!(t.tasks[2].depends_on, vec!["design"]);
        assert_eq!(
            (t.tasks[3].start, t.tasks[3].end),
            (day("2026-03-20"), day("2026-03-20"))
        );
        assert_eq!(t.groups(), vec![Some("Phase 1"), None]);
        assert_eq!(Timeline::parse(&t.to_value()).unwrap(), t);
        assert_eq!(t.critical_path(), vec!["design", "api", "beta"]);
        assert!(t.late().is_empty());
    }

    #[test]
    fn readable_errors() {
        let errors = Timeline::parse(&json!({"tasks": [
            {"id": "a", "start": "2026-03-02"},
            {"id": "b", "start": "March", "end": "2026-03-04"},
            {"id": "c", "start": "2026-03-05", "end": "2026-03-01"},
            {"id": "d", "start": "2026-03-05", "duration": 2, "dependsOn": ["d", "zz"]},
            {}
        ]}))
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                "task \"a\" needs an \"end\" date or a \"duration\" in days",
                "task \"b\": start \"March\" is not a date (YYYY-MM-DD)",
                "task \"c\" ends before it starts",
                "task #5 has no label",
                "task \"d\" can't depend on itself",
                "task \"d\": dependency \"zz\" does not exist",
            ]
        );
        let cycle = Timeline::parse(&json!({"tasks": [
            {"id": "a", "start": "2026-03-02", "duration": 1, "dependsOn": "b"},
            {"id": "b", "start": "2026-03-02", "duration": 1, "dependsOn": "a"}
        ]}))
        .unwrap_err();
        assert_eq!(cycle, vec!["dependency cycle: a → b → a"]);
    }

    #[test]
    fn layout_groups_rows_and_links_bars() {
        let t = sample();
        let l = layout(&t);
        assert_eq!(l.origin, day("2026-02-28"));
        assert_eq!(l.groups.len(), 1);
        // Grouped tasks under their header, the ungrouped after.
        assert!(l.rows[0].y >= GROUP_ROW && l.rows[1].y > l.rows[0].y && l.rows[2].y > l.rows[1].y);
        assert_eq!(l.bars[0].x, l.x_of(day("2026-03-02")));
        assert_eq!(l.bars[0].w, 5. * DAY);
        // The milestone is a square diamond box in the middle of its day.
        assert_eq!(l.bars[3].w, BAR_H);
        assert_eq!(l.links.len(), 4);
        let (di, ti, path) = &l.links[0];
        assert_eq!((*di, *ti), (0, 1));
        assert_eq!(path.start().x, l.bars[0].right());
        assert_eq!(path.end().x, l.bars[1].x);
        assert_eq!(
            l.day_at(l.x_of(day("2026-03-10")) + 3., false),
            day("2026-03-10")
        );
        assert_eq!(l.task_at(l.bars[2].center()), Some(2));
    }

    #[test]
    fn edits_describe_round_trip_and_flag_late_tasks() {
        let base = sample();
        let mut edits = Edits::new(base.clone());
        // Drag UI two days later; stretch API by three days.
        edits
            .edit(&Change::SetDates {
                task: "ui".into(),
                start: day("2026-03-11"),
                end: day("2026-03-15"),
            })
            .unwrap();
        edits
            .edit(&Change::SetDates {
                task: "api".into(),
                start: day("2026-03-09"),
                end: day("2026-03-21"),
            })
            .unwrap();
        let docs = Task::new(
            base.fresh_task_id("Docs"),
            "Docs",
            day("2026-03-16"),
            day("2026-03-18"),
        );
        edits.edit(&Change::AddTask { task: docs }).unwrap();
        edits
            .edit(&Change::AddDependency {
                task: "docs".into(),
                on: "ui".into(),
            })
            .unwrap();
        edits
            .edit(&Change::RemoveDependency {
                task: "beta".into(),
                on: "ui".into(),
            })
            .unwrap();
        // A loop is refused.
        assert!(
            edits
                .edit(&Change::AddDependency {
                    task: "design".into(),
                    on: "beta".into(),
                })
                .is_err()
        );
        let (values, lines) = edits.summarize();
        assert_eq!(values[2]["start"], "2026-03-11");
        assert_eq!(
            lines,
            vec![
                "“Beta” no longer waits for “UI”",
                "made “API” 13 days (Mar 9 – Mar 21)",
                "moved “UI” 2 days later (Mar 11 – Mar 15)",
                "added task “Docs” (Mar 16 – Mar 18)",
                "made “Docs” wait for “UI”",
            ]
        );
        round_trip(&base, edits.working());
        // Beta now starts before API ends.
        let w = edits.working();
        assert_eq!(w.late(), vec![(1, 3)]);
        assert_eq!(w.critical_path(), vec!["design", "api"]);
    }
}
