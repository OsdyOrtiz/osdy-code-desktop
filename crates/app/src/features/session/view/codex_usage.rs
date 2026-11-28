//! The all-account section of the owning tab's plan-limits popover.

use std::time::Instant;

use gentle_core::codex_accounts::{ALL_USAGE_BATCH_TIMEOUT, ALL_USAGE_DISPLAY_TIMEOUT, Feed};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::session::SessionView;
use crate::shared::{i18n::t, ui};

pub(super) fn accounts_section(
    feed: &Feed,
    loading: bool,
    live: bool,
    owner: WeakEntity<SessionView>,
) -> AnyElement {
    v_flex()
        .flex_shrink_0()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .child(ui::section_label(t("Accounts")))
                .child(div().flex_1())
                .when(loading, |row| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(ui::muted())
                            .child(t("Loading usage…")),
                    )
                })
                .child(
                    Button::new("codex-usage-refresh")
                        .ghost()
                        .xsmall()
                        .label(t("Refresh"))
                        .disabled(live)
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| this.refresh_codex_usage(true, cx));
                        }),
                ),
        )
        .when(feed.accounts.is_empty(), |list| {
            list.child(
                div()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(t("No saved Codex accounts.")),
            )
        })
        .children(feed.accounts.iter().map(|account| {
            v_flex()
                .gap_0p5()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .text_color(ui::fg())
                                .child(account.name.clone()),
                        )
                        .when(account.active, |row| {
                            row.child(ui::chip(t("Active"), ui::success()))
                        }),
                )
                .child(
                    div().text_xs().text_color(ui::muted()).child(
                        account
                            .quota
                            .clone()
                            .unwrap_or_else(|| t("Usage unavailable").to_owned()),
                    ),
                )
        }))
        .into_any_element()
}

impl SessionView {
    pub(super) fn refresh_codex_usage(&mut self, force: bool, cx: &mut Context<Self>) {
        if !self.has_codex_usage() {
            return;
        }
        let Some(feed) = self.codex_account_feed() else {
            return;
        };
        let Some(run) = self
            .extension
            .codex_all_usage
            .start(&feed, force, Instant::now())
        else {
            return;
        };
        for (timeout, display_only) in [
            (ALL_USAGE_DISPLAY_TIMEOUT, true),
            (ALL_USAGE_BATCH_TIMEOUT, false),
        ] {
            let run = run.clone();
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(timeout).await;
                let _ = this.update(cx, |this, cx| {
                    if this.extension.codex_all_usage.run_id() == Some(run.as_str()) {
                        if display_only {
                            this.hide_codex_usage_loading(cx);
                        } else {
                            this.observe_codex_all_usage(cx);
                        }
                    }
                });
            })
            .detach();
        }
        self.run_codex_all_usage_command(run, cx);
        cx.notify();
    }

    pub(in crate::features::session) fn hide_codex_usage_loading(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.extension.codex_all_usage.hide_loading() {
            cx.notify();
        }
    }

    pub(in crate::features::session) fn observe_codex_all_usage(&mut self, cx: &mut Context<Self>) {
        if !self.extension.codex_all_usage.live() {
            return;
        }
        let feed = self.codex_account_feed();
        if self
            .extension
            .codex_all_usage
            .observe(feed.as_ref(), Instant::now())
        {
            cx.notify();
        }
    }
}
