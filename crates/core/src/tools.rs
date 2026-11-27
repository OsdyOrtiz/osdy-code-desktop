//! Human summaries for tool cards: `Edit src/a.rs +12 −3`, `Bash cargo test`.

use serde_json::Value;

use crate::i18n::{Language, tr};

#[derive(Debug, PartialEq, Eq)]
pub struct ToolSummary {
    pub title: String,
    pub subject: String,
    pub added: Option<usize>,
    pub removed: Option<usize>,
}

pub fn summarize(name: &str, args: &Value, details: &Value) -> ToolSummary {
    summarize_in(Language::English, name, args, details)
}

pub fn summarize_in(language: Language, name: &str, args: &Value, details: &Value) -> ToolSummary {
    let arg = |key: &str| args[key].as_str().unwrap_or_default().to_string();
    let (title, subject) = match name {
        "bash" => ("Bash", crate::text::first_line(&arg("command"), 120)),
        "edit" => ("Edit", arg("path")),
        "write" => ("Write", arg("path")),
        "read" => ("Read", arg("path")),
        "grep" => ("Grep", arg("pattern")),
        "find" => ("Find", arg("pattern")),
        "ls" => ("List", arg("path")),
        "todo" => ("Todo", arg("action")),
        // `click app · text/Save`, `open https://… `, `screenshot app`.
        "browser" => {
            let head = [arg("op"), arg("tab")]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let detail = ["url", "target", "js", "text"]
                .iter()
                .map(|k| arg(k))
                .find(|s| !s.is_empty());
            let subject = match detail {
                Some(detail) => format!("{head} · {detail}"),
                None => head,
            };
            ("Browser", crate::text::first_line(&subject, 120))
        }
        // `launch · npm run app`, `click 885 · 36,212`, `type 885 · hello`.
        "screen" => {
            let number = |key: &str| args[key].as_f64().map(|n| n.to_string());
            let head = [Some(arg("op")), number("window")]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let command = args["command"].as_array().map(|c| {
                c.iter()
                    .filter_map(|w| w.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            });
            let point = number("x")
                .zip(number("y"))
                .map(|(x, y)| format!("{x},{y}"));
            let detail = [
                command,
                point,
                Some(arg("text")),
                Some(arg("keys")),
                Some(arg("app")),
            ]
            .into_iter()
            .flatten()
            .find(|s| !s.is_empty());
            let subject = match detail {
                Some(detail) => format!("{head} · {detail}"),
                None => head,
            };
            ("Screen", crate::text::first_line(&subject, 120))
        }
        _ => {
            let mut title: String = name.replace('_', " ");
            if let Some(first) = title.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            let subject = [
                "path", "command", "query", "url", "pattern", "prompt", "task",
            ]
            .iter()
            .find_map(|k| args[*k].as_str())
            .map(|s| crate::text::first_line(s, 120))
            .unwrap_or_default();
            return ToolSummary {
                title,
                subject,
                added: None,
                removed: None,
            };
        }
    };
    let (added, removed) = match name {
        "edit" => match details["diff"].as_str() {
            Some(diff) => {
                let (a, r) = diff_counts(diff);
                (Some(a), Some(r))
            }
            None => (None, None),
        },
        "write" => (args["content"].as_str().map(|c| c.lines().count()), None),
        _ => (None, None),
    };
    ToolSummary {
        title: tr(language, title).into(),
        subject,
        added,
        removed,
    }
}

/// Count `+`/`-` lines in Pi's edit diff (`+12 text`, `-3 text`, ` 4 ctx`).
pub fn diff_counts(diff: &str) -> (usize, usize) {
    diff.lines()
        .fold((0, 0), |(a, r), line| match line.as_bytes().first() {
            Some(b'+') => (a + 1, r),
            Some(b'-') => (a, r + 1),
            _ => (a, r),
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoTask {
    pub title: String,
    pub status: String,
    pub note: Option<String>,
}

/// The todo list snapshot every successful `todo` result carries in
/// `details.gentleTodo` (gentle-pi `lib/shell-todo.ts`).
pub fn todo_tasks(details: &Value) -> Option<Vec<TodoTask>> {
    let tasks = details["gentleTodo"]["tasks"].as_array()?;
    Some(
        tasks
            .iter()
            .map(|t| TodoTask {
                title: t["title"].as_str().unwrap_or_default().to_string(),
                status: t["status"].as_str().unwrap_or("pending").to_string(),
                note: t["note"].as_str().map(str::to_string),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn edit_counts_come_from_the_diff() {
        let s = summarize(
            "edit",
            &json!({"path": "src/prompt.rs"}),
            &json!({"diff": " 1 keep\n-2 old\n+2 new\n+3 more"}),
        );
        assert_eq!(
            s,
            ToolSummary {
                title: "Edit".into(),
                subject: "src/prompt.rs".into(),
                added: Some(2),
                removed: Some(1)
            }
        );
    }

    #[test]
    fn unknown_tools_use_a_known_argument() {
        let s = summarize("web_search", &json!({"query": "gpui"}), &Value::Null);
        assert_eq!(
            (s.title.as_str(), s.subject.as_str()),
            ("Web search", "gpui")
        );
    }

    #[test]
    fn browser_cards_name_the_op_page_and_target() {
        let s = summarize(
            "browser",
            &json!({"op": "click", "tab": "app", "target": "text/Save"}),
            &Value::Null,
        );
        assert_eq!(s.subject, "click app · text/Save");
        let s = summarize("browser", &json!({"op": "tabs"}), &Value::Null);
        assert_eq!(s.subject, "tabs");
    }

    #[test]
    fn screen_cards_name_the_op_window_and_what() {
        let s = summarize(
            "screen",
            &json!({"op": "click", "window": 885, "x": 36, "y": 212}),
            &Value::Null,
        );
        assert_eq!(s.subject, "click 885 · 36,212");
        let s = summarize(
            "screen",
            &json!({"op": "launch", "command": ["npm", "run", "app"]}),
            &Value::Null,
        );
        assert_eq!(s.subject, "launch · npm run app");
    }

    #[test]
    fn todo_snapshot_is_read_from_details() {
        let tasks = todo_tasks(
            &json!({"gentleTodo": {"tasks": [{"id":1,"title":"Write","status":"in_progress"}]}}),
        )
        .unwrap();
        assert_eq!(
            tasks[0],
            TodoTask {
                title: "Write".into(),
                status: "in_progress".into(),
                note: None
            }
        );
        assert!(todo_tasks(&json!({})).is_none());
    }
}
