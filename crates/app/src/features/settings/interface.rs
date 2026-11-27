//! Tab placement and sidebar visibility controls for Configuration →
//! Appearance → Panels & tabs. State belongs to the workspace's existing
//! saved layout; the view forwards changes through `SettingsEvent`.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Selectable as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::layout::{control, section};
use super::{SettingsEvent, SettingsView};
use crate::features::workspaces::TabLayout;
use crate::shared::i18n::t;
use crate::shared::ui;

impl SettingsView {
    pub(super) fn interface_section(&self, cx: &mut Context<Self>) -> Div {
        let vertical = self.tab_layout == TabLayout::Vertical;
        section(IconName::PanelLeft, t("Panels & tabs"))
            .child(control(
                t("Tabs"),
                false,
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("settings-horizontal-tabs")
                            .ghost()
                            .small()
                            .selected(self.tab_layout == TabLayout::Horizontal)
                            .icon(Icon::new(IconName::List))
                            .label(t("Horizontal"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.select_tab_layout(TabLayout::Horizontal, cx)
                            })),
                    )
                    .child(
                        Button::new("settings-vertical-tabs")
                            .ghost()
                            .small()
                            .selected(vertical)
                            .icon(Icon::new(IconName::PanelLeft))
                            .label(t("Vertical"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.select_tab_layout(TabLayout::Vertical, cx)
                            })),
                    ),
            ))
            .when(vertical, |card| {
                card.child(control(
                    t("Navigator"),
                    false,
                    ui::segmented(
                        "settings-tabs-sidebar-hidden",
                        &[t("Show"), t("Hide")],
                        Some(usize::from(self.tabs_sidebar_hidden)),
                        cx.listener(move |this, index: &usize, _, cx| {
                            this.select_tabs_sidebar_hidden(*index == 1, cx)
                        }),
                    ),
                ))
                .child(control(
                    t("Navigator style"),
                    self.tabs_sidebar_hidden,
                    ui::segmented(
                        "settings-sidebar-compact",
                        &[t("Expanded"), t("Icon rail")],
                        Some(usize::from(self.sidebar_compact)),
                        cx.listener(move |this, index: &usize, _, cx| {
                            this.select_sidebar_compact(*index == 1, cx)
                        }),
                    ),
                ))
            })
            .child(control(
                t("Files"),
                false,
                ui::segmented(
                    "settings-files-sidebar",
                    &[t("Show"), t("Hide")],
                    Some(usize::from(!self.files_sidebar)),
                    cx.listener(move |this, index: &usize, _, cx| {
                        this.select_files_sidebar(*index == 0, cx)
                    }),
                ),
            ))
            .child(div().pt_2().text_xs().text_color(ui::faint()).child(t(
                "Tabs appear at the top or on the left. Files always opens on the right.",
            )))
    }

    fn select_tab_layout(&mut self, layout: TabLayout, cx: &mut Context<Self>) {
        if self.tab_layout == layout {
            return;
        }
        self.tab_layout = layout;
        cx.emit(SettingsEvent::TabLayoutChanged(layout));
        cx.notify();
    }

    fn select_sidebar_compact(&mut self, compact: bool, cx: &mut Context<Self>) {
        if self.sidebar_compact == compact {
            return;
        }
        self.sidebar_compact = compact;
        cx.emit(SettingsEvent::SidebarCompactChanged(compact));
        cx.notify();
    }

    fn select_tabs_sidebar_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        if self.tabs_sidebar_hidden == hidden {
            return;
        }
        self.tabs_sidebar_hidden = hidden;
        cx.emit(SettingsEvent::TabsSidebarHidden(hidden));
        cx.notify();
    }

    fn select_files_sidebar(&mut self, shown: bool, cx: &mut Context<Self>) {
        if self.files_sidebar == shown {
            return;
        }
        self.files_sidebar = shown;
        cx.emit(SettingsEvent::FilesSidebarShown(shown));
        cx.notify();
    }
}
