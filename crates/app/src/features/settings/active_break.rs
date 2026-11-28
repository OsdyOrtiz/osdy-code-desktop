//! The Active break reminder section: on or off, how often, and a preview.

use gentle_core::hydration::{INTERVALS, Reminder};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::SettingsView;
use super::layout::{control, section};
use crate::shared::i18n::t;
use crate::{
    features::{active_break, hydration},
    shared::ui,
};

impl SettingsView {
    pub(super) fn active_break_section(&self, cx: &mut Context<Self>) -> Div {
        let reminder = active_break::reminder(cx);
        let set = |change: fn(Reminder, usize) -> Reminder| {
            move |_: &mut Self, index: &usize, _: &mut Window, cx: &mut Context<Self>| {
                active_break::set(change(active_break::reminder(cx), *index), cx);
                cx.notify();
            }
        };
        section(IconName::Activity, t("Active break"))
            .child(control(
                t("Reminder"),
                false,
                ui::segmented(
                    "active-break-enabled",
                    &[t("Off"), t("On")],
                    Some(usize::from(reminder.enabled)),
                    cx.listener(set(|r, index| Reminder {
                        enabled: index == 1,
                        ..r
                    })),
                ),
            ))
            .child(control(
                t("Every"),
                !reminder.enabled,
                ui::segmented(
                    "active-break-every",
                    &hydration::LABELS.map(t),
                    INTERVALS.iter().position(|m| *m == reminder.every_minutes),
                    cx.listener(set(|r, index| Reminder {
                        every_minutes: INTERVALS[index],
                        ..r
                    })),
                ),
            ))
            .child(
                v_flex()
                    .pt_2()
                    .gap_2()
                    .child(div().text_xs().text_color(ui::faint()).child(
                        t("A pixel figure gets up and stretches. The clock starts when the app opens; changing how often keeps counting from then."),
                    ))
                    .child(
                        h_flex().child(
                            Button::new("active-break-preview")
                                .ghost()
                                .small()
                                .icon(Icon::new(IconName::Activity))
                                .label(t("Show it now"))
                                .on_click(|_, _, cx| active_break::show(cx)),
                        ),
                    ),
            )
    }
}
