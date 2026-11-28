//! MCP servers and optional companion tools, with a link to device settings.

use gentle_core::mcp_status::{self, ErrorKind, RowStatus};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::{SettingsView, layout::section, navigation::SettingsPage, snapshot::Snapshot};
use crate::shared::i18n::t;
use crate::shared::ui;

pub(super) fn render(
    view: &SettingsView,
    snapshot: &Snapshot,
    cx: &mut Context<SettingsView>,
) -> Div {
    let feed = view
        .session
        .as_ref()
        .and_then(|session| session.read(cx).mcp_status_feed());
    let rows = mcp_status::rows(&snapshot.mcp_servers, feed.as_ref());

    v_flex()
        .gap_3()
        .child(
            section(IconName::Bot, t("MCP servers"))
                .when(rows.is_empty(), |card| {
                    card.child(
                        div()
                            .text_xs()
                            .text_color(ui::muted())
                            .child(t("Not available")),
                    )
                })
                .children(rows.into_iter().map(|(name, status)| {
                    h_flex()
                        .gap_3()
                        .py_1()
                        .items_start()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(ui::fg())
                                .child(SharedString::from(name.to_owned())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(ui::muted())
                                .child(status_label(status)),
                        )
                }))
                .child(div().pt_2().text_xs().text_color(ui::faint()).child(t(
                    "Status comes from the active agent. This view never starts connections.",
                ))),
        )
        .child(view.codegraph_section(cx))
        .child(view.maestro_section(cx))
        .child(
            h_flex().child(
                Button::new("integrations-device-settings")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::Smartphone))
                    .label(t("Configure devices"))
                    .on_click(
                        cx.listener(|this, _, _, cx| this.navigate_to(SettingsPage::Devices, cx)),
                    ),
            ),
        )
        .child(view.music_section(cx))
        .child(view.tower_section(cx))
}

fn status_label(status: RowStatus) -> SharedString {
    match status {
        RowStatus::Ready => t("Connected"),
        RowStatus::Connecting => t("Connecting"),
        RowStatus::Disabled => t("Disabled"),
        RowStatus::Unavailable => t("Not available"),
        RowStatus::Error(None) => t("Error"),
        RowStatus::Error(Some(kind)) => {
            let kind = match kind {
                ErrorKind::Auth => t("Authentication"),
                ErrorKind::Timeout => t("Timeout"),
                ErrorKind::Transport => t("Transport"),
                ErrorKind::Config => t("Configuration"),
                ErrorKind::Unknown => t("Unknown"),
            };
            return t("Error · {kind}").replace("{kind}", kind).into();
        }
    }
    .into()
}
