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
        .find(|window| {
            window.window_seconds == (5 * time::HOUR) as f64 && window.used_percent.is_finite()
        })
        .map(|window| window.used_percent.round().clamp(0.0, 100.0) as i64)
}

fn finite(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|v| v.is_finite())
}
