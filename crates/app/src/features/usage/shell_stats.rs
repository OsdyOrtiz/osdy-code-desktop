//! Local stats from Gentle Shell itself (`get_usage_stats`, the same history
//! and aggregation `/gentle:stats` shows), mapped onto the tab's summary.
//! Used when the active agent tab's Shell advertises the method; the file
//! scan in `stats` stays the path for every other Shell.

use gentle_core::usage::stats::{
    ModelStats, ProjectStats, ProviderStats, StatsRange, StatsSummary, TokenTotals,
    chart_from_days, parse_day_key, today,
};
use pi_rpc::desktop_rpc::{StatsRange as RpcStatsRange, UsageStats};

/// The wire range for the tab's range.
pub(super) fn rpc_range(range: StatsRange) -> RpcStatsRange {
    match range {
        StatsRange::All => RpcStatsRange::All,
        StatsRange::Month => RpcStatsRange::Month,
        StatsRange::Week => RpcStatsRange::Week,
    }
}

/// The tab's summary from Gentle Shell's answer for `range`. The answer has
/// no provider per model, so that stays empty; Gentle Shell 5.0.0 on adds the
/// cost per day and the providers (an older Shell's days chart tokens only,
/// and it lists no providers). Its days cover only the range, so a week
/// charts seven days. `byProject` is present because the tab always asks for
/// every project.
pub(super) fn shell_summary(
    stats: &UsageStats,
    range: StatsRange,
    now: i64,
    offset_secs: i64,
) -> StatsSummary {
    let totals = &stats.totals;
    let rows: Vec<(i64, u64, f64)> = stats
        .by_day
        .iter()
        .filter_map(|day| {
            Some((
                parse_day_key(&day.day)?,
                day.tokens,
                day.cost.unwrap_or(0.0),
            ))
        })
        .collect();
    StatsSummary {
        totals: TokenTotals {
            input: totals.input_tokens,
            output: totals.output_tokens,
            cache_read: totals.cache_read_tokens,
            cache_write: totals.cache_write_tokens,
            total: totals.total_tokens,
            cost: totals.cost,
        },
        messages: totals.messages as usize,
        sessions: totals.sessions as usize,
        active_days: stats.by_day.iter().filter(|day| day.tokens > 0).count(),
        models: stats
            .by_model
            .iter()
            .map(|model| ModelStats {
                model: model.model.clone(),
                provider: String::new(),
                tokens: model.tokens,
                cost: model.cost,
                messages: model.messages as usize,
                share: model.share,
            })
            .collect(),
        projects: stats
            .by_project
            .iter()
            .flatten()
            .map(|project| ProjectStats {
                project: project.name.clone(),
                sessions: project.sessions as usize,
                messages: project.messages as usize,
                tokens: project.tokens,
                cost: project.cost,
            })
            .collect(),
        providers: stats.by_provider.as_ref().map(|providers| {
            providers
                .iter()
                .map(|provider| ProviderStats {
                    provider: provider.provider.clone(),
                    messages: provider.messages as usize,
                    tokens: provider.tokens,
                    cost: provider.cost,
                    share: provider.share,
                })
                .collect()
        }),
        daily: chart_from_days(&rows, today(now, offset_secs), range.chart_days()),
    }
}

/// Whether the answer's days carry their cost (Gentle Shell 5.0.0 on), so
/// the chart can show cost: every day row has one, and an answer without
/// rows counts only when it is from a Shell that also lists providers.
pub(super) fn has_day_cost(stats: &UsageStats) -> bool {
    stats.by_day.iter().all(|day| day.cost.is_some())
        && (stats.by_provider.is_some() || !stats.by_day.is_empty())
}

/// What the tab says about rows Gentle Shell left out (totals still count
/// them).
pub(super) fn shell_notes(stats: &UsageStats) -> Vec<&'static str> {
    let mut notes = Vec::new();
    if stats.truncated.by_model {
        notes.push("Only the 50 largest models are listed; the totals count every model.");
    }
    if stats.truncated.by_project == Some(true) {
        notes.push("Only the 50 largest projects are listed; the totals count every project.");
    }
    if stats.truncated.by_provider == Some(true) {
        notes.push("Only the 50 largest providers are listed; the totals count every provider.");
    }
    if stats.truncated.by_day {
        notes.push("Active days count only the 400 most recent days with usage.");
    }
    notes
}

#[cfg(test)]
mod tests {
    use gentle_core::usage::stats::StatsRange;
    use gentle_core::usage::time::{DAY, days_from_civil};
    use pi_rpc::desktop_rpc::{DesktopRecord, SCHEMA, UsageStats, decode_payload};

    use super::{has_day_cost, shell_notes, shell_summary};

    /// Decode through the adapter's own parser, so the mapping stays on the
    /// real wire shape (the spec's example).
    fn stats(data: serde_json::Value) -> UsageStats {
        let line = serde_json::json!({
            "schema": SCHEMA, "kind": "response", "requestId": "desktop-61", "ok": true, "data": data,
        })
        .to_string();
        match decode_payload(&line).unwrap() {
            DesktopRecord::Response(response) => response.usage_stats().unwrap(),
            other => panic!("expected a response, got {other:?}"),
        }
    }

    fn example() -> UsageStats {
        stats(serde_json::json!({
            "range": "30d",
            "scope": "all",
            "totals": { "sessions": 3, "messages": 4, "inputTokens": 1310, "outputTokens": 155, "cacheReadTokens": 1000, "cacheWriteTokens": 200, "totalTokens": 2665, "cost": 1.76 },
            "byModel": [
                { "model": "claude-opus-5-5", "messages": 2, "tokens": 1365, "cost": 0.51, "share": 0.512 },
                { "model": "gpt-5.5", "messages": 2, "tokens": 1300, "cost": 1.25, "share": 0.488 }
            ],
            "byDay": [{ "day": "2026-09-20", "tokens": 1000 }, { "day": "2026-10-08", "tokens": 300 }, { "day": "2026-10-09", "tokens": 1365 }],
            "byProject": [
                { "name": "alpha", "sessions": 2, "messages": 3, "tokens": 2365, "cost": 1.51 },
                { "name": "beta", "sessions": 1, "messages": 1, "tokens": 300, "cost": 0.25 }
            ],
            "truncated": { "byModel": false, "byDay": false, "byProject": true }
        }))
    }

    #[test]
    fn the_spec_example_maps_onto_the_summary() {
        let now = (days_from_civil(2026, 10, 9) * DAY + 12 * 3600) * 1000;
        let summary = shell_summary(&example(), StatsRange::Month, now, 0);
        assert_eq!(summary.totals.total, 2665);
        assert_eq!(summary.totals.input, 1310);
        assert_eq!(summary.totals.cache_write, 200);
        assert!((summary.totals.cost - 1.76).abs() < 1e-9);
        assert_eq!((summary.sessions, summary.messages), (3, 4));
        assert_eq!(summary.active_days, 3);
        assert_eq!(summary.models.len(), 2);
        assert_eq!(summary.models[0].model, "claude-opus-5-5");
        assert!(summary.models[0].provider.is_empty());
        assert_eq!(summary.projects[1].project, "beta");
        assert_eq!(summary.projects[0].sessions, 2);
        // Thirty days ending today; 2026-09-20 is 19 days back.
        assert_eq!(summary.daily.len(), 30);
        assert_eq!(summary.daily[29].tokens, 1365);
        assert_eq!(summary.daily[28].tokens, 300);
        assert_eq!(summary.daily[10].tokens, 1000);
        // A Shell older than 5.0.0: tokens only, no providers.
        assert!(summary.daily.iter().all(|d| d.cost == 0.0));
        assert_eq!(summary.providers, None);
        assert!(!has_day_cost(&example()));
    }

    #[test]
    fn a_week_charts_seven_days() {
        let now = days_from_civil(2026, 10, 9) * DAY * 1000;
        let summary = shell_summary(&example(), StatsRange::Week, now, 0);
        assert_eq!(summary.daily.len(), 7);
        assert_eq!(summary.daily.iter().map(|d| d.tokens).sum::<u64>(), 1665);
    }

    #[test]
    fn truncation_is_said_per_array() {
        let notes = shell_notes(&example());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("projects"));
    }

    /// The 5.0.0 spec example: the same answer plus cost per day and
    /// `byProvider`.
    fn example_5_0() -> UsageStats {
        stats(serde_json::json!({
            "range": "30d",
            "scope": "all",
            "totals": { "sessions": 3, "messages": 4, "inputTokens": 1310, "outputTokens": 155, "cacheReadTokens": 1000, "cacheWriteTokens": 200, "totalTokens": 2665, "cost": 1.76 },
            "byModel": [
                { "model": "claude-opus-5-5", "messages": 2, "tokens": 1365, "cost": 0.51, "share": 0.512 },
                { "model": "gpt-5.5", "messages": 2, "tokens": 1300, "cost": 1.25, "share": 0.488 }
            ],
            "byDay": [{ "day": "2026-09-20", "tokens": 1000, "cost": 1 }, { "day": "2026-10-08", "tokens": 300, "cost": 0.25 }, { "day": "2026-10-09", "tokens": 1365, "cost": 0.51 }],
            "byProvider": [{ "provider": "anthropic", "messages": 4, "tokens": 2665, "cost": 1.76, "share": 1 }],
            "byProject": [
                { "name": "alpha", "sessions": 2, "messages": 3, "tokens": 2365, "cost": 1.51 },
                { "name": "beta", "sessions": 1, "messages": 1, "tokens": 300, "cost": 0.25 }
            ],
            "truncated": { "byModel": false, "byDay": false, "byProvider": true, "byProject": false }
        }))
    }

    #[test]
    fn days_carry_their_cost_and_providers_are_listed_from_5_0() {
        let stats = example_5_0();
        assert!(has_day_cost(&stats));
        let now = (days_from_civil(2026, 10, 9) * DAY + 12 * 3600) * 1000;
        let summary = shell_summary(&stats, StatsRange::Month, now, 0);
        assert_eq!(summary.daily[29].cost, 0.51);
        assert_eq!(summary.daily[28].cost, 0.25);
        assert_eq!(summary.daily[10].cost, 1.0);
        let providers = summary.providers.unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].provider, "anthropic");
        assert_eq!((providers[0].messages, providers[0].tokens), (4, 2665));
        assert!((providers[0].cost - 1.76).abs() < 1e-9);
        assert_eq!(providers[0].share, 1.0);
        let notes = shell_notes(&stats);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("providers"));

        // A 5.0.0 answer for a range without usage still says its days
        // carry cost: it lists providers (none).
        let empty = stats_with(|data| {
            data["byDay"] = serde_json::json!([]);
            data["byProvider"] = serde_json::json!([]);
        });
        assert!(has_day_cost(&empty));
        let older_empty = stats_with(|data| {
            data["byDay"] = serde_json::json!([]);
            data.as_object_mut().unwrap().remove("byProvider");
        });
        assert!(!has_day_cost(&older_empty));
    }

    fn stats_with(edit: impl FnOnce(&mut serde_json::Value)) -> UsageStats {
        let mut data = serde_json::json!({
            "range": "7d", "scope": "all",
            "totals": { "sessions": 0, "messages": 0, "inputTokens": 0, "outputTokens": 0, "cacheReadTokens": 0, "cacheWriteTokens": 0, "totalTokens": 0, "cost": 0 },
            "byModel": [], "byDay": [], "byProject": [],
            "truncated": { "byModel": false, "byDay": false, "byProject": false }
        });
        edit(&mut data);
        stats(data)
    }
}
