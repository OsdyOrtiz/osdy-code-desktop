//! Device permissions and runtime prerequisites, and where pages open (the
//! browser). Maestro lives in Integrations.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::super::{SettingsView, navigation::SettingsPage};
use crate::shared::i18n::t;

pub(super) fn render(view: &SettingsView, cx: &mut Context<SettingsView>) -> Div {
    v_flex()
        .gap_3()
        .child(view.devices_section(cx))
        .child(
            h_flex().child(
                Button::new("devices-maestro-settings")
                    .ghost()
                    .small()
                    .icon(Icon::new(IconName::ArrowUpRight))
                    .label(t("Configure Maestro UI testing"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigate_to(SettingsPage::Integrations, cx)
                    })),
            ),
        )
        .child(view.browser_section(cx))
}
