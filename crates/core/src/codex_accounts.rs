//! The display-only account list published by the osdy-gentle extension.

use serde_json::Value;
use std::time::Duration;

pub const USAGE_COMMAND: &str = "osdy-usage";
pub const USAGE_TIMEOUT: Duration = Duration::from_secs(20);

/// A missing command (or a prompt with the same name) must never receive the
/// picker's command text as a conversation prompt.
pub fn lists_extension_command(data: &Value, name: &str) -> bool {
    matches!(name, "osdy-profile" | "osdy-account" | USAGE_COMMAND)
        && data["commands"].as_array().is_some_and(|commands| {
            commands.iter().any(|command| {
                command["source"] == "extension" && command["name"].as_str() == Some(name)
            })
        })
}

pub const WIDGET_KEY: &str = "osdy-gentle:codex-accounts";
const ACCOUNT_LIMIT: usize = 50;
const STRING_LIMIT: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feed {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub active: bool,
    pub binding: Option<String>,
    pub quota: Option<String>,
    truncated_name: bool,
}

impl Account {
    /// A display abbreviation must never be sent as a different account name.
    pub fn command_name(&self) -> Option<&str> {
        let name = self.name.trim();
        (!self.truncated_name && !name.is_empty() && !name.contains(['\r', '\n'])).then_some(name)
    }
}

/// Snapshot of the row when an on-demand usage lookup was requested.
pub struct UsageRequest {
    name: String,
    quota: Option<String>,
}

impl UsageRequest {
    pub fn new(account: &Account) -> Option<Self> {
        Some(Self {
            name: account.command_name()?.to_owned(),
            quota: account.quota.clone(),
        })
    }

    pub fn command(&self) -> String {
        format!("/{USAGE_COMMAND} {}", self.name)
    }

    pub fn matches(&self, account: &Account) -> bool {
        account.command_name() == Some(self.name.as_str())
    }

    pub fn finished(&self, feed: &Feed, failed: bool, elapsed: Duration) -> bool {
        failed
            || elapsed >= USAGE_TIMEOUT
            || feed
                .accounts
                .iter()
                .find(|account| self.matches(account))
                .is_none_or(|account| account.quota != self.quota)
    }
}

fn capped(text: &str) -> (String, bool) {
    if let Some((offset, _)) = text.char_indices().nth(STRING_LIMIT) {
        (text[..offset].to_owned(), true)
    } else {
        (text.to_owned(), false)
    }
}

fn nullable(value: &Value) -> Option<Option<String>> {
    match value {
        Value::Null => Some(None),
        Value::String(text) => Some(Some(capped(text).0)),
        _ => None,
    }
}

/// Parse the one-line v1 widget, validating even rows past the display cap.
pub fn parse(line: &str) -> Option<Feed> {
    if line.contains(['\r', '\n']) {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    let object = value.as_object()?;
    if object.get("v")?.as_u64()? != 1 {
        return None;
    }
    let active = nullable(object.get("active")?)?;
    let rows = object.get("accounts")?.as_array()?;
    let mut accounts = Vec::with_capacity(rows.len().min(ACCOUNT_LIMIT));
    for (index, row) in rows.iter().enumerate() {
        let row = row.as_object()?;
        let name = row.get("name")?.as_str()?;
        let active = row.get("active")?.as_bool()?;
        let binding = nullable(row.get("binding")?)?;
        let quota = nullable(row.get("quota")?)?;
        if index < ACCOUNT_LIMIT {
            let (name, truncated_name) = capped(name);
            accounts.push(Account {
                name,
                active,
                binding,
                quota,
                truncated_name,
            });
        }
    }
    Some(Feed { active, accounts })
}

#[cfg(test)]
mod tests {
    use super::{UsageRequest, lists_extension_command, parse};
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn codex_dispatch_requires_an_exact_extension_command() {
        let data = json!({"commands": [
            {"name": "osdy-profile", "source": "extension"},
            {"name": "osdy-account", "source": "prompt"},
            {"name": "osdy-account-alt", "source": "extension"},
            {"name": "osdy-usage", "source": "extension"},
            {"name": "other", "source": "extension"},
        ]});
        assert!(lists_extension_command(&data, "osdy-profile"));
        assert!(!lists_extension_command(&data, "osdy-account"));
        assert!(lists_extension_command(&data, "osdy-usage"));
        assert!(!lists_extension_command(&data, "other"));
        assert!(!lists_extension_command(&json!({}), "osdy-profile"));
        assert!(lists_extension_command(
            &json!({"commands": [{"name": "osdy-account", "source": "extension"}]}),
            "osdy-account"
        ));
        assert!(!lists_extension_command(
            &json!({"commands": [
                {"name": "osdy-usage", "source": "prompt"},
                {"name": "osdy-usage-alt", "source": "extension"}
            ]}),
            "osdy-usage"
        ));
    }

    #[test]
    fn usage_request_waits_for_its_own_quota_change_failure_or_timeout() {
        fn feed(work_quota: Option<&str>, personal_quota: Option<&str>) -> super::Feed {
            parse(
                &json!({"v":1,"active":"Work account","accounts":[
                    {"name":"Personal","active":false,"binding":null,"quota":personal_quota},
                    {"name":"Work account","active":true,"binding":null,"quota":work_quota}
                ]})
                .to_string(),
            )
            .unwrap()
        }
        let before = feed(None, None);
        let request = UsageRequest::new(&before.accounts[1]).unwrap();
        assert_eq!(request.command(), "/osdy-usage Work account");
        let almost = Duration::from_millis(19_999);
        assert!(!request.finished(&before, false, almost));
        assert!(!request.finished(&feed(None, Some("5h 25% left")), false, almost));
        assert!(request.finished(&feed(Some("5h 75% left"), None), false, almost));
        assert!(request.finished(&before, true, Duration::ZERO));
        assert!(request.finished(&before, false, Duration::from_secs(20)));

        let cached = feed(Some("5h 75% left"), None);
        let request = UsageRequest::new(&cached.accounts[1]).unwrap();
        assert!(!request.finished(&cached, false, almost));
        assert!(request.finished(&before, false, almost));
    }

    #[test]
    fn parses_account_feed() {
        let feed = parse(r#"{"v":1,"active":"Work account","accounts":[{"name":"Work account","active":true,"binding":"Daily","quota":"68% remaining"},{"name":"Personal","active":false,"binding":null,"quota":null}]}"#).unwrap();
        assert_eq!(feed.active.as_deref(), Some("Work account"));
        assert_eq!(feed.accounts.len(), 2);
        let [work, personal] = feed.accounts.as_slice() else {
            unreachable!()
        };
        assert_eq!(work.name, "Work account");
        assert!(work.active);
        assert_eq!(work.binding.as_deref(), Some("Daily"));
        assert_eq!(work.quota.as_deref(), Some("68% remaining"));
        assert_eq!(personal.name, "Personal");
        assert!(!personal.active);
        assert_eq!(personal.binding, None);
        assert_eq!(personal.quota, None);
    }

    #[test]
    fn rejects_unknown_version_and_invalid_json() {
        assert!(parse(r#"{"v":2,"active":null,"accounts":[]}"#).is_none());
        assert!(parse(r#"{"v":1,"active":null,"accounts":[]]"#).is_none());
    }

    #[test]
    fn rejects_wrong_shapes_and_missing_fields() {
        for input in [
            r#"[1,null,[]]"#,
            r#"{"v":"1","active":null,"accounts":[]}"#,
            r#"{"v":1,"accounts":[]}"#,
            r#"{"v":1,"active":true,"accounts":[]}"#,
            r#"{"v":1,"active":null,"accounts":{}}"#,
            r#"{"v":1,"active":null,"accounts":[["A",false,null,null]]}"#,
            r#"{"v":1,"active":null,"accounts":[{"name":1,"active":false,"binding":null,"quota":null}]}"#,
            r#"{"v":1,"active":null,"accounts":[{"name":"A","active":"false","binding":null,"quota":null}]}"#,
            r#"{"v":1,"active":null,"accounts":[{"name":"A","active":false,"binding":0,"quota":null}]}"#,
            r#"{"v":1,"active":null,"accounts":[{"name":"A","active":false,"binding":null,"quota":[]}]}"#,
            r#"{"v":1,"active":null,"accounts":[{"name":"A","active":false,"binding":null}]}"#,
        ] {
            assert!(parse(input).is_none(), "{input}");
        }
    }

    #[test]
    fn ignores_unknown_fields() {
        let known = r#"{"v":1,"active":null,"accounts":[{"name":"A","active":false,"binding":null,"quota":null}]}"#;
        let extra = r#"{"v":1,"active":null,"extra":{},"accounts":[{"name":"A","active":false,"binding":null,"quota":null,"extra":[1]}]}"#;
        assert_eq!(parse(extra), parse(known));
        assert!(parse(extra).is_some());
    }

    #[test]
    fn caps_accounts_in_feed_order_and_strings_at_unicode_character_boundaries() {
        let exact = "é".repeat(64);
        let long = format!("{exact}x");
        let mut accounts: Vec<_> = (0..51)
            .map(|index| json!({"name":index.to_string(),"active":false,"binding":null,"quota":null}))
            .collect();
        accounts[0] = json!({"name":long,"active":true,"binding":long,"quota":exact});
        let feed = parse(&json!({"v":1,"active":long,"accounts":accounts}).to_string()).unwrap();
        assert_eq!(feed.accounts.len(), 50);
        assert_eq!(feed.accounts[49].name, "49");
        assert_eq!(feed.active.as_deref(), Some(exact.as_str()));
        let first = &feed.accounts[0];
        assert_eq!(first.name, exact);
        assert_eq!(first.binding.as_deref(), Some(exact.as_str()));
        assert_eq!(first.quota.as_deref(), Some(exact.as_str()));
        assert_eq!(first.command_name(), None);
        assert!(UsageRequest::new(first).is_none());
        assert_eq!(feed.accounts[49].command_name(), Some("49"));
    }

    #[test]
    fn parses_empty_accounts() {
        let feed = parse(r#"{"v":1,"active":null,"accounts":[]}"#).unwrap();
        assert_eq!(feed.active, None);
        assert!(feed.accounts.is_empty());
    }
}
