//! Provider subscription windows (`lib/shell-usage.ts`): the Codex
//! `wham/usage` and NaN Cloud quota payloads, row grouping and the Codex
//! token's account claim.

use serde_json::{Map, Value};

use crate::i18n::{Language, tr};

use super::finite;
use super::time::{DAY, HOUR, MINUTE, WEEK, local_day, parse_iso_ms};

pub const CODEX_PROVIDER: &str = "openai-codex";
pub const OPENAI_PROVIDER: &str = "openai";
pub const ANTHROPIC_PROVIDER: &str = "anthropic";
pub const NAN_PROVIDER: &str = "nan";
const CODEX_MAIN_LIMIT: &str = "codex";
const CODEX_ACCOUNT_CLAIM: &str = "https://api.openai.com/auth";
const NAN_DEFAULT_WINDOW_TOKENS: f64 = 400_000_000.0;
const NAN_DEFAULT_WINDOW_HOURS: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindow {
    /// "5h", "week", … Empty for NaN's billing-period allowance.
    pub label: String,
    pub used_percent: f64,
    pub window_seconds: f64,
    /// Unix milliseconds.
    pub reset_at: Option<i64>,
    /// Raw allowance numbers; only NaN reports them.
    pub used: Option<f64>,
    pub budget: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageLimit {
    pub name: String,
    pub windows: Vec<UsageWindow>,
    pub limit_reached: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderUsage {
    pub provider: String,
    pub plan: Option<String>,
    pub limits: Vec<UsageLimit>,
    pub fetched_at: i64,
}

pub fn window_label(seconds: i64) -> String {
    if seconds == WEEK {
        "week".into()
    } else if seconds >= DAY && seconds % DAY == 0 {
        format!("{}d", seconds / DAY)
    } else if seconds >= HOUR && seconds % HOUR == 0 {
        format!("{}h", seconds / HOUR)
    } else {
        format!("{}m", (seconds as f64 / MINUTE as f64).round() as i64)
    }
}

pub fn format_reset(reset_at: Option<i64>, now: i64) -> String {
    format_reset_in(Language::English, reset_at, now)
}

pub fn format_reset_in(language: Language, reset_at: Option<i64>, now: i64) -> String {
    let Some(reset_at) = reset_at else {
        return String::new();
    };
    let seconds = (reset_at - now).div_euclid(1000);
    if seconds <= 0 {
        tr(language, "resets now").into()
    } else if seconds < HOUR {
        let minutes = ((seconds as f64 / MINUTE as f64).round() as i64).max(1);
        tr(language, "resets in {minutes}m").replace("{minutes}", &minutes.to_string())
    } else if seconds < DAY {
        tr(language, "resets in {hours}h {minutes}m")
            .replace("{hours}", &(seconds / HOUR).to_string())
            .replace("{minutes}", &((seconds % HOUR) / MINUTE).to_string())
    } else {
        tr(language, "resets in {days}d {hours}h")
            .replace("{days}", &(seconds / DAY).to_string())
            .replace("{hours}", &((seconds % DAY) / HOUR).to_string())
    }
}

/// How alarming one used percent is: calm below 80, warning from 80,
/// danger from 95. The plan-limits chip, the legacy limit chips and the
/// usage meters share these thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitTone {
    Calm,
    Warning,
    Danger,
}

pub fn limit_tone(percent: f64) -> LimitTone {
    if percent >= 95.0 {
        LimitTone::Danger
    } else if percent >= 80.0 {
        LimitTone::Warning
    } else {
        LimitTone::Calm
    }
}

/// The canonical provider id behind a model's provider name: Codex reports
/// as `codex` or `openai-codex` and Claude as `claude`, `anthropic` or
/// `claude-bridge` (Claude Code's subscription through the bridge, whose
/// 5h/7d windows Gentle Shell files under the provider that served the
/// response), so either side of a model-provider → subscription-provider
/// match uses this; anything else has no subscription windows to match.
pub fn provider_alias(provider: &str) -> Option<&'static str> {
    match provider {
        "codex" | "openai-codex" => Some("codex"),
        "claude" | "anthropic" | "claude-bridge" => Some("claude"),
        "openai" => Some("openai"),
        "nan" => Some("nan"),
        _ => None,
    }
}

/// A provider's windows flattened across its limits, most used first (the
/// payload order stands on ties), so a compact view leads with the window
/// that is about to bite.
pub fn windows_most_used(limits: &[UsageLimit]) -> Vec<UsageWindow> {
    let mut windows: Vec<UsageWindow> = limits
        .iter()
        .flat_map(|limit| limit.windows.iter().cloned())
        .collect();
    windows.sort_by(|a, b| b.used_percent.total_cmp(&a.used_percent));
    windows
}

/// A provider's windows flattened across its limits, shortest first (the
/// payload order stands on ties), so the plan card reads its 5-hour window
/// above its weekly one while the trigger still shows the most-used window.
/// Windows without a declared length (NaN's billing-period allowance has no
/// label, and its `window_seconds` count down to the reset) go last.
pub fn windows_by_length(limits: &[UsageLimit]) -> Vec<UsageWindow> {
    let length = |window: &UsageWindow| {
        if window.window_seconds > 0.0 && !window.label.is_empty() {
            Some(window.window_seconds)
        } else {
            None
        }
    };
    let mut windows: Vec<UsageWindow> = limits
        .iter()
        .flat_map(|limit| limit.windows.iter().cloned())
        .collect();
    // `sort_by` is stable, so ties and the unknown tail keep the payload's
    // order.
    windows.sort_by(|a, b| match (length(a), length(b)) {
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(a), Some(b)) => a.total_cmp(&b),
        (None, None) => std::cmp::Ordering::Equal,
    });
    windows
}

/// A window's row label: friendly for the two windows a plan cares about —
/// a 5-hour window reads "5-hour limit" and a week reads "Weekly · all
/// models" — the payload's own label otherwise, and "period" for NaN's
/// billing-period allowance (whose `window_seconds` counts down to the
/// reset instead of naming a window length).
pub fn window_display_label(window: &UsageWindow) -> String {
    window_display_label_in(Language::English, window)
}

pub fn window_display_label_in(language: Language, window: &UsageWindow) -> String {
    if window.label.is_empty() {
        return tr(language, "period").into();
    }
    let week =
        window.label == "week" || window.label == "7d" || window.window_seconds >= WEEK as f64;
    let five_hours = window.label == "5h"
        || (window.window_seconds > 0.0 && window.window_seconds == 5.0 * HOUR as f64);
    if week {
        tr(language, "Weekly · all models").into()
    } else if five_hours {
        tr(language, "5-hour limit").into()
    } else {
        window.label.clone()
    }
}

/// When this window resets, in the reader's local clock (`offset_secs` from
/// UTC): a reset later the same local day reads "resets at 5:30 p.m.", a
/// later day reads "resets Sun 6:00 a.m.", and a past or unknown reset says
/// nothing.
pub fn reset_clock_label(reset_at: Option<i64>, now: i64, offset_secs: i64) -> String {
    reset_clock_label_in(Language::English, reset_at, now, offset_secs)
}

pub fn reset_clock_label_in(
    language: Language,
    reset_at: Option<i64>,
    now: i64,
    offset_secs: i64,
) -> String {
    let Some(reset_at) = reset_at.filter(|at| *at > now) else {
        return String::new();
    };
    let local_secs = |ms: i64| ms.div_euclid(1000) + offset_secs;
    let clock = |ms: i64| {
        let time_of_day = local_secs(ms).rem_euclid(DAY);
        let (mut hour, minute) = (time_of_day / HOUR, (time_of_day % HOUR) / MINUTE);
        let meridiem = if hour >= 12 { "p.m." } else { "a.m." };
        if hour >= 12 {
            hour -= 12;
        }
        if hour == 0 {
            hour = 12;
        }
        format!("{hour}:{minute:02} {meridiem}")
    };
    if local_day(reset_at, offset_secs) == local_day(now, offset_secs) {
        tr(language, "resets at {time}").replace("{time}", &clock(reset_at))
    } else {
        const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        // 1970-01-01 (day 0) was a Thursday, four days into the table.
        let weekday = (local_day(reset_at, offset_secs) + 4).rem_euclid(7) as usize;
        tr(language, "resets {weekday} {time}")
            .replace("{weekday}", tr(language, WEEKDAYS[weekday]))
            .replace("{time}", &clock(reset_at))
    }
}

fn parse_codex_window(raw: Option<&Value>, now: i64) -> Option<UsageWindow> {
    let raw = raw?.as_object()?;
    let used = finite(raw.get("used_percent"))?;
    let seconds = finite(raw.get("limit_window_seconds")).filter(|s| *s > 0.0)?;
    let reset_at = match (
        raw.get("reset_at").and_then(Value::as_f64),
        raw.get("reset_after_seconds").and_then(Value::as_f64),
    ) {
        (Some(at), _) => Some((at * 1000.0) as i64),
        (None, Some(after)) => Some(now + (after * 1000.0) as i64),
        _ => None,
    };
    Some(UsageWindow {
        label: window_label(seconds.round() as i64),
        used_percent: used,
        window_seconds: seconds,
        reset_at,
        used: None,
        budget: None,
    })
}

fn parse_codex_rate_limit(name: &str, raw: Option<&Value>, now: i64) -> Option<UsageLimit> {
    let raw = raw?.as_object()?;
    let windows: Vec<UsageWindow> = [raw.get("primary_window"), raw.get("secondary_window")]
        .into_iter()
        .filter_map(|window| parse_codex_window(window, now))
        .collect();
    if windows.is_empty() {
        return None;
    }
    Some(UsageLimit {
        name: name.to_string(),
        windows,
        limit_reached: raw.get("limit_reached") == Some(&Value::Bool(true)),
    })
}

/// The `wham/usage` payload: the main Codex limit plus any additional ones.
pub fn parse_codex_usage(payload: &Value, now: i64) -> ProviderUsage {
    let mut limits = Vec::new();
    if let Some(main) = parse_codex_rate_limit(CODEX_MAIN_LIMIT, payload.get("rate_limit"), now) {
        limits.push(main);
    }
    for extra in payload
        .get("additional_rate_limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = extra
            .get("limit_name")
            .and_then(Value::as_str)
            .unwrap_or("limit");
        if let Some(limit) = parse_codex_rate_limit(name, extra.get("rate_limit"), now) {
            limits.push(limit);
        }
    }
    ProviderUsage {
        provider: CODEX_PROVIDER.into(),
        plan: payload
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_string),
        limits,
        fetched_at: now,
    }
}

fn quota_timestamp(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n
            .as_f64()
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| (v * 1000.0) as i64),
        Value::String(s) => parse_iso_ms(s),
        _ => None,
    }
}

fn quota_number(value: Option<&Value>, positive: bool) -> Option<f64> {
    let v = finite(value)?;
    if positive {
        (v > 0.0).then_some(v)
    } else {
        (v >= 0.0).then_some(v)
    }
}

fn nan_rolling_window(raw: &Map<String, Value>) -> Option<UsageWindow> {
    let used = quota_number(raw.get("windowTokensUsed"), false)?;
    let budget = quota_number(raw.get("fullWindowTokens"), true)
        .or_else(|| quota_number(raw.get("windowTokens"), true))
        .unwrap_or(NAN_DEFAULT_WINDOW_TOKENS);
    let hours = quota_number(raw.get("windowHours"), true).unwrap_or(NAN_DEFAULT_WINDOW_HOURS);
    let seconds = hours * HOUR as f64;
    Some(UsageWindow {
        label: window_label(seconds.round() as i64),
        used_percent: used / budget * 100.0,
        window_seconds: seconds,
        reset_at: quota_timestamp(raw.get("windowResetsAt")),
        used: None,
        budget: None,
    })
}

/// NaN Cloud's quota payload: one billing-period allowance per metered model
/// plus its rolling window. A metered model whose usage is unreadable makes
/// the whole read empty (schema drift), exactly like the source.
pub fn parse_nan_quota(payload: &Value, now: i64) -> ProviderUsage {
    let empty = |limits| ProviderUsage {
        provider: NAN_PROVIDER.into(),
        plan: None,
        limits,
        fetched_at: now,
    };
    let fallback_reset = quota_timestamp(payload.get("periodEnd"));
    let mut limits = Vec::new();
    for entry in payload
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(model) = entry.as_object() else {
            continue;
        };
        let Some(name) = model
            .get("model")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(allowance) = quota_number(model.get("fullCap"), true)
            .or_else(|| quota_number(model.get("cap"), true))
        else {
            continue;
        };
        let Some(tokens_used) = quota_number(model.get("tokensUsed"), false) else {
            return empty(Vec::new());
        };
        let reset_at = quota_timestamp(model.get("periodEnd")).or(fallback_reset);
        let mut windows = vec![UsageWindow {
            label: String::new(),
            used_percent: tokens_used / allowance * 100.0,
            window_seconds: reset_at
                .map_or(0.0, |at| ((at - now) as f64 / 1000.0).round().max(0.0)),
            reset_at,
            used: Some(tokens_used),
            budget: Some(allowance),
        }];
        windows.extend(nan_rolling_window(model));
        limits.push(UsageLimit {
            name: name.to_string(),
            windows,
            limit_reached: tokens_used >= allowance,
        });
    }
    empty(limits)
}

fn raw_allowance(limit: &UsageLimit) -> Option<(f64, f64)> {
    let first = limit.windows.first()?;
    match (first.used, first.budget) {
        (Some(used), Some(budget)) if budget > 0.0 => Some((used, budget)),
        _ => None,
    }
}

/// The leading alphabetic run of a model id: `glm5.3-flash` → `glm`.
pub fn model_family(model_id: &str) -> String {
    let run: String = model_id
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if run.is_empty() {
        model_id.to_lowercase()
    } else {
        run.to_lowercase()
    }
}

/// NaN rows: families together, families by Σused/Σbudget, members most used
/// first. Providers without raw allowances keep their payload order.
pub fn group_usage_limits(limits: &[UsageLimit]) -> Vec<UsageLimit> {
    if limits.is_empty() || !limits.iter().all(|l| raw_allowance(l).is_some()) {
        return limits.to_vec();
    }
    let percent = |l: &UsageLimit| l.windows.first().map_or(0.0, |w| w.used_percent);
    let mut families: Vec<(String, Vec<UsageLimit>)> = Vec::new();
    for limit in limits {
        let family = model_family(&limit.name);
        match families.iter_mut().find(|(f, _)| *f == family) {
            Some((_, members)) => members.push(limit.clone()),
            None => families.push((family, vec![limit.clone()])),
        }
    }
    let mut ranked: Vec<(f64, Vec<UsageLimit>)> = families
        .into_iter()
        .map(|(_, mut members)| {
            members.sort_by(|a, b| percent(b).total_cmp(&percent(a)));
            let (used, budget) = members
                .iter()
                .filter_map(raw_allowance)
                .fold((0.0, 0.0), |(u, b), (mu, mb)| (u + mu, b + mb));
            let share = if budget > 0.0 {
                used / budget * 100.0
            } else {
                percent(&members[0])
            };
            (share, members)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranked
        .into_iter()
        .flat_map(|(_, members)| members)
        .collect()
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The ChatGPT account id claim inside a Codex access token.
pub fn account_id_from_token(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let (Some(_), Some(payload), Some(_), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let claims: Value = serde_json::from_slice(&base64url_decode(payload)?).ok()?;
    claims
        .get(CODEX_ACCOUNT_CLAIM)?
        .get("chatgpt_account_id")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{
        LimitTone, UsageLimit, UsageWindow, group_usage_limits, limit_tone, parse_codex_usage,
        parse_nan_quota, provider_alias, reset_clock_label, window_display_label,
        windows_by_length, windows_most_used,
    };
    use crate::usage::time::{DAY, HOUR, MINUTE, WEEK, days_from_civil, parse_iso_ms};
    use serde_json::Value;
    use serde_json::json;

    const NOW: i64 = 1_762_400_000_000;

    fn window(label: &str, percent: f64) -> UsageWindow {
        UsageWindow {
            label: label.into(),
            used_percent: percent,
            window_seconds: 0.0,
            reset_at: None,
            used: None,
            budget: None,
        }
    }

    fn window_len(label: &str, percent: f64, seconds: f64) -> UsageWindow {
        UsageWindow {
            window_seconds: seconds,
            ..window(label, percent)
        }
    }

    #[test]
    fn tone_thresholds_are_80_and_95() {
        assert_eq!(limit_tone(0.0), LimitTone::Calm);
        assert_eq!(limit_tone(79.0), LimitTone::Calm);
        assert_eq!(limit_tone(79.9), LimitTone::Calm);
        assert_eq!(limit_tone(80.0), LimitTone::Warning);
        assert_eq!(limit_tone(94.0), LimitTone::Warning);
        assert_eq!(limit_tone(94.9), LimitTone::Warning);
        assert_eq!(limit_tone(95.0), LimitTone::Danger);
        assert_eq!(limit_tone(100.0), LimitTone::Danger);
    }

    #[test]
    fn provider_aliases_match_the_same_account() {
        assert_eq!(provider_alias("codex"), Some("codex"));
        assert_eq!(provider_alias("openai-codex"), Some("codex"));
        assert_eq!(provider_alias("claude"), Some("claude"));
        assert_eq!(provider_alias("anthropic"), Some("claude"));
        assert_eq!(provider_alias("claude-bridge"), Some("claude"));
        assert_eq!(provider_alias("openai"), Some("openai"));
        assert_eq!(provider_alias("nan"), Some("nan"));
        assert_eq!(provider_alias("deepseek"), None);
        assert_eq!(provider_alias(""), None);
    }

    #[test]
    fn reset_clock_speaks_the_local_day() {
        // 2025-11-06 is a Thursday; 2025-11-07 a Friday.
        let day = days_from_civil(2025, 11, 6);
        let now = day * DAY * 1000 + (10 * HOUR + 30 * MINUTE) * 1000;
        // Same local day: a plain clock time.
        assert_eq!(
            reset_clock_label(Some(now + 7 * HOUR * 1000), now, 0),
            "resets at 5:30 p.m."
        );
        // A later day: short weekday first.
        assert_eq!(
            reset_clock_label(
                Some(now + DAY * 1000 - (4 * HOUR + 30 * MINUTE) * 1000),
                now,
                0
            ),
            "resets Fri 6:00 a.m."
        );
        // Midnight reads as 12 a.m., noon as 12 p.m.
        assert_eq!(
            reset_clock_label(
                Some(now + DAY * 1000 + 13 * HOUR * 1000 + 30 * MINUTE * 1000),
                now,
                0
            ),
            "resets Sat 12:00 a.m."
        );
        assert_eq!(
            reset_clock_label(Some(now + 2 * HOUR * 1000), now, 0),
            "resets at 12:30 p.m."
        );
        // A past or unknown reset says nothing.
        assert_eq!(reset_clock_label(Some(now - 1), now, 0), "");
        assert_eq!(reset_clock_label(Some(now), now, 0), "");
        assert_eq!(reset_clock_label(None, now, 0), "");
        // The offset moves the local day: 10:30 UTC with a -5h offset is
        // 5:30 a.m. local, and a 17:30 UTC reset lands on the same local day.
        let offset = -5 * HOUR;
        assert_eq!(
            reset_clock_label(Some(now + 7 * HOUR * 1000), now, offset),
            "resets at 12:30 p.m."
        );
        // Just before local midnight: the reset lands on the next day.
        let utc_now = day * DAY * 1000 + 3 * HOUR * 1000; // local 22:00 the day before
        assert_eq!(
            reset_clock_label(Some(day * DAY * 1000 + 9 * HOUR * 1000), utc_now, offset),
            "resets Thu 4:00 a.m."
        );
    }

    #[test]
    fn windows_order_most_used_first() {
        let limits = vec![
            UsageLimit {
                name: "prompts".into(),
                windows: vec![window("5h", 42.0), window("week", 10.0)],
                limit_reached: false,
            },
            UsageLimit {
                name: "codex".into(),
                windows: vec![window("1d", 77.5), window("5h", 42.0)],
                limit_reached: false,
            },
        ];
        let ordered: Vec<f64> = windows_most_used(&limits)
            .iter()
            .map(|w| w.used_percent)
            .collect();
        assert_eq!(ordered, vec![77.5, 42.0, 42.0, 10.0]);
        assert!(windows_most_used(&[]).is_empty());
    }

    #[test]
    fn windows_order_shortest_first_with_unknown_last() {
        let limits = vec![
            UsageLimit {
                name: "prompts".into(),
                windows: vec![
                    window_len("week", 10.0, WEEK as f64),
                    window_len("5h", 42.0, 5.0 * HOUR as f64),
                ],
                limit_reached: false,
            },
            UsageLimit {
                name: "codex".into(),
                windows: vec![
                    window_len("1d", 77.5, DAY as f64),
                    window_len("4h", 3.0, 4.0 * HOUR as f64),
                ],
                limit_reached: false,
            },
        ];
        let ordered: Vec<String> = windows_by_length(&limits)
            .iter()
            .map(|w| w.label.clone())
            .collect();
        assert_eq!(ordered, ["4h", "5h", "1d", "week"], "shortest first");
        // Windows without a length (NaN's billing-period allowance) go last,
        // in the payload's order.
        let with_unknown = vec![UsageLimit {
            name: "glm".into(),
            windows: vec![
                window_len("4h", 3.0, 4.0 * HOUR as f64),
                window("", 25.0),
                window_len("1d", 77.5, DAY as f64),
            ],
            limit_reached: false,
        }];
        let ordered: Vec<String> = windows_by_length(&with_unknown)
            .iter()
            .map(|w| w.label.clone())
            .collect();
        assert_eq!(ordered, ["4h", "1d", ""], "unknown lengths last");
        assert!(windows_by_length(&[]).is_empty());
    }

    #[test]
    fn window_labels_speak_like_the_plan_card() {
        // The two windows Claude reports read like the plan, everything else
        // keeps the payload's label ("period" for NaN's billing-period
        // allowance, whose `window_seconds` counts down instead).
        assert_eq!(window_display_label(&window("5h", 0.0)), "5-hour limit");
        assert_eq!(
            window_display_label(&window("week", 0.0)),
            "Weekly · all models"
        );
        assert_eq!(
            window_display_label(&window("7d", 0.0)),
            "Weekly · all models"
        );
        assert_eq!(window_display_label(&window("1d", 0.0)), "1d");
        assert_eq!(window_display_label(&window("4h", 0.0)), "4h");
        assert_eq!(window_display_label(&window("", 0.0)), "period");
        // NaN's billing-period allowance keeps "period" even when its
        // remaining time coincides with a window length: only a payload that
        // names the window gets the friendly name.
        let five_hours_left = UsageWindow {
            window_seconds: 5.0 * HOUR as f64,
            ..window("", 0.0)
        };
        assert_eq!(window_display_label(&five_hours_left), "period");
    }

    #[test]
    fn codex_usage_parses_main_and_additional_limits() {
        let payload = json!({
            "plan_type": "plus",
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "primary_window": { "used_percent": 42, "limit_window_seconds": 18000, "reset_after_seconds": 3600, "reset_at": 1_762_403_600 },
                "secondary_window": { "used_percent": 81.5, "limit_window_seconds": 604800, "reset_after_seconds": 7200 }
            },
            "additional_rate_limits": [
                { "limit_name": "codex_other", "rate_limit": { "limit_reached": true, "primary_window": { "used_percent": 100, "limit_window_seconds": 86400 } } },
                { "limit_name": "empty", "rate_limit": { "primary_window": null, "secondary_window": null } },
                { "rate_limit": { "primary_window": { "used_percent": 5, "limit_window_seconds": 0 } } }
            ]
        });
        let usage = parse_codex_usage(&payload, NOW);
        assert_eq!(usage.provider, "openai-codex");
        assert_eq!(usage.plan.as_deref(), Some("plus"));
        assert_eq!(usage.limits.len(), 2);
        let main = &usage.limits[0];
        assert_eq!(main.name, "codex");
        assert!(!main.limit_reached);
        assert_eq!(main.windows[0].label, "5h");
        assert_eq!(main.windows[0].used_percent, 42.0);
        assert_eq!(
            main.windows[0].reset_at,
            Some(1_762_403_600_000),
            "reset_at wins over reset_after_seconds"
        );
        assert_eq!(main.windows[1].label, "week");
        assert_eq!(main.windows[1].reset_at, Some(NOW + 7_200_000));
        let extra = &usage.limits[1];
        assert_eq!(extra.name, "codex_other");
        assert!(extra.limit_reached);
        assert_eq!(extra.windows[0].label, "1d");
        assert_eq!(extra.windows[0].reset_at, None);
        assert!(parse_codex_usage(&Value::Null, NOW).limits.is_empty());
    }

    #[test]
    fn nan_quota_parses_period_and_rolling_windows() {
        let payload = json!({
            "periodEnd": "2025-11-30T00:00:00.000Z",
            "models": [
                { "model": "glm5.2", "cap": 500, "fullCap": 1000, "tokensUsed": 250,
                  "windowHours": 4, "fullWindowTokens": 200, "windowTokensUsed": 50, "windowResetsAt": 1_762_401_000 },
                { "model": "kimi-k2", "cap": 100, "tokensUsed": 100, "periodEnd": "2025-11-20T00:00:00Z" },
                { "model": "free-model", "tokensUsed": 9 },
                { "model": "", "cap": 10, "tokensUsed": 1 },
                "junk"
            ]
        });
        let usage = parse_nan_quota(&payload, NOW);
        assert_eq!(usage.provider, "nan");
        assert_eq!(
            usage.limits.len(),
            2,
            "unmetered and nameless models are skipped"
        );
        let glm = &usage.limits[0];
        assert_eq!(glm.name, "glm5.2");
        assert_eq!(glm.windows[0].label, "");
        assert_eq!(
            glm.windows[0].used_percent, 25.0,
            "fullCap is the allowance"
        );
        assert_eq!(glm.windows[0].budget, Some(1000.0));
        assert_eq!(
            glm.windows[0].reset_at,
            parse_iso_ms("2025-11-30T00:00:00Z"),
            "falls back to the payload periodEnd"
        );
        assert_eq!(glm.windows[1].label, "4h");
        assert_eq!(glm.windows[1].used_percent, 25.0);
        assert_eq!(glm.windows[1].reset_at, Some(1_762_401_000_000));
        assert!(!glm.limit_reached);
        let kimi = &usage.limits[1];
        assert!(kimi.limit_reached);
        assert_eq!(kimi.windows.len(), 1);
        assert_eq!(
            kimi.windows[0].reset_at,
            parse_iso_ms("2025-11-20T00:00:00Z")
        );

        // Rolling defaults: 400M tokens over 4h.
        let defaults = parse_nan_quota(
            &json!({ "models": [{ "model": "m", "cap": 10, "tokensUsed": 0, "windowTokensUsed": 40_000_000 }] }),
            NOW,
        );
        assert_eq!(defaults.limits[0].windows[1].used_percent, 10.0);
        assert_eq!(defaults.limits[0].windows[1].label, "4h");

        // A metered model with unreadable usage is drift: the whole read is empty.
        let drift = json!({ "models": [{ "model": "a", "cap": 10, "tokensUsed": 1 }, { "model": "b", "cap": 10, "tokensUsed": "x" }] });
        assert!(parse_nan_quota(&drift, NOW).limits.is_empty());

        // Grouping: kimi (100%) family before glm (25%).
        let grouped = group_usage_limits(&usage.limits);
        assert_eq!(grouped[0].name, "kimi-k2");
    }
}
