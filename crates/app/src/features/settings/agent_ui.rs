//! The Agent UI section: how readily the agent shows generative UI
//! (`gentle_ui`), the app's own setting (`genui::mode`).

use gentle_core::genui::mode::Mode;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::SettingsView;
use super::layout::section;
use crate::shared::i18n::t;
use crate::{features::genui, shared::ui};

fn hint(mode: Mode) -> &'static str {
    t(match mode {
        Mode::Proactive => {
            "The agent shows a diagram, table, form, checklist, chart or review on its own when one clearly beats a text answer: a system with several parts, a plan of four or more steps, options to compare, several questions at once, risky edits. Never for short answers, and at most one new UI per reply."
        }
        Mode::WhenAsked => {
            "Only when you ask for one (\"draw it\", \"as a table\", \"let me review the changes first\"), or to update one already shown."
        }
        Mode::Off => {
            "The agent has no gentle_ui tool. UIs already in a conversation stay and your edits still reach it, but it can't change them; Insert UI template still works."
        }
    })
}

impl SettingsView {
    pub(super) fn agent_ui_section(&self, cx: &mut Context<Self>) -> Div {
        let mode = genui::mode::mode(cx);
        let labels = Mode::ALL.map(|mode| t(mode.label()));
        section(IconName::Shapes, t("Agent UI")).child(
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
                                .child(t("Shows UIs")),
                        )
                        .child(ui::segmented(
                            "agent-ui-mode",
                            &labels,
                            Mode::ALL.iter().position(|m| *m == mode),
                            cx.listener(|_, index: &usize, _, cx| {
                                genui::mode::set_mode(Mode::ALL[*index], cx);
                                cx.notify();
                            }),
                        )),
                )
                .child(div().text_xs().text_color(ui::faint()).child(hint(mode)))
                .child(div().text_xs().text_color(ui::faint()).child(t(
                    "Every agent tab follows it from its next turn. Saved in the app's settings.json (genuiMode).",
                ))),
        )
    }
}
