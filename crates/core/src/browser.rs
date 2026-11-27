//! Agents driving the in-app browser, after Tern's `tern browser`: the
//! operations a `gentle-browser` command (or the `browser` tool) asks for,
//! how its command line reads, how a target names an element, and which
//! website-data profile a page uses. The app runs them in the page; this
//! module only reads and words them.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What one `gentle-browser` call asks the app's browser to do. `tab` names
/// a page by the name it was opened with (or its number).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Open `url` in a card named `name` (or show/navigate the one that has
    /// that name). `show: false` keeps it out of sight but live.
    Open {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        profile: Option<String>,
        #[serde(default = "yes")]
        show: bool,
    },
    /// Every page open in the window, the user's too.
    Tabs,
    Goto {
        tab: String,
        url: String,
    },
    Back {
        tab: String,
    },
    Forward {
        tab: String,
    },
    Reload {
        tab: String,
    },
    /// URL, title and whether it is still loading.
    State {
        tab: String,
    },
    /// Run JavaScript (an expression, or statements with `return`); the
    /// answer is its value, awaited, as JSON.
    Eval {
        tab: String,
        js: String,
    },
    /// The page as an outline of elements with refs (`e12`) `act` takes.
    Snapshot {
        tab: String,
        #[serde(default)]
        root: Option<String>,
        #[serde(default)]
        text: bool,
        #[serde(default)]
        nodes: Option<usize>,
    },
    /// Do something to the element `target` names (see [`Target`]).
    Act {
        tab: String,
        action: Action,
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        dy: Option<f64>,
    },
    /// Wait until `target` shows (or is gone, with `gone`), up to `timeout`
    /// seconds.
    Wait {
        tab: String,
        target: String,
        #[serde(default)]
        gone: bool,
        #[serde(default)]
        timeout: Option<f64>,
    },
    /// A PNG of what the page shows; the answer names the file.
    Screenshot {
        tab: String,
    },
    /// Wait for the user to click an element; the answer is its ref.
    Pick {
        tab: String,
    },
    Close {
        tab: String,
    },
    /// Hand the page back to the user: the agent stops driving it.
    Release {
        tab: String,
    },
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Click,
    /// Replace an input's value with `text`.
    Fill,
    /// Type `text` where the focus is (or into `target`).
    Type,
    /// Press keys: `Enter`, `Tab`, `Meta+a`.
    Press,
    /// Choose the option whose value or label is `text`.
    Select,
    Check,
    Uncheck,
    Hover,
    Focus,
    /// Scroll the page, or `target`, by `dy` px (600 by default).
    Scroll,
}

impl Op {
    /// The page the op works on; `None` for `open` and `tabs`.
    pub fn tab(&self) -> Option<&str> {
        match self {
            Op::Open { .. } | Op::Tabs => None,
            Op::Goto { tab, .. }
            | Op::Back { tab }
            | Op::Forward { tab }
            | Op::Reload { tab }
            | Op::State { tab }
            | Op::Eval { tab, .. }
            | Op::Snapshot { tab, .. }
            | Op::Act { tab, .. }
            | Op::Wait { tab, .. }
            | Op::Screenshot { tab }
            | Op::Pick { tab }
            | Op::Close { tab }
            | Op::Release { tab } => Some(tab),
        }
    }

    /// Ops that only read the page don't mark it as driven by the agent.
    pub fn drives(&self) -> bool {
        !matches!(
            self,
            Op::Tabs
                | Op::State { .. }
                | Op::Snapshot { .. }
                | Op::Screenshot { .. }
                | Op::Release { .. }
        )
    }

    /// How long the app may take to answer, in seconds.
    pub fn timeout(&self) -> f64 {
        match self {
            Op::Pick { .. } => 300.,
            Op::Wait { timeout, .. } => timeout.unwrap_or(10.).clamp(0.1, 120.) + 5.,
            Op::Open { .. } | Op::Goto { .. } => 60.,
            _ => 30.,
        }
    }
}

/// The answer the command prints: `{"ok": …}` or `{"error": {kind, message}}`.
pub fn ok(value: Value) -> Value {
    json!({ "ok": value })
}

pub fn error(kind: &str, message: impl Into<String>) -> Value {
    json!({ "error": { "kind": kind, "message": message.into() } })
}

/// How long to wait for the app's answer to `op` (JSON as sent): the op's
/// own time plus a margin, or 35 s for one that doesn't parse.
pub fn answer_within(op: &Value) -> std::time::Duration {
    let secs = serde_json::from_value::<Op>(op.clone()).map_or(30., |op| op.timeout());
    std::time::Duration::from_secs_f64(secs + 5.)
}

/// The script that starts a page-side operation `name` with `args`; its
/// answer is polled with [`take_script`].
pub fn run_script(id: &str, name: &str, args: &Value) -> String {
    format!(
        "window.__gentleBrowser.run({}, {}, {})",
        Value::from(id),
        Value::from(name),
        args
    )
}

/// The script that starts the agent's own JavaScript: an expression, or
/// statements that `return` the answer. It is written into the app's script
/// rather than handed to `eval`, which a page's CSP may forbid.
pub fn eval_script(id: &str, js: &str) -> String {
    let has_return = js
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
        .any(|word| word == "return");
    let body = if has_return {
        js.to_string()
    } else {
        format!("return ({}\n);", js.trim().trim_end_matches(';'))
    };
    format!(
        "window.__gentleBrowser.start({}, async () => {{\n{body}\n}})",
        Value::from(id)
    )
}

/// The script that reads operation `id`'s answer: `""` while it runs,
/// `"__gone__"` once the page it ran in navigated away.
pub fn take_script(id: &str) -> String {
    format!(
        "window.__gentleBrowser ? window.__gentleBrowser.take({}) : \"__gone__\"",
        Value::from(id)
    )
}

// ── Targets ─────────────────────────────────────────────────────────────────

/// How a target names an element, after Puppeteer's selectors: a snapshot
/// ref (`e12`, `@e12`), `text/Sign in` (the smallest visible element whose
/// text contains it), `aria/Search` (a role-bearing element whose accessible
/// name contains it, optionally `aria/button/Search`), `xpath/…`, else CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Ref(String),
    Text(String),
    Aria { role: Option<String>, name: String },
    XPath(String),
    Css(String),
}

impl Target {
    pub fn parse(raw: &str) -> Target {
        let raw = raw.trim();
        if let Some(rest) = raw.strip_prefix("text/") {
            return Target::Text(rest.to_string());
        }
        if let Some(rest) = raw.strip_prefix("aria/") {
            const ROLES: &[&str] = &[
                "button",
                "link",
                "textbox",
                "checkbox",
                "radio",
                "combobox",
                "listbox",
                "option",
                "menuitem",
                "tab",
                "heading",
                "img",
                "searchbox",
                "switch",
            ];
            return match rest.split_once('/') {
                Some((role, name)) if ROLES.contains(&role) => Target::Aria {
                    role: Some(role.to_string()),
                    name: name.to_string(),
                },
                _ => Target::Aria {
                    role: None,
                    name: rest.to_string(),
                },
            };
        }
        if let Some(rest) = raw.strip_prefix("xpath/") {
            return Target::XPath(rest.to_string());
        }
        let bare = raw.strip_prefix('@').unwrap_or(raw);
        if bare.len() > 1 && bare.starts_with('e') && bare[1..].bytes().all(|b| b.is_ascii_digit())
        {
            return Target::Ref(bare.to_string());
        }
        Target::Css(raw.to_string())
    }

    /// The target as the page script's `find` reads it.
    pub fn to_json(&self) -> Value {
        match self {
            Target::Ref(r) => json!({ "ref": r }),
            Target::Text(t) => json!({ "text": t }),
            Target::Aria { role, name } => json!({ "aria": name, "role": role }),
            Target::XPath(x) => json!({ "xpath": x }),
            Target::Css(c) => json!({ "css": c }),
        }
    }
}

// ── Profiles ────────────────────────────────────────────────────────────────

/// Where a page keeps its cookies, storage and caches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Profile {
    /// One store for every page that doesn't ask for another (the user's).
    Shared,
    /// One per repository: the pages of agents working in it.
    Repository(String),
    /// One per agent conversation.
    Session(String),
    /// The agents' own store, apart from the user's: the default for pages
    /// an agent opens.
    Agent,
    /// Nothing kept: gone when the page closes.
    Private,
    /// One the user named.
    Named(String),
}

impl Profile {
    /// `shared`, `agent`, `private`, `repo:<path>`, `session:<id>`, else a
    /// name of the user's.
    pub fn parse(raw: &str) -> Profile {
        let raw = raw.trim();
        match raw.to_ascii_lowercase().as_str() {
            "" | "shared" => Profile::Shared,
            "agent" | "script" => Profile::Agent,
            "private" | "incognito" => Profile::Private,
            _ => {
                if let Some(path) = raw.strip_prefix("repo:") {
                    Profile::Repository(path.to_string())
                } else if let Some(id) = raw.strip_prefix("session:") {
                    Profile::Session(id.to_string())
                } else {
                    Profile::Named(raw.to_string())
                }
            }
        }
    }

    /// The text [`Profile::parse`] reads back.
    pub fn key(&self) -> String {
        match self {
            Profile::Shared => "shared".into(),
            Profile::Agent => "agent".into(),
            Profile::Private => "private".into(),
            Profile::Repository(path) => format!("repo:{path}"),
            Profile::Session(id) => format!("session:{id}"),
            Profile::Named(name) => name.clone(),
        }
    }

    /// What the card's menu calls it, and the kind beside it.
    pub fn label(&self) -> (String, &'static str) {
        let leaf = |path: &str| {
            path.trim_end_matches(['/', '\\'])
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(path)
                .to_string()
        };
        match self {
            Profile::Shared => ("Shared".into(), ""),
            Profile::Agent => ("Agent".into(), ""),
            Profile::Private => ("Private".into(), ""),
            Profile::Repository(path) => (leaf(path), "Repository"),
            Profile::Session(id) => (id.chars().take(8).collect(), "Session"),
            Profile::Named(name) => (name.clone(), "Profile"),
        }
    }

    /// Keeps nothing on disk.
    pub fn is_private(&self) -> bool {
        *self == Profile::Private
    }

    /// The general-purpose store shared by pages that use the Shared profile.
    pub fn is_shared(&self) -> bool {
        *self == Profile::Shared
    }

    /// A folder name for the store: readable, file-system safe, and unique
    /// per profile (a hash of its key follows the readable part).
    pub fn folder(&self) -> String {
        let mut out = String::new();
        for ch in self.key().chars() {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                out.push(ch.to_ascii_lowercase());
            } else if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
        }
        let readable: String = out.trim_end_matches('_').chars().take(40).collect();
        let hash = format!("{:08x}", fnv(self.key().as_bytes(), 0) as u32);
        if readable.is_empty() {
            hash
        } else {
            format!("{readable}-{hash}")
        }
    }

    /// A stable 16-byte WebKit store identifier, scoped to this app so the
    /// same browser profile in upstream gets a different store.
    pub fn store_id(&self) -> [u8; 16] {
        let key = format!("osdy-code-desktop:{}", self.key());
        let mut id = [0u8; 16];
        id[..8].copy_from_slice(&fnv(key.as_bytes(), 0).to_le_bytes());
        id[8..].copy_from_slice(&fnv(key.as_bytes(), 0x9e37_79b9_7f4a_7c15).to_le_bytes());
        id
    }
}

/// FNV-1a, 64 bits, with `seed` mixed into the offset basis.
fn fnv(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325 ^ seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

// ── How browser cards open ──────────────────────────────────────────────────

/// Key of the app's `settings.json` for [`CardLayout`].
pub const LAYOUT_KEY: &str = "browserCard";

/// How the app's browser cards open, as the user last left them: docked as
/// a column at the window's right or floating, the docked column's width
/// and a floating card's size (logical px). The window keeps them inside
/// itself when it lays the cards out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CardLayout {
    pub docked: bool,
    pub dock_width: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for CardLayout {
    fn default() -> Self {
        Self {
            docked: false,
            dock_width: 480.,
            width: 560.,
            height: 380.,
        }
    }
}

impl CardLayout {
    /// The saved layout; a missing, broken or nonsensical size (not a
    /// positive number) takes its default.
    pub fn from_json(value: &Value) -> Self {
        let saved: Self = serde_json::from_value(value.clone()).unwrap_or_default();
        let fallback = Self::default();
        let size = |value: f32, default: f32| {
            if value.is_finite() && value > 0. {
                value
            } else {
                default
            }
        };
        Self {
            docked: saved.docked,
            dock_width: size(saved.dock_width, fallback.dock_width),
            width: size(saved.width, fallback.width),
            height: size(saved.height, fallback.height),
        }
    }

    pub fn to_json(self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// ── The command line ────────────────────────────────────────────────────────

pub const USAGE: &str = "\
gentle-browser: drive a page in the Osdy Code Desktop browser

  gentle-browser open <url> [--name N] [--profile P] [--hidden]
  gentle-browser tabs
  gentle-browser goto <tab> <url>         back|forward|reload|state|close|release <tab>
  gentle-browser snapshot <tab> [--text] [--root <target>] [--nodes N]
  gentle-browser click|hover|focus|check|uncheck <tab> <target>
  gentle-browser fill <tab> <target> <text>      type <tab> [<target>] <text>
  gentle-browser select <tab> <target> <option>  press <tab> <keys>   (Enter, Meta+a)
  gentle-browser scroll <tab> [<target>] [--dy PX]
  gentle-browser wait <tab> <target> [--gone] [--timeout S]
  gentle-browser eval <tab> <js>          screenshot <tab>      pick <tab>
  gentle-browser --json '<op>'            any op as JSON

Targets: a snapshot ref (e12), text/<text>, aria/[role/]<name>, xpath/<path>, or CSS.
Profiles: agent (default for agents), shared, private, repo:<path>, session:<id>, or a name.
Answers {\"ok\": …} or {\"error\": {\"kind\", \"message\"}} (exit 1).";

/// The op in `gentle-browser`'s arguments.
pub fn parse_args(args: &[String]) -> Result<Op, String> {
    let mut words: Vec<&str> = Vec::new();
    let mut flags: Vec<(&str, Option<&str>)> = Vec::new();
    let valued = [
        "--name",
        "--profile",
        "--root",
        "--nodes",
        "--dy",
        "--timeout",
        "--json",
    ];
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if valued.contains(&arg) {
            flags.push((arg, args.get(i + 1).map(String::as_str)));
            i += 2;
        } else if arg.starts_with("--") && arg.len() > 2 {
            flags.push((arg, None));
            i += 1;
        } else {
            words.push(arg);
            i += 1;
        }
    }
    let flag = |name: &str| flags.iter().find(|(f, _)| *f == name).and_then(|(_, v)| *v);
    let has = |name: &str| flags.iter().any(|(f, _)| *f == name);
    if let Some(json) = flag("--json") {
        return serde_json::from_str(json).map_err(|e| format!("--json: {e}"));
    }
    let usage = || USAGE.to_string();
    let word = |n: usize| words.get(n).copied().ok_or_else(usage);
    let tab = || word(1).map(str::to_string);
    let number = |name: &str| -> Result<Option<f64>, String> {
        flag(name)
            .map(|v| {
                v.parse::<f64>()
                    .map_err(|_| format!("{name} takes a number"))
            })
            .transpose()
    };
    let act =
        |action: Action, target: Option<String>, text: Option<String>| -> Result<Op, String> {
            Ok(Op::Act {
                tab: tab()?,
                action,
                target,
                text,
                dy: number("--dy")?,
            })
        };
    let rest = |from: usize| (words.len() > from).then(|| words[from..].join(" "));
    match word(0)? {
        "open" => Ok(Op::Open {
            name: flag("--name").map(str::to_string),
            url: words.get(1).map(|u| u.to_string()),
            profile: flag("--profile").map(str::to_string),
            show: !has("--hidden"),
        }),
        "tabs" => Ok(Op::Tabs),
        "goto" => Ok(Op::Goto {
            tab: tab()?,
            url: word(2)?.to_string(),
        }),
        "back" => Ok(Op::Back { tab: tab()? }),
        "forward" => Ok(Op::Forward { tab: tab()? }),
        "reload" => Ok(Op::Reload { tab: tab()? }),
        "state" => Ok(Op::State { tab: tab()? }),
        "close" => Ok(Op::Close { tab: tab()? }),
        "release" => Ok(Op::Release { tab: tab()? }),
        "screenshot" => Ok(Op::Screenshot { tab: tab()? }),
        "pick" => Ok(Op::Pick { tab: tab()? }),
        "eval" => Ok(Op::Eval {
            tab: tab()?,
            js: rest(2).ok_or_else(usage)?,
        }),
        "snapshot" => Ok(Op::Snapshot {
            tab: tab()?,
            root: flag("--root").map(str::to_string),
            text: has("--text"),
            nodes: number("--nodes")?.map(|n| n.max(1.) as usize),
        }),
        "wait" => Ok(Op::Wait {
            tab: tab()?,
            target: rest(2).ok_or_else(usage)?,
            gone: has("--gone"),
            timeout: number("--timeout")?,
        }),
        "click" => act(Action::Click, Some(rest(2).ok_or_else(usage)?), None),
        "hover" => act(Action::Hover, Some(rest(2).ok_or_else(usage)?), None),
        "focus" => act(Action::Focus, Some(rest(2).ok_or_else(usage)?), None),
        "check" => act(Action::Check, Some(rest(2).ok_or_else(usage)?), None),
        "uncheck" => act(Action::Uncheck, Some(rest(2).ok_or_else(usage)?), None),
        "fill" => act(
            Action::Fill,
            Some(word(2)?.to_string()),
            Some(rest(3).ok_or_else(usage)?),
        ),
        "select" => act(
            Action::Select,
            Some(word(2)?.to_string()),
            Some(rest(3).ok_or_else(usage)?),
        ),
        "type" => match words.len() {
            0..=2 => Err(usage()),
            3 => act(Action::Type, None, Some(words[2].to_string())),
            _ => act(Action::Type, Some(words[2].to_string()), rest(3)),
        },
        "press" => act(Action::Press, None, Some(rest(2).ok_or_else(usage)?)),
        "scroll" => act(Action::Scroll, rest(2), None),
        _ => Err(usage()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn card_layout_round_trips_and_repairs_what_it_cannot_use() {
        let layout = CardLayout {
            docked: true,
            dock_width: 620.,
            width: 900.,
            height: 500.,
        };
        assert_eq!(CardLayout::from_json(&layout.to_json()), layout);
        assert_eq!(
            layout.to_json(),
            json!({"docked": true, "dockWidth": 620.0, "width": 900.0, "height": 500.0})
        );
        // Missing keys and broken sizes take the defaults; the rest stays.
        let partial = CardLayout::from_json(&json!({"docked": true, "width": -4, "height": 0}));
        assert!(partial.docked);
        assert_eq!(
            (partial.dock_width, partial.width, partial.height),
            (480., 560., 380.)
        );
        assert_eq!(
            CardLayout::from_json(&json!("nonsense")),
            CardLayout::default()
        );
        assert!(!CardLayout::default().docked);
    }

    #[test]
    fn targets_read_like_puppeteer_selectors() {
        assert_eq!(Target::parse("@e12"), Target::Ref("e12".into()));
        assert_eq!(Target::parse("e7"), Target::Ref("e7".into()));
        assert_eq!(
            Target::parse("text/1 reply"),
            Target::Text("1 reply".into())
        );
        assert_eq!(
            Target::parse("aria/button/Send"),
            Target::Aria {
                role: Some("button".into()),
                name: "Send".into()
            }
        );
        // A slash in a plain accessible name isn't a role.
        assert_eq!(
            Target::parse("aria/Search docs/help"),
            Target::Aria {
                role: None,
                name: "Search docs/help".into()
            }
        );
        assert_eq!(
            Target::parse("xpath///button"),
            Target::XPath("//button".into())
        );
        // Words starting with e aren't refs; CSS stays CSS.
        assert_eq!(Target::parse("em"), Target::Css("em".into()));
        assert_eq!(
            Target::parse("#login .submit"),
            Target::Css("#login .submit".into())
        );
    }

    #[test]
    fn the_command_line_reads_each_op() {
        assert_eq!(
            parse_args(&args(
                "open https://x.dev --name slack-app --profile shared --hidden"
            )),
            Ok(Op::Open {
                name: Some("slack-app".into()),
                url: Some("https://x.dev".into()),
                profile: Some("shared".into()),
                show: false
            })
        );
        assert_eq!(
            parse_args(&args("click slack-app text/1 reply")),
            Ok(Op::Act {
                tab: "slack-app".into(),
                action: Action::Click,
                target: Some("text/1 reply".into()),
                text: None,
                dy: None
            })
        );
        assert_eq!(
            parse_args(&args("fill t #email me@x.dev")),
            Ok(Op::Act {
                tab: "t".into(),
                action: Action::Fill,
                target: Some("#email".into()),
                text: Some("me@x.dev".into()),
                dy: None
            })
        );
        // `type` with one word types it where the focus is.
        assert_eq!(
            parse_args(&args("type t hello")),
            Ok(Op::Act {
                tab: "t".into(),
                action: Action::Type,
                target: None,
                text: Some("hello".into()),
                dy: None
            })
        );
        assert_eq!(
            parse_args(&args("wait t text/Done --gone --timeout 5")),
            Ok(Op::Wait {
                tab: "t".into(),
                target: "text/Done".into(),
                gone: true,
                timeout: Some(5.)
            })
        );
        assert_eq!(
            parse_args(&args("eval t document.title")),
            Ok(Op::Eval {
                tab: "t".into(),
                js: "document.title".into()
            })
        );
        assert!(parse_args(&args("click t")).is_err());
        assert!(parse_args(&args("scroll t --dy lots")).is_err());
        assert!(parse_args(&[]).is_err());
    }

    #[test]
    fn json_ops_round_trip_and_default_to_shown() {
        let op: Op = serde_json::from_str(r#"{"op":"open","url":"https://x.dev"}"#).unwrap();
        assert_eq!(
            op,
            Op::Open {
                name: None,
                url: Some("https://x.dev".into()),
                profile: None,
                show: true
            }
        );
        let json = r#"{"op":"act","tab":"t","action":"press","text":"Meta+a"}"#;
        let via_flag = parse_args(&["--json".to_string(), json.to_string()]).unwrap();
        assert_eq!(via_flag, serde_json::from_str::<Op>(json).unwrap());
    }

    #[test]
    fn profiles_parse_and_name_their_store() {
        assert_eq!(Profile::parse(""), Profile::Shared);
        assert_eq!(Profile::parse("Script"), Profile::Agent);
        assert_eq!(
            Profile::parse("repo:/Users/me/code/app"),
            Profile::Repository("/Users/me/code/app".into())
        );
        for profile in [
            Profile::Agent,
            Profile::Private,
            Profile::Session("01a1".into()),
            Profile::Named("Work".into()),
        ] {
            assert_eq!(Profile::parse(&profile.key()), profile);
        }
        assert_eq!(
            Profile::Repository("/Users/me/code/app".into()).label(),
            ("app".to_string(), "Repository")
        );
        let folder = Profile::Repository("/Users/Me/code/app".into()).folder();
        assert!(folder.starts_with("repo_users_me_code_app-"), "{folder}");
        // Names that read the same still get stores of their own.
        assert_ne!(
            Profile::Named("a b".into()).folder(),
            Profile::Named("a_b".into()).folder()
        );
        assert!(!Profile::Named("../..".into()).folder().contains('.'));
        assert_ne!(Profile::Agent.store_id(), Profile::Shared.store_id());
    }

    #[test]
    fn fork_identity_uses_separate_webkit_profile_stores() {
        let profile = Profile::Named("Work".into());
        let key = profile.key();
        let mut upstream = [0; 16];
        upstream[..8].copy_from_slice(&fnv(key.as_bytes(), 0).to_le_bytes());
        upstream[8..].copy_from_slice(&fnv(key.as_bytes(), 0x9e37_79b9_7f4a_7c15).to_le_bytes());
        assert_ne!(profile.store_id(), upstream);
    }

    #[test]
    fn reads_do_not_drive_and_waits_get_their_time() {
        assert!(
            !Op::Snapshot {
                tab: "t".into(),
                root: None,
                text: false,
                nodes: None
            }
            .drives()
        );
        assert!(
            Op::Goto {
                tab: "t".into(),
                url: "u".into()
            }
            .drives()
        );
        assert_eq!(
            Op::Wait {
                tab: "t".into(),
                target: "x".into(),
                gone: false,
                timeout: Some(500.)
            }
            .timeout(),
            125.
        );
    }

    #[test]
    fn agent_javascript_is_an_expression_unless_it_returns() {
        let expression = eval_script("g1", "document.title;");
        assert!(expression.contains("return (document.title\n);"));
        let statements = eval_script("g1", "const a = 1; return a + 1");
        assert!(statements.contains("{\nconst a = 1; return a + 1\n}"));
        // `returnValue` is not `return`.
        let property = eval_script("g1", "event.returnValue");
        assert!(property.contains("return (event.returnValue\n);"));
    }
}
