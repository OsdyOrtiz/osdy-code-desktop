//! Gentle Tower: whether the Gentle Shell the app runs has Tower's web app,
//! and building it when its release shipped without it (`setup::tower`).

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::SettingsView;
use super::layout::{row, section};
use crate::features::setup::tower::{self, TowerStatus};
use crate::shared::i18n::t;
use crate::shared::ui;

/// Build output lines kept on screen.
const LOG_KEEP: usize = 8;

#[derive(Default)]
pub(super) struct Tower {
    status: Option<TowerStatus>,
    busy: bool,
    log: Vec<String>,
    note: Option<(String, bool)>,
    task: Option<Task<()>>,
}

impl SettingsView {
    pub(super) fn refresh_tower(&mut self, cx: &mut Context<Self>) {
        self.tower.task = Some(cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async { tower::status() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.tower.status = Some(status);
                cx.notify();
            });
        }));
    }

    fn enable_tower(&mut self, cx: &mut Context<Self>) {
        if self.tower.busy {
            return;
        }
        self.tower.busy = true;
        self.tower.log.clear();
        self.tower.note = None;
        let (tx, rx) = async_channel::unbounded::<String>();
        self.tower.task = Some(cx.spawn(async move |this, cx| {
            let done = cx.background_executor().spawn(async move {
                tower::install(&move |line: String| {
                    let _ = tx.send_blocking(line);
                })
            });
            while let Ok(line) = rx.recv().await {
                let alive = this.update(cx, |this, cx| {
                    // Download progress lines replace each other.
                    if line.trim_start().ends_with(" MB")
                        && this.tower.log.last().is_some_and(|l| l.ends_with(" MB"))
                    {
                        this.tower.log.pop();
                    }
                    this.tower.log.push(line);
                    let extra = this.tower.log.len().saturating_sub(LOG_KEEP);
                    this.tower.log.drain(..extra);
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            let result = done.await;
            let status = cx
                .background_executor()
                .spawn(async { tower::status() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.tower.busy = false;
                this.tower.status = Some(status);
                this.tower.note = Some(match result {
                    Ok(()) => (
                        t("Gentle Tower is ready: open it from the tab bar's + or with /gentle:tower \
                         (a Tower tab showing the error: Try again).")
                            .into(),
                        true,
                    ),
                    Err(error) => (error, false),
                });
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn tower_section(&self, cx: &mut Context<Self>) -> Div {
        let section = section(IconName::Building2, "Gentle Tower").child(
            div().pb_2().text_xs().text_color(ui::muted()).child(t(
                "Every open session as a floor of its project's building: tasks, talk, \
                 artifacts, cost and permissions. Gentle Shell 4.9.0 was published without \
                 Tower's web app; Enable downloads that version's source from GitHub and builds \
                 it into your install (needs a few minutes and an internet connection).",
            )),
        );
        let Some(status) = &self.tower.status else {
            return section.child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(Spinner::new().xsmall().color(ui::accent()))
                    .child(t("Looking for Gentle Shell…")),
            );
        };
        let busy = self.tower.busy;
        let (state, color) = match (&status.package, status.built) {
            (None, _) => (t("Gentle Shell not found"), ui::faint()),
            (Some(_), true) => (t("Ready"), ui::success()),
            (Some(_), false) => (t("Web app missing in this Gentle Shell"), ui::warning()),
        };
        section
            .child(h_flex().pb_2().child(ui::chip(state, color)))
            .child(row(
                "Gentle Shell",
                status.version.clone().unwrap_or_else(|| "—".into()),
            ))
            .when_some(status.package.as_ref(), |d, package| {
                d.child(row(t("Package"), package.display().to_string()))
            })
            .child(
                h_flex()
                    .pt_3()
                    .gap_2()
                    .flex_wrap()
                    .when(status.package.is_some() && !status.built, |d| {
                        d.child(
                            Button::new("tower-enable")
                                .small()
                                .primary()
                                .icon(Icon::new(IconName::ArrowDown))
                                .label(t("Enable Gentle Tower"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.enable_tower(cx))),
                        )
                    })
                    .child(
                        Button::new("tower-refresh")
                            .small()
                            .ghost()
                            .icon(Icon::new(IconName::RefreshCw))
                            .label(t("Check again"))
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_tower(cx))),
                    ),
            )
            .when(busy, |d| {
                d.child(
                    h_flex()
                        .pt_2()
                        .gap_2()
                        .text_xs()
                        .text_color(ui::muted())
                        .child(Spinner::new().xsmall().color(ui::accent()))
                        .child(t("Building Gentle Tower…")),
                )
            })
            .when(!self.tower.log.is_empty(), |d| {
                d.child(
                    ui::well_box().mt_2().px_3().py_2().gap_0p5().children(
                        self.tower
                            .log
                            .iter()
                            .map(|line| div().text_color(ui::muted()).child(line.clone())),
                    ),
                )
            })
            .when_some(self.tower.note.clone(), |d, (text, ok)| {
                d.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(if ok { ui::success() } else { ui::danger() })
                        .child(text),
                )
            })
    }
}
