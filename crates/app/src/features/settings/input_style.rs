//! Input style for Configuration → Appearance: how every agent tab's
//! composer looks. The choice lives in `session::input_style`.

use gentle_core::input_style::InputStyle;
use gpui_kit::assets::IconName;
use gpui_kit::component::h_flex;
use gpui_kit::*;

use super::SettingsView;
use super::layout::section;
use crate::features::session::input_style;
use crate::shared::i18n::t;
use crate::shared::ui;

impl SettingsView {
    pub(super) fn input_style_section(&self, cx: &mut Context<Self>) -> Div {
        let current = input_style::style(cx);
        let labels = InputStyle::ALL.map(|style| t(style.label()));
        section(IconName::MessageSquare, t("Composer"))
            .child(
                h_flex()
                    .pt_2()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .w(rems(120. / 16.))
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(ui::muted())
                            .child(t("Input style")),
                    )
                    .child(ui::segmented(
                        "input-style",
                        &labels,
                        InputStyle::ALL.iter().position(|s| *s == current),
                        cx.listener(|_, index: &usize, _, cx| {
                            input_style::set_style(InputStyle::ALL[*index], cx);
                            cx.notify();
                        }),
                    )),
            )
            .child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(ui::faint())
                    .child(t(current.summary())),
            )
    }
}
