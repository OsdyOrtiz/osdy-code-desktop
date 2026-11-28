//! Sounds, attention alerts, and wellness reminders share one discoverable page.

use gpui_kit::component::v_flex;
use gpui_kit::*;

use super::super::SettingsView;

pub(super) fn render(view: &SettingsView, cx: &mut Context<SettingsView>) -> Div {
    v_flex()
        .gap_3()
        .child(view.notifications_section(cx))
        .child(view.hydration_section(cx))
        .child(view.active_break_section(cx))
}
