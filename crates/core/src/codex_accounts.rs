//! The display-only account list published by the osdy-gentle extension.

use serde_json::Value;
use std::time::{Duration, Instant};

pub const USAGE_COMMAND: &str = "osdy-usage";
pub const ALL_USAGE_COMMAND: &str = "/osdy-usage --all";
pub const USAGE_TIMEOUT: Duration = Duration::from_secs(20);
pub const ALL_USAGE_TIMEOUT: Duration = Duration::from_secs(30);
const ALL_USAGE_FRESHNESS: Duration = Duration::from_secs(60);

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

/// Per-tab batch lifetime and the shorter visible loading state.
#[derive(Default)]
pub struct AllUsage {
    live: Option<(Instant, Vec<QuotaSnapshot>)>,
    loading: bool,
    last_finished: Option<Instant>,
}

type QuotaSnapshot = (String, Option<String>);

fn quotas(feed: &Feed) -> Vec<QuotaSnapshot> {
    let mut quotas: Vec<_> = feed
        .accounts
        .iter()
        .map(|account| (account.name.clone(), account.quota.clone()))
        .collect();
    quotas.sort_unstable();
    quotas
}

impl AllUsage {
    pub fn pending(&self) -> bool {
        self.loading
    }

    pub fn live(&self) -> bool {
        self.started_at().is_some()
    }

    pub fn started_at(&self) -> Option<Instant> {
        self.live.as_ref().map(|(started, _)| *started)
    }

    pub fn hide_loading(&mut self) -> bool {
        std::mem::take(&mut self.loading)
    }

    pub fn start(&mut self, feed: &Feed, force: bool, now: Instant) -> bool {
        if self.live()
            || (!force
                && self
                    .last_finished
                    .is_some_and(|at| now - at < ALL_USAGE_FRESHNESS))
        {
            return false;
        }
        self.live = Some((now, quotas(feed)));
        self.loading = true;
        true
    }

    /// A successful prompt disposition acknowledges dispatch, not completion.
    /// With no completion marker, use all changed quotas, a batch failure,
    /// or the deadline to end the run.
    pub fn finish(&mut self, feed: Option<&Feed>, failed: bool, now: Instant) -> bool {
        let Some((started, before)) = &self.live else {
            return false;
        };
        let after = feed.map(quotas);
        let changed = after.as_ref().is_some_and(|after| after != before);
        let all_changed = after.as_ref().is_some_and(|after| {
            !before.is_empty()
                && before.len() == after.len()
                && before
                    .iter()
                    .zip(after)
                    .all(|((name, quota), (new_name, new_quota))| {
                        name == new_name && quota != new_quota
                    })
        });
        if failed || now - *started >= ALL_USAGE_TIMEOUT || all_changed {
            self.live = None;
            self.last_finished = Some(now);
            self.loading = false;
            true
        } else {
            changed && self.hide_loading()
        }
    }
}

/// The notify RPC has no command id. Only the extension's known batch
/// failure messages settle a live lookup; other notices just hide loading.
pub fn is_all_usage_failure_notice(message: &str) -> bool {
    matches!(
        message,
        "Could not list saved accounts." | "No se pudieron listar las cuentas guardadas."
    ) || [
        "Accounts with unavailable Codex quota: ",
        "Cuentas con cuota de Codex no disponible: ",
    ]
    .iter()
    .any(|prefix| {
        message
            .strip_prefix(prefix)
            .and_then(|count| count.strip_suffix('.'))
            .is_some_and(|count| count.parse::<usize>().is_ok_and(|count| count > 0))
    })
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
    fn all_usage_auto_refresh_uses_completion_time_and_manual_refresh_bypasses_it() {
        let feed = parse(r#"{"v":1,"active":null,"accounts":[]}"#).unwrap();
        let now = std::time::Instant::now();
        let mut usage = super::AllUsage::default();
        assert!(usage.start(&feed, false, now));
        assert!(!usage.start(&feed, false, now));
        assert!(!usage.start(&feed, true, now));
        let finished = now + Duration::from_secs(30);
        assert!(usage.finish(Some(&feed), false, finished));
        assert!(!usage.start(&feed, false, finished + Duration::from_millis(59_999)));
        assert!(usage.start(&feed, false, finished + Duration::from_secs(60)));
        assert!(usage.finish(Some(&feed), true, finished + Duration::from_secs(60)));
        assert!(usage.start(&feed, true, finished + Duration::from_secs(60)));
    }

    #[test]
    fn all_usage_loading_ends_only_on_quota_change_notice_or_timeout() {
        let feed = |quota, active| {
            parse(
                &json!({"v":1,"active":null,"accounts":[
                    {"name":"Work","active":active,"binding":null,"quota":quota}
                ]})
                .to_string(),
            )
            .unwrap()
        };
        let before = feed(None, false);
        let now = std::time::Instant::now();
        let mut usage = super::AllUsage::default();
        assert!(usage.start(&before, false, now));
        let almost = now + Duration::from_millis(29_999);
        assert!(!usage.finish(Some(&before), false, almost));
        assert!(!usage.finish(Some(&feed(None, true)), false, almost));
        assert!(!usage.finish(None, false, almost));
        assert!(usage.pending());
        assert!(usage.finish(
            Some(&feed(Some("5h 75% left · weekly 40% left"), true)),
            false,
            almost
        ));
        assert!(!usage.pending());
        assert!(!usage.finish(Some(&before), true, almost));
        assert!(usage.start(&before, true, almost));
        assert!(usage.finish(Some(&before), true, almost));
        assert!(usage.start(&before, true, now + Duration::from_secs(30)));
        assert!(usage.finish(Some(&before), false, now + Duration::from_secs(60)));
        assert!(!usage.pending());
    }

    #[test]
    fn progressive_rows_do_not_finish_a_live_run_or_start_its_cooldown() {
        let feed = |one, two| {
            parse(
                &json!({"v":1,"active":null,"accounts":[
                    {"name":"One","active":false,"binding":null,"quota":one},
                    {"name":"Two","active":false,"binding":null,"quota":two}
                ]})
                .to_string(),
            )
            .unwrap()
        };
        let before = feed(None, None);
        let first = feed(Some("5h 70% left"), None);
        let all = feed(Some("5h 70% left"), Some("5h 60% left"));
        let now = std::time::Instant::now();
        let at = |s| now + Duration::from_secs(s);
        let mut usage = super::AllUsage::default();
        assert!(usage.start(&before, false, at(0)));
        assert!(usage.finish(Some(&first), false, at(1)));
        assert!(!usage.pending(), "first row stops the spinner");
        assert!(
            !usage.start(&first, true, at(2)),
            "refresh must not overlap the batch"
        );
        assert!(!usage.start(&first, false, at(2)));
        assert!(!usage.hide_loading());
        assert!(usage.live());
        assert!(usage.finish(Some(&all), false, at(25)));
        assert!(!usage.start(&all, false, at(61)));
        assert!(!usage.start(&all, false, at(84)));
        assert!(usage.start(&all, false, at(85)));
    }

    #[test]
    fn only_known_batch_failure_notices_end_a_live_lookup() {
        for (text, expected) in [
            ("Accounts with unavailable Codex quota: 2.", true),
            ("Cuentas con cuota de Codex no disponible: 1.", true),
            ("Could not list saved accounts.", true),
            ("No se pudieron listar las cuentas guardadas.", true),
            ("Codex quota unavailable.", false),
            ("Accounts with unavailable Codex quota: 0.", false),
            ("Accounts with unavailable Codex quota: many.", false),
        ] {
            assert_eq!(super::is_all_usage_failure_notice(text), expected, "{text}");
        }
    }

    #[test]
    fn parses_empty_accounts() {
        let feed = parse(r#"{"v":1,"active":null,"accounts":[]}"#).unwrap();
        assert_eq!(feed.active, None);
        assert!(feed.accounts.is_empty());
    }
}
