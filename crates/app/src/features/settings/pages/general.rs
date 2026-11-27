//! Environment details, Gentle Shell's own status and the existing
//! onboarding checklist.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use pi_rpc::desktop_rpc::ShellStatus;

use super::super::{
    SettingsView,
    layout::{row, section},
    shell_status::{status_rows, warns},
    snapshot::Snapshot,
};
use crate::shared::i18n::t;
use crate::shared::ui;

pub(super) fn render(
    view: &SettingsView,
    snapshot: &Snapshot,
    cx: &mut Context<SettingsView>,
) -> Div {
    v_flex()
        .gap_3()
        .child(gentle_shell_section(
            snapshot,
            view.shell_status.status.as_ref(),
        ))
        .child(view.runs_on_section(cx))
        .child(view.diagnostics_section(cx))
        .child(section(IconName::CircleCheck, t("Setup & requirements")).child(view.setup.clone()))
}

/// The homes and launcher, then, when the active agent tab's Shell offers
/// `get_status`, what `/gentle:status` reports and whether it warns.
fn gentle_shell_section(snapshot: &Snapshot, status: Option<&ShellStatus>) -> Div {
    section(IconName::SquareTerminal, "Gentle Shell")
        .child(row(
            t("Home"),
            if snapshot.home_mode.is_empty() {
                snapshot.agent.display().to_string()
            } else {
                snapshot.home_mode.clone()
            },
        ))
        .child(row(t("Agent dir"), snapshot.agent.display().to_string()))
        .child(row(t("Config dir"), snapshot.config.display().to_string()))
        .child(row(
            t("Launcher"),
            snapshot
                .launcher
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| t("not found").into()),
        ))
        .children(status.map(status_block))
        .child(
            h_flex()
                .pt_2()
                .gap_2()
                .flex_wrap()
                .child({
                    let dir = snapshot.agent.clone();
                    Button::new("open-agent-dir")
                        .ghost()
                        .small()
                        .icon(Icon::new(IconName::FolderOpen))
                        .label(t("Open agent dir"))
                        .on_click(move |_, _, cx| cx.open_url(&format!("file://{}", dir.display())))
                })
                .child({
                    let dir = snapshot.config.clone();
                    Button::new("open-config-dir")
                        .ghost()
                        .small()
                        .icon(Icon::new(IconName::FolderOpen))
                        .label(t("Open config dir"))
                        .on_click(move |_, _, cx| cx.open_url(&format!("file://{}", dir.display())))
                }),
        )
}

/// `/gentle:status`'s facts: a chip with its severity, then one row each.
/// A warning points at `/gentle:doctor`, which repairs from a session.
fn status_block(status: &ShellStatus) -> Div {
    let warning = warns(status);
    v_flex()
        .child(
            h_flex()
                .pt_3()
                .pb_1()
                .gap_2()
                .child(ui::section_label(t("Status")))
                .child(if warning {
                    ui::chip(t("Needs attention"), ui::warning())
                } else {
                    ui::chip("OK", ui::success())
                })
                .children(warning.then(|| {
                    div()
                        .text_xs()
                        .text_color(ui::muted())
                        .child(t("Run /gentle:doctor in a session to repair it."))
                })),
        )
        .children(
            status_rows(status)
                .into_iter()
                .map(|(label, value)| row(label, value)),
        )
}
