//! The composer's plan-limits chip: the current model's subscription
//! windows ("Plan usage limits · plus", one row per window with its reset
//! clock), just after the effort picker, sharing the Usage screen's data.

use crate::shared::i18n::t;
use std::collections::BTreeMap;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::{Popover, PopoverState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use gentle_core::usage::subscription::{
    LimitTone, limit_tone, provider_alias, reset_clock_label_in, window_display_label_in,
    windows_by_length, windows_most_used,
};
use pi_rpc::desktop_rpc::{ProviderUsage as RpcProviderUsage, UsageStatus};

use super::activity::{ChipHeaderAction, ChipPopoverOptions, chip_popover};
use super::live::RateLimit;
use crate::features::session::SessionView;
use crate::features::usage::{local_offset_secs, now_ms, rpc_provider_to_core, updated_ago};
use crate::shared::ui;

/// An older Shell keeps its windows out of Desktop RPC: the popover says so
/// instead of inventing rows.
const LEGACY_LINE: &str = "Plan limits need Gentle Shell 4.8.0 or newer";
/// How often a hidden plan-limits chip asks for the usage again.
const PLAN_SEED_EVERY: std::time::Duration = std::time::Duration::from_secs(30);
/// A Shell that offers the methods but has not reported windows yet.
const NO_DATA_LINE: &str = "No plan usage reported yet.";

/// One window row of the plan-limits card.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlanRow {
    /// "5-hour limit", "Weekly · all models", "period" (NaN's billing-period
    /// allowance).
    pub(super) label: String,
    /// "resets at 5:30 p.m." today, "resets Sun 6:00 a.m." later, "" unknown.
    pub(super) reset: String,
    pub(super) percent: i64,
}

/// What the plan-limits chip shows, as plain values so the decision is
/// testable without a session.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlanChip {
    /// The most-used window's percent: the trigger's label and tone.
    pub(super) percent: i64,
    /// The provider's plan, when it reports one ("Plan usage limits · plus").
    pub(super) plan: Option<String>,
    /// One row per window, shortest first. Empty when only the legacy
    /// notices exist.
    pub(super) rows: Vec<PlanRow>,
    /// Freshness of the structured data ("updated 3m ago", "stale", …).
    pub(super) status_line: Option<String>,
    /// When there are no structured windows, the one line the popover shows.
    pub(super) fallback_line: Option<&'static str>,
}

/// What the composer's plan-limits chip shows for this session: the
/// subscription provider matching the current model's provider (by alias),
/// the trigger percent from the most-used window and the card rows ordered
/// shortest first — or, when there are none, the legacy notices' highest
/// percent over an honest fallback line. `None` hides the chip: nothing
/// reports plan usage at all, or the legacy limit chips (visible on the
/// activity line) are what would feed it, so no percent is duplicated.
pub(super) fn plan_chip(
    providers: &[RpcProviderUsage],
    legacy_snapshot: bool,
    legacy_chips_shown: bool,
    rate_limits: &BTreeMap<String, RateLimit>,
    current_provider: Option<&str>,
    offset_secs: i64,
    now: i64,
) -> Option<PlanChip> {
    let alias = current_provider.and_then(provider_alias);
    let matched = alias.and_then(|alias| {
        providers
            .iter()
            .find(|provider| provider_alias(&provider.provider) == Some(alias))
    });
    let windows = matched.map(|provider| {
        let core = rpc_provider_to_core(provider, now);
        let most_used = windows_most_used(&core.limits);
        (provider, core, most_used)
    });
    if let Some((provider, core, most_used)) = windows.filter(|(_, _, windows)| !windows.is_empty())
    {
        let percent = |value: f64| value.round().clamp(0.0, 100.0) as i64;
        return Some(PlanChip {
            percent: percent(most_used[0].used_percent),
            plan: core.plan.clone(),
            status_line: Some(match provider.status {
                UsageStatus::Ok => updated_ago(core.fetched_at, now),
                UsageStatus::Pending => t("updating…").into(),
                UsageStatus::Stale => t("stale").into(),
                UsageStatus::Failed => t("failed").into(),
                UsageStatus::Unavailable => t("unavailable").into(),
            }),
            rows: windows_by_length(&core.limits)
                .iter()
                .map(|window| PlanRow {
                    label: window_display_label_in(crate::shared::i18n::language(), window),
                    reset: reset_clock_label_in(
                        crate::shared::i18n::language(),
                        window.reset_at,
                        now,
                        offset_secs,
                    ),
                    percent: percent(window.used_percent),
                })
                .collect(),
            fallback_line: None,
        });
    }
    // No structured windows: the legacy notices decide, but only when the
    // activity line is not already showing them as limit chips (one number,
    // never the same percent twice).
    if legacy_chips_shown {
        return None;
    }
    let percent = rate_limits.values().map(|notice| notice.percent).max()?;
    Some(PlanChip {
        percent: i64::from(percent),
        plan: None,
        rows: Vec::new(),
        status_line: None,
        fallback_line: Some(if legacy_snapshot {
            LEGACY_LINE
        } else {
            NO_DATA_LINE
        }),
    })
}

/// A row's meter fill and percent color, from the shared 80/95 thresholds.
fn row_color(percent: i64) -> Hsla {
    match limit_tone(percent as f64) {
        LimitTone::Danger => ui::danger(),
        LimitTone::Warning => ui::warning(),
        LimitTone::Calm => ui::muted(),
    }
}

impl SessionView {
    /// The plan-limits chip, inside the composer's box just after the effort
    /// picker: the current model provider's plan usage, toned by the shared
    /// 80/95 thresholds. While it has nothing to show and the session has a
    /// model, it asks for the subscription usage itself — outside the render
    /// pass, at most once per [`PLAN_SEED_EVERY`] (seeding is idempotent,
    /// like the Usage screen's).
    pub(super) fn render_plan_limits_chip(
        &self,
        legacy_chips_shown: bool,
        cx: &mut Context<Self>,
    ) -> Option<Popover> {
        let snapshot = self.subscription_snapshot();
        let current = self.models.current.as_ref().map(|m| m.provider.clone());
        let chip = plan_chip(
            &snapshot.providers,
            snapshot.legacy,
            legacy_chips_shown,
            &self.extension.rate_limits,
            current.as_deref(),
            *self.local_offset.get_or_init(local_offset_secs),
            now_ms(),
        );
        let Some(chip) = chip else {
            // Hidden without data, so its card can't open to ask for it: once
            // the session has a model, ask here (seeding is idempotent and a
            // no-op on Shells without the method), throttled.
            if current.is_some() {
                self.ask_plan_usage(cx);
            }
            return None;
        };
        let weak = cx.entity().downgrade();
        let color = row_color(chip.percent);
        let title: SharedString = match &chip.plan {
            Some(plan) => t("Plan usage limits · {plan}")
                .replace("{plan}", plan)
                .into(),
            None => t("Plan usage limits").into(),
        };
        let rows = chip.rows;
        let fallback = chip.fallback_line.map(t);
        let status_line = chip.status_line;
        let body = move || {
            let mut items: Vec<AnyElement> = rows
                .iter()
                .map(|row| {
                    let color = row_color(row.percent);
                    v_flex()
                        .gap_1()
                        .child(
                            h_flex()
                                .gap_2()
                                .min_w_0()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .text_color(ui::muted())
                                        .child(row.label.clone()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .text_color(ui::faint())
                                        .child(row.reset.clone()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(ui::code_rems(12.))
                                        .font_family(ui::mono())
                                        .text_color(color)
                                        .child(format!("{}%", row.percent)),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(4.))
                                .rounded_full()
                                .bg(ui::well())
                                .border_1()
                                .border_color(ui::border())
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative((row.percent as f32 / 100.0).min(1.0)))
                                        .rounded_full()
                                        .bg(color),
                                ),
                        )
                        .into_any_element()
                })
                .collect();
            if let Some(line) = fallback {
                items.push(
                    div()
                        .text_xs()
                        .text_color(ui::muted())
                        .child(line)
                        .into_any_element(),
                );
            }
            if let Some(line) = status_line.clone() {
                items.push(
                    div()
                        .text_xs()
                        .text_color(ui::muted())
                        .child(line)
                        .into_any_element(),
                );
            }
            items
        };
        let weak_open = weak.clone();
        let weak_see_all = weak.clone();
        let see_all: ChipHeaderAction = Box::new(
            move |_window: &mut Window, cx: &mut Context<PopoverState>| {
                // Header, right side: "See all →" opens Usage & stats and
                // closes the card (text only, no icon).
                let weak = weak_see_all.clone();
                Button::new("plan-see-all")
                    .ghost()
                    .xsmall()
                    .label(t("See all →"))
                    .on_click(cx.listener(move |state, _, window, cx| {
                        state.dismiss(window, cx);
                        let _ = weak.update(cx, |_, cx| {
                            cx.emit(crate::features::session::SessionEvent::OpenScreen(
                                gentle_core::commands::Screen::Usage,
                            ))
                        });
                    }))
                    .into_any_element()
            },
        );
        Some(
            chip_popover(
                ChipPopoverOptions::new(
                    "plan-limits",
                    IconName::Gauge,
                    color,
                    format!("{}%", chip.percent),
                )
                .title(title)
                .action(see_all),
                body,
            )
            .on_open_change(move |open, _, cx| {
                if *open {
                    let _ = weak_open.update(cx, |this, cx| this.subscription_seed(cx));
                }
            }),
        )
    }

    /// Ask for the subscription usage outside the render pass, at most once
    /// per [`PLAN_SEED_EVERY`] while the chip has nothing to show.
    fn ask_plan_usage(&self, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        if self
            .plan_seed_asked
            .get()
            .is_some_and(|last| now.duration_since(last) < PLAN_SEED_EVERY)
        {
            return;
        }
        self.plan_seed_asked.set(Some(now));
        let weak = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = weak.update(cx, |this, cx| this.subscription_seed(cx));
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pi_rpc::desktop_rpc::UsageWindow as RpcUsageWindow;
    use pi_rpc::desktop_rpc::{UsageLimit as RpcUsageLimit, UsageResetAt, UsageStatus};

    use super::{RpcProviderUsage, plan_chip};
    use crate::features::session::view::live::RateLimit;

    const NOW: i64 = 1_762_400_000_000;

    fn window(label: &str, percent: f64, reset_at: UsageResetAt) -> RpcUsageWindow {
        window_with(label, percent, 18_000.0, reset_at)
    }

    fn window_with(
        label: &str,
        percent: f64,
        window_seconds: f64,
        reset_at: UsageResetAt,
    ) -> RpcUsageWindow {
        RpcUsageWindow {
            label: label.into(),
            used_percent: percent,
            window_seconds,
            reset_at,
            used: None,
            budget: None,
        }
    }

    fn limit(name: &str, windows: Vec<RpcUsageWindow>) -> RpcUsageLimit {
        RpcUsageLimit {
            name: name.into(),
            limit_reached: false,
            scope: None,
            windows,
        }
    }

    fn provider(
        id: &str,
        status: UsageStatus,
        plan: Option<&str>,
        fetched_at: Option<f64>,
        limits: Vec<RpcUsageLimit>,
    ) -> RpcProviderUsage {
        RpcProviderUsage {
            provider: id.into(),
            status,
            fetched_at,
            plan: plan.map(str::to_string),
            refresh: None,
            limits,
        }
    }

    fn notices(windows: &[(&str, u32)]) -> BTreeMap<String, RateLimit> {
        windows
            .iter()
            .map(|(window, percent)| {
                (
                    (*window).to_string(),
                    RateLimit {
                        percent: *percent,
                        text: format!("{percent}% used ({window})"),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn the_current_model_provider_drives_the_chip() {
        // Claude windows, the model provider named `claude`.
        let providers = vec![provider(
            "claude",
            UsageStatus::Ok,
            Some("pro"),
            Some((NOW - 120_000) as f64),
            vec![limit(
                "prompts",
                vec![
                    window("5h", 42.0, UsageResetAt::Null),
                    window("week", 10.0, UsageResetAt::At(1_762_668_000_000.0)),
                ],
            )],
        )];
        let chip = plan_chip(
            &providers,
            false,
            false,
            &BTreeMap::new(),
            Some("claude"),
            0,
            NOW,
        )
        .expect("chip shown");
        assert_eq!(chip.percent, 42, "the trigger shows the most-used window");
        assert_eq!(chip.plan.as_deref(), Some("pro"));
        assert_eq!(chip.rows.len(), 2, "most used first");
        assert_eq!(chip.rows[0].label, "5-hour limit");
        assert_eq!(chip.rows[0].reset, "");
        assert_eq!(chip.rows[1].label, "Weekly · all models");
        assert_eq!(chip.rows[1].percent, 10);
        assert_eq!(
            chip.rows[1].reset, "resets Sun 6:00 a.m.",
            "1_762_668_000_000 is Sunday 06:00 UTC"
        );
        assert_eq!(chip.status_line.as_deref(), Some("updated 2m ago"));
        assert_eq!(chip.fallback_line, None);
    }

    #[test]
    fn aliases_match_across_provider_names() {
        let providers = vec![provider(
            "codex",
            UsageStatus::Ok,
            None,
            None,
            vec![limit("codex", vec![window("5h", 12.0, UsageResetAt::Null)])],
        )];
        for model_provider in ["codex", "openai-codex"] {
            let chip = plan_chip(
                &providers,
                false,
                false,
                &BTreeMap::new(),
                Some(model_provider),
                0,
                NOW,
            );
            assert!(chip.is_some(), "{model_provider} matches the codex account");
        }
        // `anthropic` matches a provider the wire names `claude`.
        let claude = vec![provider(
            "claude",
            UsageStatus::Ok,
            None,
            None,
            vec![limit(
                "prompts",
                vec![window("week", 5.0, UsageResetAt::Null)],
            )],
        )];
        assert!(
            plan_chip(
                &claude,
                false,
                false,
                &BTreeMap::new(),
                Some("anthropic"),
                0,
                NOW
            )
            .is_some()
        );
        // A provider the model doesn't belong to never drives the chip.
        assert!(
            plan_chip(
                &providers,
                false,
                false,
                &BTreeMap::new(),
                Some("nan"),
                0,
                NOW
            )
            .is_none()
        );
        assert!(plan_chip(&providers, false, false, &BTreeMap::new(), None, 0, NOW).is_none());
        // An unknown model provider has no subscription windows to match.
        assert!(
            plan_chip(
                &providers,
                false,
                false,
                &BTreeMap::new(),
                Some("deepseek"),
                0,
                NOW
            )
            .is_none()
        );
    }

    #[test]
    fn statuses_and_stale_data_are_shown_honestly() {
        let windows = vec![limit(
            "prompts",
            vec![window("5h", 42.0, UsageResetAt::Null)],
        )];
        let stale = provider("claude", UsageStatus::Stale, None, None, windows.clone());
        let chip = plan_chip(
            &[stale],
            false,
            false,
            &BTreeMap::new(),
            Some("claude"),
            0,
            NOW,
        )
        .unwrap();
        assert_eq!(chip.status_line.as_deref(), Some("stale"));
        assert_eq!(chip.rows.len(), 1, "last good windows still show");

        let pending = provider("claude", UsageStatus::Pending, None, None, windows.clone());
        let chip = plan_chip(
            &[pending],
            false,
            false,
            &BTreeMap::new(),
            Some("claude"),
            0,
            NOW,
        )
        .unwrap();
        assert_eq!(chip.status_line.as_deref(), Some("updating…"));

        let failed = provider("claude", UsageStatus::Failed, None, None, windows.clone());
        let chip = plan_chip(
            &[failed],
            false,
            false,
            &BTreeMap::new(),
            Some("claude"),
            0,
            NOW,
        )
        .unwrap();
        assert_eq!(chip.status_line.as_deref(), Some("failed"));

        // No windows reported at all: no chip, the notices decide.
        let empty = provider("claude", UsageStatus::Ok, None, None, Vec::new());
        assert!(
            plan_chip(
                &[empty],
                false,
                false,
                &BTreeMap::new(),
                Some("claude"),
                0,
                NOW
            )
            .is_none()
        );
    }

    #[test]
    fn rows_read_shortest_first_while_the_trigger_keeps_the_most_used() {
        // The weekly window is the one about to bite, the 5-hour one, not.
        let providers = vec![provider(
            "claude",
            UsageStatus::Ok,
            None,
            None,
            vec![limit(
                "prompts",
                vec![
                    window_with("week", 90.0, 604_800.0, UsageResetAt::Null),
                    window_with("5h", 42.0, 18_000.0, UsageResetAt::Null),
                ],
            )],
        )];
        let chip = plan_chip(
            &providers,
            false,
            false,
            &BTreeMap::new(),
            Some("claude"),
            0,
            NOW,
        )
        .expect("chip shown");
        assert_eq!(
            chip.percent, 90,
            "the trigger still shows the most-used window"
        );
        assert_eq!(
            chip.rows[0].label, "5-hour limit",
            "the 5-hour row comes first"
        );
        assert_eq!(chip.rows[0].percent, 42);
        assert_eq!(chip.rows[1].label, "Weekly · all models");
        assert_eq!(chip.rows[1].percent, 90);
    }

    #[test]
    fn legacy_notices_keep_the_chip_with_one_honest_line() {
        let notices = notices(&[("seven_day", 57), ("five_hour", 33)]);
        // An older Shell: the popover says what it needs instead of rows.
        let chip = plan_chip(&[], true, false, &notices, Some("claude"), 0, NOW).unwrap();
        assert_eq!(chip.percent, 57, "the highest notice drives the label");
        assert!(chip.rows.is_empty(), "notices never become popover rows");
        assert_eq!(chip.status_line, None);
        assert_eq!(
            chip.fallback_line,
            Some("Plan limits need Gentle Shell 4.8.0 or newer")
        );

        // A Shell that offers the methods but has not reported windows yet.
        let chip = plan_chip(&[], false, false, &notices, Some("claude"), 0, NOW).unwrap();
        assert_eq!(chip.fallback_line, Some("No plan usage reported yet."));

        // Nothing reports plan usage at all: hidden.
        assert!(plan_chip(&[], false, false, &BTreeMap::new(), Some("claude"), 0, NOW).is_none());
    }

    #[test]
    fn the_legacy_limit_chips_hide_the_chip() {
        // The activity line already shows "7d 57%" and "5h 33%": the chip
        // would duplicate the only number a legacy snapshot has, so it hides.
        let notices = notices(&[("seven_day", 57), ("five_hour", 33)]);
        assert!(plan_chip(&[], true, true, &notices, Some("claude"), 0, NOW).is_none());

        // With structured windows the chip stands on its own data, whatever
        // the legacy chips show.
        let providers = vec![provider(
            "claude",
            UsageStatus::Ok,
            None,
            None,
            vec![limit(
                "prompts",
                vec![window("5h", 42.0, UsageResetAt::Null)],
            )],
        )];
        let chip = plan_chip(&providers, false, true, &notices, Some("claude"), 0, NOW)
            .expect("structured windows keep the chip");
        assert_eq!(chip.percent, 42);
        assert_eq!(chip.fallback_line, None);
    }
}
