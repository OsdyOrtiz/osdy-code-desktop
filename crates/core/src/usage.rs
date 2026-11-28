//! Usage: provider subscription windows (`/gentle:usage`) and local token
//! and cost totals (`/gentle:stats`).
//!
//! Parsing mirrors gentle-pi's `lib/shell-usage.ts` and `lib/stats-collector.ts`.
//! The app reads `auth.json`, calls the providers, walks the session files
//! and reads the clock; this module only parses and aggregates what they return.

use serde_json::Value;

pub mod stats;
pub mod subscription;
pub mod time;

/// The used percentage shown in the toolbar, from an actual five-hour
/// subscription window rather than the most-used window or a quota summary.
pub fn five_hour_used_percent(limits: &[subscription::UsageLimit]) -> Option<i64> {
    limits
        .iter()
        .flat_map(|limit| &limit.windows)
        .find(|window| subscription::is_five_hour_window(window) && window.used_percent.is_finite())
        .map(|window| window.used_percent.round().clamp(0.0, 100.0) as i64)
}

fn finite(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::{
        five_hour_used_percent,
        subscription::{UsageLimit, UsageWindow, window_display_label},
    };

    #[test]
    fn gauge_uses_the_same_five_hour_windows_as_subscription_rows() {
        let limits = |label: &str, window_seconds| {
            [UsageLimit {
                name: "usage".into(),
                limit_reached: false,
                windows: vec![UsageWindow {
                    label: label.into(),
                    window_seconds,
                    used_percent: 63.6,
                    reset_at: None,
                    used: None,
                    budget: None,
                }],
            }]
        };
        for (label, seconds, row, percent) in [
            ("5h", 0., "5-hour limit", Some(64)),
            ("", 18_000., "period", None),
            ("quota", 18_000., "5-hour limit", Some(64)),
            ("week", 18_000., "Weekly · all models", None),
            ("4h", 14_400., "4h", None),
        ] {
            let limits = limits(label, seconds);
            assert_eq!(window_display_label(&limits[0].windows[0]), row);
            assert_eq!(five_hour_used_percent(&limits), percent, "{row}");
        }
    }
}
