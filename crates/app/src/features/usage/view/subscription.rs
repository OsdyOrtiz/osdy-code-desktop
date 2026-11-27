//! Subscription tab: one card per signed-in provider, a meter per window.

use gentle_core::usage::subscription::{UsageLimit, format_reset_in};
use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use pi_rpc::desktop_rpc::UsageRefresh;

use super::status_line;
use crate::features::usage::credentials::{CardState, ProviderCard};
use crate::features::usage::{UsageMode, UsageState, UsageView, now_ms};
use crate::shared::{
    i18n::{language, t},
    ui,
};

/// Meter fill by how much of the window is spent. The 80/95 thresholds are
/// gentle-core's `limit_tone`, shared with the plan-limits chip and the
/// legacy limit chips.
fn bar_color(percent: f64) -> Hsla {
    use gentle_core::usage::subscription::LimitTone;
    match gentle_core::usage::subscription::limit_tone(percent) {
        LimitTone::Danger => ui::danger(),
        LimitTone::Warning => ui::warning(),
        // The calm band splits at 50: a half-spent window takes the accent.
        LimitTone::Calm if percent >= 50.0 => ui::accent(),
        LimitTone::Calm => ui::success(),
    }
}

/// The Usage screen's "updated Nm ago" line, shared with the session's
/// plan-limits chip.
pub(crate) fn updated_ago(fetched_at: i64, now: i64) -> String {
    let minutes = (now - fetched_at) / 60_000;
    if minutes < 1 {
        t("updated just now").into()
    } else {
        t("updated {minutes}m ago").replace("{minutes}", &minutes.to_string())
    }
}

impl UsageView {
    pub(super) fn render_usage(&self) -> AnyElement {
        let (cards, others, empty) = match &self.usage {
            UsageState::Loading => {
                return h_flex()
                    .gap_2()
                    .text_sm()
                    .text_color(ui::muted())
                    .child(Spinner::new().small().color(ui::muted()))
                    .child(t("Reading subscription usage…"))
                    .into_any_element();
            }
            UsageState::Failed(error) => {
                return ui::card()
                    .p_4()
                    .gap_2()
                    .border_color(ui::danger().opacity(0.35))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Icon::new(IconName::CircleAlert)
                                    .small()
                                    .text_color(ui::danger()),
                            )
                            .child(ui::section_label(t("Could not read subscription usage"))),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(ui::danger())
                            .child(error.render()),
                    )
                    .into_any_element();
            }
            UsageState::Loaded {
                cards,
                others,
                empty,
            } => (cards, others, empty),
        };
        let now = now_ms();
        v_flex()
            .gap_3()
            .child(ui::section_label(t("Subscriptions")))
            .when(cards.is_empty(), |d| {
                d.child(
                    ui::card().p_4().child(
                        div()
                            .text_sm()
                            .text_color(ui::muted())
                            .child(t(empty.message())),
                    ),
                )
            })
            .children(
                cards
                    .iter()
                    .enumerate()
                    .map(|(i, card)| ui::appear(("usage-card", i), self.render_card(card, now))),
            )
            .when(!others.is_empty(), |d| {
                d.child(
                    div().text_xs().text_color(ui::faint()).child(
                        t("No subscription usage for: {providers}")
                            .replace("{providers}", &others.join(", ")),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_card(&self, card: &ProviderCard, now: i64) -> Div {
        let (plan, fetched_at) = match &card.state {
            CardState::Ready(usage) => (usage.plan.clone(), Some(usage.fetched_at)),
            CardState::Pending(Some(usage)) | CardState::Stale(Some(usage)) => {
                (usage.plan.clone(), None)
            }
            _ => (None, None),
        };
        let status = match &card.state {
            CardState::Loading => Some(ui::chip(t("fetching"), ui::info())),
            CardState::SignIn(_) => Some(ui::chip(t("signed out"), ui::warning())),
            CardState::Failed(_) => Some(ui::chip(t("error"), ui::danger())),
            CardState::Pending(_) => Some(ui::chip(t("pending"), ui::info())),
            CardState::Stale(_) => Some(ui::chip(t("stale"), ui::warning())),
            CardState::Unavailable => Some(ui::chip(t("unavailable"), ui::muted())),
            CardState::Note(_) | CardState::Ready(_) => None,
        };
        let body: AnyElement = match &card.state {
            CardState::Loading | CardState::Pending(None) => h_flex()
                .gap_2()
                .text_xs()
                .text_color(ui::muted())
                .child(Spinner::new().xsmall().color(ui::muted()))
                .child(t("Fetching usage…"))
                .into_any_element(),
            CardState::Note(note) => {
                status_line(ui::muted(), None, note.render()).into_any_element()
            }
            CardState::SignIn(note) => {
                status_line(ui::warning(), Some(IconName::LogIn), note.render()).into_any_element()
            }
            CardState::Failed(error) => {
                status_line(ui::danger(), Some(IconName::CircleAlert), error.render())
                    .into_any_element()
            }
            CardState::Unavailable => status_line(
                ui::muted(),
                None,
                t("No subscription usage is available for this provider."),
            )
            .into_any_element(),
            CardState::Stale(None) => {
                status_line(ui::muted(), None, t("No subscription usage reported yet."))
                    .into_any_element()
            }
            CardState::Ready(usage)
            | CardState::Pending(Some(usage))
            | CardState::Stale(Some(usage)) => v_flex()
                .gap_3()
                .children(
                    usage
                        .limits
                        .iter()
                        .map(|limit| self.render_limit(limit, now)),
                )
                .into_any_element(),
        };
        let title = if self.mode == UsageMode::Rpc && card.id == "claude-bridge" {
            t("Claude Code · bridge").into()
        } else {
            card.title.clone()
        };
        ui::card()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .gap_2()
                    .min_w_0()
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ui::champagne())
                            .child(title),
                    )
                    .when_some(plan, |d, plan| {
                        d.child(ui::chip(plan, ui::accent()).flex_shrink_0())
                    })
                    .children(status)
                    .when_some(card.refresh, |d, refresh| {
                        d.child(Self::refresh_chip(refresh).flex_shrink_0())
                    })
                    .child(div().flex_1())
                    .when_some(fetched_at, |d, at| {
                        d.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(ui::faint())
                                .child(updated_ago(at, now)),
                        )
                    }),
            )
            .child(body)
    }

    /// Whether the server can refresh this provider's usage on request.
    fn refresh_chip(refresh: UsageRefresh) -> Div {
        match refresh {
            UsageRefresh::Unsupported => ui::chip(t("refresh unsupported"), ui::muted()),
            UsageRefresh::OutOfScope => ui::chip(t("refresh out of scope"), ui::muted()),
        }
    }

    fn render_limit(&self, limit: &UsageLimit, now: i64) -> AnyElement {
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .min_w_0()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ui::muted())
                            .child(if self.mode == UsageMode::Legacy && limit.name == "limit" {
                                t("limit").into()
                            } else {
                                limit.name.clone()
                            }),
                    )
                    .when(limit.limit_reached, |d| {
                        d.child(ui::chip(t("limit reached"), ui::danger()).flex_shrink_0())
                    }),
            )
            .children(limit.windows.iter().map(|window| {
                let percent = window.used_percent.max(0.0);
                let color = bar_color(percent);
                let label = if window.label.is_empty() {
                    t("period").to_string()
                } else if window.label == "week" {
                    t("week").to_string()
                } else {
                    window.label.clone()
                };
                h_flex()
                    .gap_3()
                    .min_w_0()
                    .child(
                        div()
                            .w(rems(52. / 16.))
                            .flex_shrink_0()
                            .truncate()
                            .text_xs()
                            .text_color(ui::muted())
                            .child(label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(rems(60. / 16.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(ui::well())
                            .border_1()
                            .border_color(ui::border())
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative((percent / 100.0).min(1.0) as f32))
                                    .rounded_full()
                                    .bg(color),
                            ),
                    )
                    .child(
                        div()
                            .w(rems(44. / 16.))
                            .flex_shrink_0()
                            .text_size(ui::code_rems(12.))
                            .font_family(ui::mono())
                            .text_color(color)
                            .text_right()
                            .child(format!("{}%", percent.round() as i64)),
                    )
                    .child(
                        div()
                            .w(rems(128. / 16.))
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(ui::faint())
                            .child(format_reset_in(language(), window.reset_at, now)),
                    )
            }))
            .into_any_element()
    }
}
