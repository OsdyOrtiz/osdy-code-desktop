//! The display-only account list published by the osdy-gentle extension.

use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const USAGE_COMMAND: &str = "osdy-usage";
pub const USAGE_TIMEOUT: Duration = Duration::from_secs(20);
pub const ALL_USAGE_DISPLAY_TIMEOUT: Duration = Duration::from_secs(30);
pub const ALL_USAGE_BATCH_TIMEOUT: Duration = Duration::from_secs(120);
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
    pub usage_run: Option<String>,
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
    live: Option<LiveUsage>,
    loading: bool,
    last_finished: Option<Instant>,
}

struct LiveUsage {
    id: String,
    started: Instant,
    quotas: Vec<QuotaSnapshot>,
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

fn fresh_usage_run() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    // The timestamp distinguishes app launches; the counter distinguishes
    // dispatches across tabs even when the system clock has not advanced.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{stamp:016x}{serial:016x}")
}

fn valid_usage_run(id: &str) -> bool {
    (1..=32).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl AllUsage {
    pub fn pending(&self) -> bool {
        self.loading
    }

    pub fn live(&self) -> bool {
        self.live.is_some()
    }

    pub fn run_id(&self) -> Option<&str> {
        self.live.as_ref().map(|run| run.id.as_str())
    }

    pub fn hide_loading(&mut self) -> bool {
        std::mem::take(&mut self.loading)
    }

    pub fn start(&mut self, feed: &Feed, force: bool, now: Instant) -> Option<String> {
        if self.live()
            || (!force
                && self
                    .last_finished
                    .is_some_and(|at| now - at < ALL_USAGE_FRESHNESS))
        {
            return None;
        }
        let id = fresh_usage_run();
        self.live = Some(LiveUsage {
            id: id.clone(),
            started: now,
            quotas: quotas(feed),
        });
        self.loading = true;
        Some(id)
    }

    /// Only a matching marker or the fallback can release the batch lock.
    /// Quota changes still clear the spinner without starting the debounce.
    pub fn observe(&mut self, feed: Option<&Feed>, now: Instant) -> bool {
        let Some(run) = &self.live else {
            return false;
        };
        let elapsed = now - run.started;
        if elapsed >= ALL_USAGE_BATCH_TIMEOUT
            || feed.and_then(|feed| feed.usage_run.as_deref()) == Some(run.id.as_str())
        {
            self.live = None;
            self.last_finished = Some(now);
            self.loading = false;
            return true;
        }
        self.loading
            && (elapsed >= ALL_USAGE_DISPLAY_TIMEOUT
                || feed.is_some_and(|feed| quotas(feed) != run.quotas))
            && self.hide_loading()
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
    let usage_run = object
        .get("usageRun")
        .and_then(Value::as_str)
        .filter(|id| valid_usage_run(id))
        .map(str::to_owned);
    Some(Feed {
        active,
        accounts,
        usage_run,
    })
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

    fn batch_feed(quota: Option<&str>, run: Option<&str>) -> super::Feed {
        parse(
            &json!({"v":1,"active":null,"usageRun":run,"accounts":[
                {"name":"Work","active":false,"binding":null,"quota":quota}
            ]})
            .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn parses_optional_valid_usage_run() {
        for id in ["-", "A-9", &"a".repeat(32)] {
            assert_eq!(batch_feed(None, Some(id)).usage_run.as_deref(), Some(id));
        }
        for value in [
            json!(""),
            json!("a".repeat(33)),
            json!("a_b"),
            json!("a b"),
            json!("é"),
            json!(1),
            json!(null),
        ] {
            let feed =
                parse(&json!({"v":1,"active":null,"accounts":[],"usageRun":value}).to_string())
                    .unwrap();
            assert_eq!(feed.usage_run, None, "{value}");
        }
    }

    #[test]
    fn unrelated_named_usage_must_not_complete_a_bulk_run() {
        let mut usage = super::AllUsage::default();
        let now = std::time::Instant::now();
        let at = |seconds| now + Duration::from_secs(seconds);
        let before = batch_feed(None, Some("older-run"));
        let run = usage.start(&before, false, now).unwrap();
        assert!(usage.pending());
        let named = batch_feed(Some("5h 64% left"), Some("older-run"));
        assert!(usage.observe(Some(&named), at(1)));
        assert!(!usage.pending());
        assert!(usage.live());
        assert!(usage.start(&named, true, at(2)).is_none());

        let completed = batch_feed(Some("5h 64% left"), Some(&run));
        assert!(usage.observe(Some(&completed), at(3)));
        assert!(!usage.live());
        assert!(usage.start(&completed, false, at(62)).is_none());
        let next = usage.start(&completed, false, at(63)).unwrap();
        assert_ne!(next, run);
        assert!(super::valid_usage_run(&next));
        assert!(!usage.observe(Some(&completed), at(64)));
        assert!(usage.live());
    }

    #[test]
    fn display_deadline_must_not_unlock_a_slow_live_batch() {
        let mut usage = super::AllUsage::default();
        let now = std::time::Instant::now();
        let at = |seconds| now + Duration::from_secs(seconds);
        let feed = batch_feed(None, None);
        let run = usage.start(&feed, false, now).unwrap();
        assert!(!usage.observe(Some(&feed), now + Duration::from_millis(29_999)));
        assert!(usage.pending());
        assert!(usage.observe(Some(&feed), at(30)));
        assert!(!usage.pending());
        assert!(usage.live());
        assert!(usage.start(&feed, true, at(31)).is_none());
        assert!(!usage.observe(Some(&feed), now + Duration::from_millis(119_999)));
        assert!(usage.live());
        assert!(usage.observe(Some(&feed), at(120)));
        assert!(!usage.live());
        assert!(usage.start(&feed, false, at(179)).is_none());
        let next = usage.start(&feed, false, at(180)).unwrap();
        assert_ne!(next, run);
    }

    #[test]
    fn notices_hide_loading_and_manual_refresh_bypasses_only_the_debounce() {
        let mut usage = super::AllUsage::default();
        let now = std::time::Instant::now();
        let feed = batch_feed(None, None);
        let run = usage.start(&feed, false, now).unwrap();
        assert!(usage.hide_loading());
        assert!(!usage.pending());
        assert!(usage.start(&feed, true, now).is_none());
        assert!(!usage.observe(None, now));
        assert!(usage.observe(Some(&batch_feed(None, Some(&run))), now));
        assert!(usage.start(&feed, false, now).is_none());
        assert!(usage.start(&feed, true, now).is_some());
    }

    #[test]
    fn parses_empty_accounts() {
        let feed = parse(r#"{"v":1,"active":null,"accounts":[]}"#).unwrap();
        assert_eq!(feed.active, None);
        assert!(feed.accounts.is_empty());
    }
}
