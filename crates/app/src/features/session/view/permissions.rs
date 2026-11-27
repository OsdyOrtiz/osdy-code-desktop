//! The permission mode chip and its picker.

use crate::shared::i18n::t;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::{Popover, PopoverState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::session::SessionView;
use crate::shared::ui;
use gentle_core::gentle_state::PermissionMode;

impl SessionView {
    pub(super) fn render_mode_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // The chip is this session's mode. Picking one switches it now over
        // Desktop RPC; on a Shell without it the backend restarts on the same
        // conversation, mid-turn once the turn ends, and the chip shows both
        // until then.
        let current = self.mode;
        let default = self.default_mode.or(current);
        let pending = default.filter(|d| current.is_some_and(|c| c != *d));
        let (icon, color) = mode_style(current);
        let weak = cx.entity().downgrade();
        let label = match (current, pending) {
            (Some(live), Some(next)) => format!("{} → {}", t(live.label()), t(next.label())),
            (Some(live), None) => t(live.label()).to_string(),
            (None, _) => t("Permissions").to_string(),
        };
        let tooltip = match (current, pending) {
            (Some(_), Some(next)) => t("{label} · switches to {mode} when the current turn ends")
                .replace("{label}", &label)
                .replace("{mode}", t(next.label())),
            _ => t("Permission mode: {label}").replace("{label}", &label),
        };
        // Every mode is its icon alone, in its color: Normal quiet, the
        // riskier ones in their warning tones; the name is in the tooltip.
        let trigger = Button::new("mode-picker")
            .ghost()
            .xsmall()
            .rounded(px(999.))
            .icon(Icon::new(icon))
            .text_color(if current == Some(PermissionMode::Normal) {
                ui::faint()
            } else {
                color
            })
            .tooltip(tooltip);
        Popover::new("mode-popover")
            .anchor(Anchor::BottomRight)
            .trigger(trigger)
            .w(rems(340. / 16.))
            .appearance(false)
            .content(move |_, _, cx| {
                let rows = PermissionMode::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(i, mode)| {
                        let (icon, color) = mode_style(Some(mode));
                        let weak = weak.clone();
                        let is_current = default == Some(mode);
                        h_flex()
                            .id(("mode", i))
                            .relative()
                            .w_full()
                            .gap_3()
                            .px_2()
                            .py_2()
                            .rounded(rems(ui::ISLAND_ROW / 16.))
                            .cursor_pointer()
                            .hover(|s| s.bg(ui::surface_hover()))
                            .when(is_current, |this| this.bg(ui::surface_hover()))
                            .child(
                                div()
                                    .size(rems(28. / 16.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_lg()
                                    .bg(color.opacity(0.12))
                                    .child(Icon::new(icon).small().text_color(color)),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(rems(13. / 16.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(ui::fg())
                                            .child(t(mode.label())),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(12. / 16.))
                                            .text_color(ui::muted())
                                            .child(t(mode.description())),
                                    ),
                            )
                            .when(is_current, |this| {
                                this.child(
                                    Icon::new(IconName::Check).small().text_color(ui::accent()),
                                )
                            })
                            .on_click(cx.listener(
                                move |state: &mut PopoverState, _, window, cx| {
                                    state.dismiss(window, cx);
                                    let _ = weak
                                        .update(cx, |this, cx| this.choose_mode(mode, window, cx));
                                },
                            ))
                    });
                let content = v_flex()
                    .w_full()
                    .p(rems(6. / 16.))
                    .font_family(cx.theme().font_family.clone())
                    .child(
                        div()
                            .px_2()
                            .pt_1p5()
                            .pb_1()
                            .child(ui::section_label(t("Permissions"))),
                    )
                    .children(rows)
                    .child(
                        div()
                            .px_2()
                            .pt_1p5()
                            .pb_1()
                            .text_size(rems(11. / 16.))
                            .text_color(ui::faint())
                            .child(match pending {
                                Some(next) => t("Switches to {mode} when the current turn ends.")
                                    .replace("{mode}", t(next.label())),
                                None => t("Applies now to this conversation and to new ones.")
                                    .to_string(),
                            }),
                    );
                ui::island(cx)
                    .w_full()
                    .child(ui::popover_in(
                        ElementId::Name("mode-popover-in".into()),
                        content,
                    ))
                    .into_any_element()
            })
    }
}

fn mode_style(mode: Option<PermissionMode>) -> (IconName, Hsla) {
    match mode {
        Some(PermissionMode::Normal) => (IconName::Shield, ui::muted()),
        Some(PermissionMode::Auto) => (IconName::Sparkles, ui::info()),
        Some(PermissionMode::Yolo) => (IconName::Zap, ui::warning()),
        Some(PermissionMode::SuperYolo) => (IconName::Flame, ui::danger()),
        None => (IconName::Shield, ui::faint()),
    }
}
