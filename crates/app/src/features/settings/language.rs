//! The Language section: the interface language, the app's own setting
//! (`shared::i18n`).

use gentle_core::i18n::Choice;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::SettingsView;
use super::layout::section;
use crate::shared::i18n::{self, t};
use crate::shared::ui;

fn label(choice: Choice) -> &'static str {
    match choice {
        Choice::System => t("System"),
        Choice::Fixed(language) => language.native_name(),
    }
}

impl SettingsView {
    pub(super) fn language_section(&self, cx: &mut Context<Self>) -> Div {
        let choice = i18n::choice(cx);
        let labels = Choice::ALL.map(label);
        let system_hint = (choice == Choice::System).then(|| {
            t("Follows the operating system's language: Spanish when it is set to Spanish, English otherwise.")
        });
        section(IconName::Globe, t("Language")).child(
            v_flex()
                .pt_1()
                .gap_1p5()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .w(rems(120. / 16.))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(ui::muted())
                                .child(t("Interface language")),
                        )
                        .child(ui::segmented(
                            "ui-language",
                            &labels,
                            Choice::ALL.iter().position(|c| *c == choice),
                            cx.listener(|_, index: &usize, _, cx| {
                                i18n::set_choice(Choice::ALL[*index], cx);
                                cx.notify();
                            }),
                        )),
                )
                .children(
                    system_hint.map(|hint| div().text_xs().text_color(ui::faint()).child(hint)),
                )
                .child(div().text_xs().text_color(ui::faint()).child(t(
                    "Only the app's interface changes; the agent keeps answering in the language you write to it. Saved in the app's settings.json (language).",
                ))),
        )
    }
}
