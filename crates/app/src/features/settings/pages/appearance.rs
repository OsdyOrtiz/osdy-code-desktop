//! Desktop theme, window appearance and terminal appearance (read only).

use gpui_kit::assets::IconName;
use gpui_kit::component::v_flex;
use gpui_kit::*;

use super::super::{
    SettingsView,
    layout::{or_dash, row, section},
    snapshot::Snapshot,
};
use crate::shared::ui;

pub(super) fn render(
    view: &SettingsView,
    snapshot: &Snapshot,
    cx: &mut Context<SettingsView>,
) -> Div {
    v_flex()
        .gap_3()
        .child(view.language_section(cx))
        .child(view.interface_section(cx))
        .child(view.input_style_section(cx))
        .child(
            div()
                .id("theme-drop")
                .rounded_xl()
                .drag_over::<ExternalPaths>(|style, _, _, _| {
                    style.bg(ui::surface_hover())
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.theme_drop(paths, cx)
                }))
                .child(view.theme_section(cx)),
        )
        .child(
            section(IconName::SquareTerminal, "Terminal appearance")
                .child(row("Theme (terminal)", or_dash(&snapshot.theme, "default")))
                .child(row(
                    "Banner",
                    or_dash(&snapshot.banner, "pink · rose on · text on"),
                ))
                .child(
                    div().pt_2().text_xs().text_color(ui::faint()).child(
                        "These values reflect Gentle Shell's terminal configuration. Desktop themes and terminal themes are stored separately.",
                    ),
                ),
        )
}
