//! Runs of finished steps: consecutive tool steps that finished quietly
//! fold into one summary line ("Read 2 files · Ran 1 command") until a
//! click shows them.

use std::ops::Range;

use super::{Item, ToolStatus, assistant_visible};

/// What a tool step did, for a run's summary line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Read,
    Search,
    Edit,
    Command,
    Delegate,
    Web,
    Other,
}

impl StepKind {
    /// The kind of the tool called `name`.
    pub fn of(name: &str) -> Self {
        match name {
            "read" => Self::Read,
            "grep" | "find" | "ls" => Self::Search,
            "edit" | "write" => Self::Edit,
            "bash" | "bash_background" | "monitor" | "codemode" => Self::Command,
            n if n.starts_with("subagent") => Self::Delegate,
            n if n.contains("search") => Self::Search,
            n if n.starts_with("web") || n.contains("fetch") || n.contains("browser") => Self::Web,
            _ => Self::Other,
        }
    }

    /// `count` steps of this kind, in words: "Read 2 files", "Ran 1 command".
    pub fn label(self, count: usize) -> String {
        self.label_in(crate::i18n::Language::English, count)
    }

    pub fn label_in(self, language: crate::i18n::Language, count: usize) -> String {
        let template = match (self, count) {
            (Self::Read, 1) => "Read {count} file",
            (Self::Read, _) => "Read {count} files",
            (Self::Search, 1) => "Searched {count} time",
            (Self::Search, _) => "Searched {count} times",
            (Self::Edit, 1) => "Edited {count} file",
            (Self::Edit, _) => "Edited {count} files",
            (Self::Command, 1) => "Ran {count} command",
            (Self::Command, _) => "Ran {count} commands",
            (Self::Delegate, 1) => "Delegated {count} task",
            (Self::Delegate, _) => "Delegated {count} tasks",
            (Self::Web, 1) => "Fetched {count} page",
            (Self::Web, _) => "Fetched {count} pages",
            (Self::Other, 1) => "Used {count} tool",
            (Self::Other, _) => "Used {count} tools",
        };
        crate::i18n::tr(language, template).replace("{count}", &count.to_string())
    }
}

/// Whether `item` can hide in a run's summary: a tool step that finished
/// and folds to its quiet line (no pictures to show, not the generative-UI
/// tool, which is a live card of its own).
pub fn groupable(item: &Item) -> bool {
    match item {
        Item::Tool {
            name,
            status,
            images,
            ..
        } => *status == ToolStatus::Done && images.is_empty() && name != crate::genui::TOOL,
        _ => false,
    }
}

/// Whether `item` can sit inside a run: a groupable step, or an item that
/// shows nothing (a finished assistant message that only called tools).
fn in_run(item: &Item) -> bool {
    groupable(item) || !assistant_visible(item)
}

/// The run of groupable steps around item `ix`: from its first groupable
/// item to just past its last, flowing across items that show nothing.
/// Anything else visible ends it. `None` unless `ix` lies inside a run of
/// at least two steps (one step shows its own line).
pub fn step_run(items: &[Item], ix: usize) -> Option<Range<usize>> {
    if !in_run(items.get(ix)?) {
        return None;
    }
    let lo = items[..ix]
        .iter()
        .rposition(|item| !in_run(item))
        .map_or(0, |i| i + 1);
    let hi = items[ix..]
        .iter()
        .position(|item| !in_run(item))
        .map_or(items.len(), |i| ix + i);
    let members = &items[lo..hi];
    let start = lo + members.iter().position(groupable)?;
    let end = lo + members.iter().rposition(groupable)? + 1;
    let count = items[start..end].iter().filter(|i| groupable(i)).count();
    (count >= 2 && (start..end).contains(&ix)).then_some(start..end)
}

/// What the steps in `run` did, counted by kind in first-seen order.
pub fn run_summary(items: &[Item], run: Range<usize>) -> Vec<(StepKind, usize)> {
    let mut counts: Vec<(StepKind, usize)> = Vec::new();
    for item in &items[run] {
        let Item::Tool { name, .. } = item else {
            continue;
        };
        let kind = StepKind::of(name);
        match counts.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((kind, 1)),
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Block;
    use serde_json::json;

    fn tool(name: &str, status: ToolStatus) -> Item {
        Item::Tool {
            call_id: name.into(),
            name: name.into(),
            args: json!({}),
            status,
            output: String::new(),
            images: Vec::new(),
            details: json!({}),
            duration_ms: None,
        }
    }

    fn done(name: &str) -> Item {
        tool(name, ToolStatus::Done)
    }

    /// A finished assistant message that only called tools: shows nothing.
    fn calls() -> Item {
        Item::Assistant {
            blocks: vec![Block::ToolCall],
            streaming: false,
            error: None,
        }
    }

    fn answer(text: &str) -> Item {
        Item::Assistant {
            blocks: vec![Block::Text(text.into())],
            streaming: false,
            error: None,
        }
    }

    #[test]
    fn a_run_flows_across_messages_that_show_nothing() {
        let items = [
            answer("looking"),
            calls(),
            done("read"),
            calls(),
            done("read"),
            done("bash"),
            calls(),
            answer("done"),
        ];
        // Every item inside the run finds it; its edges are steps.
        for ix in 2..6 {
            assert_eq!(step_run(&items, ix), Some(2..6));
        }
        // What shows nothing around the run is outside it.
        assert_eq!(step_run(&items, 1), None);
        assert_eq!(step_run(&items, 6), None);
        assert_eq!(step_run(&items, 0), None);
        assert_eq!(step_run(&items, 7), None);
        assert_eq!(step_run(&items, 8), None);
    }

    #[test]
    fn a_visible_item_or_an_unfinished_step_breaks_a_run() {
        let user = Item::User {
            text: "hi".into(),
            images: vec![],
        };
        for breaker in [
            answer("so far so good"),
            user,
            tool("bash", ToolStatus::Failed),
            tool("bash", ToolStatus::Running),
        ] {
            let items = [
                done("read"),
                done("grep"),
                breaker,
                done("edit"),
                done("ls"),
            ];
            assert_eq!(step_run(&items, 0), Some(0..2));
            assert_eq!(step_run(&items, 1), Some(0..2));
            assert_eq!(step_run(&items, 2), None);
            assert_eq!(step_run(&items, 3), Some(3..5));
        }
    }

    #[test]
    fn steps_that_show_more_than_a_line_never_group() {
        let mut shot = done("browser");
        if let Item::Tool { images, .. } = &mut shot {
            images.push(crate::transcript::UserImage::new(
                "image/png",
                "AA==".into(),
            ));
        }
        for item in [shot, done(crate::genui::TOOL)] {
            assert!(!groupable(&item));
            let items = [done("read"), item, done("read")];
            assert_eq!(step_run(&items, 0), None);
        }
    }

    #[test]
    fn a_single_step_is_not_grouped() {
        let items = [answer("a"), done("read"), calls(), answer("b")];
        assert_eq!(step_run(&items, 1), None);
        assert_eq!(step_run(&items, 2), None);
    }

    #[test]
    fn a_summary_counts_steps_by_kind_in_first_seen_order() {
        let items = [
            done("read"),
            calls(),
            done("bash"),
            done("read"),
            done("web_search"),
            done("grep"),
            done("subagent_run"),
            done("web_fetch"),
            done("write"),
            done("todo"),
        ];
        assert_eq!(
            run_summary(&items, 0..items.len()),
            vec![
                (StepKind::Read, 2),
                (StepKind::Command, 1),
                (StepKind::Search, 2),
                (StepKind::Delegate, 1),
                (StepKind::Web, 1),
                (StepKind::Edit, 1),
                (StepKind::Other, 1),
            ]
        );
    }

    #[test]
    fn labels_read_as_sentences() {
        assert_eq!(StepKind::Read.label(2), "Read 2 files");
        assert_eq!(StepKind::Read.label(1), "Read 1 file");
        assert_eq!(StepKind::Search.label(3), "Searched 3 times");
        assert_eq!(StepKind::Search.label(1), "Searched 1 time");
        assert_eq!(StepKind::Edit.label(1), "Edited 1 file");
        assert_eq!(StepKind::Command.label(4), "Ran 4 commands");
        assert_eq!(StepKind::Command.label(1), "Ran 1 command");
        assert_eq!(StepKind::Delegate.label(2), "Delegated 2 tasks");
        assert_eq!(StepKind::Web.label(1), "Fetched 1 page");
        assert_eq!(StepKind::Other.label(2), "Used 2 tools");
    }

    #[test]
    fn kinds_come_from_tool_names() {
        assert_eq!(StepKind::of("ls"), StepKind::Search);
        assert_eq!(StepKind::of("web_search"), StepKind::Search);
        assert_eq!(StepKind::of("browser"), StepKind::Web);
        assert_eq!(StepKind::of("fetch_url"), StepKind::Web);
        assert_eq!(StepKind::of("subagent_spawn"), StepKind::Delegate);
        assert_eq!(StepKind::of("monitor"), StepKind::Command);
        assert_eq!(StepKind::of("screen"), StepKind::Other);
    }
}
