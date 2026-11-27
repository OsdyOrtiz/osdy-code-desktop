//! A workspace's badge: its initials or a chosen icon, on a theme color (one
//! its name picks, or one chosen), as the sidebar and the title bar's
//! workspace chip show it. The workspace menu's "Badge…" item opens the
//! picker for both icon and color.

use super::*;
use crate::features::workspaces::{BadgeColor, BadgeIcon};

/// The picker's width, in px at 100 %: eight 30 px cells and their gaps.
const PICKER_WIDTH: f32 = 300.;
const CELL: f32 = 30.;

/// What a workspace's badge shows and in which color.
#[derive(Clone)]
pub(super) struct Mark {
    name: SharedString,
    badge: Badge,
}

impl Mark {
    pub fn of(space: &Space) -> Self {
        Self {
            name: space.name.clone().into(),
            badge: space.badge,
        }
    }

    /// The chosen color, else the one the name picks; both follow the theme.
    pub fn hue(&self) -> Hsla {
        badge_hue(
            self.badge
                .color
                .unwrap_or_else(|| BadgeColor::for_name(&self.name)),
        )
    }

    /// The chosen icon, or the first `letters` initials, about `size` px
    /// tall, in `color`.
    pub fn glyph(&self, size: f32, letters: usize, color: Hsla) -> AnyElement {
        match self.badge.icon {
            Some(icon) => Icon::new(icon.icon())
                .size(rems(size * 1.15 / 16.))
                .text_color(color)
                .into_any_element(),
            None => div()
                .text_size(rems(size / 16.))
                .font_weight(FontWeight::BOLD)
                .line_height(relative(1.))
                .text_color(color)
                .child(initials(&self.name, letters))
                .into_any_element(),
        }
    }
}

pub(super) fn badge_hue(color: BadgeColor) -> Hsla {
    match color {
        BadgeColor::Accent => ui::accent(),
        BadgeColor::Info => ui::info(),
        BadgeColor::Teal => ui::teal(),
        BadgeColor::Success => ui::success(),
        BadgeColor::Champagne => ui::champagne(),
        BadgeColor::Warning => ui::warning(),
        BadgeColor::Danger => ui::danger(),
        BadgeColor::Purple => ui::purple(),
    }
}

/// The first letters of a workspace's first `words` words, uppercase.
pub(super) fn initials(name: &str, words: usize) -> String {
    let mut out: String = name
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|word| word.chars().next())
        .take(words)
        .flat_map(char::to_uppercase)
        .collect();
    if out.is_empty() {
        out.push('·');
    } else if out.chars().count() < words {
        // One word: its second letter, lowercase, as in "Se" or "Fe".
        if let Some(second) = name.chars().filter(|c| c.is_alphanumeric()).nth(1) {
            out.extend(second.to_lowercase());
        }
    }
    out
}

/// The small square badge of the title bar's workspace chip and the tree
/// navigators.
pub(super) fn workspace_avatar(mark: &Mark) -> Div {
    let color = mark.hue();
    div()
        .size(rems(18. / 16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(5. / 16.))
        .bg(color.opacity(0.18))
        .child(mark.glyph(10.5, 1, color))
}

impl Workspace {
    /// Open the badge picker for workspace `index` at `position`.
    pub(super) fn open_badge_picker(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(window, cx);
        self.badge_picker = Some((index, position));
        crate::features::browser::popover_open("workspace-badge", true, window, cx);
        cx.notify();
    }

    pub(super) fn close_badge_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.badge_picker.take().is_some() {
            crate::features::browser::popover_open("workspace-badge", false, window, cx);
            cx.notify();
        }
    }

    fn set_space_badge(&mut self, index: usize, badge: Badge, cx: &mut Context<Self>) {
        let Some(space) = self.spaces.get_mut(index) else {
            return;
        };
        if space.badge != badge {
            space.badge = badge;
            self.schedule_save(cx);
            cx.notify();
        }
    }

    /// The picker: the badge's preview, an icon grid (initials first) and the
    /// theme's colors (the name's own first). Changes apply at once.
    pub(super) fn render_badge_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (index, position) = self.badge_picker?;
        let Some(space) = self.spaces.get(index) else {
            self.close_badge_picker(window, cx);
            return None;
        };
        let mark = Mark::of(space);
        let badge = space.badge;
        let hue = mark.hue();
        let cell = |id: ElementId, selected: bool, tip: SharedString| {
            div()
                .id(id)
                .size(rems(CELL / 16.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_lg()
                .border_1()
                .cursor_pointer()
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .map(|d| {
                    if selected {
                        d.bg(hue.opacity(0.2)).border_color(hue.opacity(0.55))
                    } else {
                        d.border_color(gpui_kit::transparent_black())
                            .hover(|s| s.bg(ui::surface_hover()))
                    }
                })
        };
        let mut icons = h_flex().flex_wrap().gap_1().child(
            cell(
                "badge-initials".into(),
                badge.icon.is_none(),
                crate::shared::i18n::t("Initials").into(),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_space_badge(
                    index,
                    Badge {
                        icon: None,
                        ..badge
                    },
                    cx,
                )
            }))
            .child(
                div()
                    .text_size(rems(11. / 16.))
                    .font_weight(FontWeight::BOLD)
                    .line_height(relative(1.))
                    .text_color(if badge.icon.is_none() {
                        hue
                    } else {
                        ui::muted()
                    })
                    .child(initials(&mark.name, 2)),
            ),
        );
        for (i, icon) in BadgeIcon::ALL.into_iter().enumerate() {
            let selected = badge.icon == Some(icon);
            icons = icons.child(
                cell(
                    ("badge-icon", i).into(),
                    selected,
                    crate::shared::i18n::t(icon.label()).into(),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let badge = Badge {
                        icon: Some(icon),
                        ..badge
                    };
                    this.set_space_badge(index, badge, cx)
                }))
                .child(Icon::new(icon.icon()).small().text_color(if selected {
                    hue
                } else {
                    ui::muted()
                })),
            );
        }
        let swatch = |id: ElementId, color: Hsla, selected: bool, tip: SharedString| {
            div()
                .id(id)
                .p(px(2.))
                .rounded_full()
                .border_2()
                .border_color(if selected {
                    ui::fg()
                } else {
                    gpui_kit::transparent_black()
                })
                .cursor_pointer()
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .child(
                    div()
                        .size(rems(18. / 16.))
                        .rounded_full()
                        .bg(color)
                        .hover(|s| s.opacity(0.8)),
                )
        };
        let auto = badge_hue(BadgeColor::for_name(&mark.name));
        let mut colors = h_flex().flex_wrap().gap_0p5().child(
            swatch(
                "badge-color-auto".into(),
                auto,
                badge.color.is_none(),
                crate::shared::i18n::t("From the name").into(),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_space_badge(
                    index,
                    Badge {
                        color: None,
                        ..badge
                    },
                    cx,
                )
            })),
        );
        for (i, color) in BadgeColor::ALL.into_iter().enumerate() {
            colors = colors.child(
                swatch(
                    ("badge-color", i).into(),
                    badge_hue(color),
                    badge.color == Some(color),
                    crate::shared::i18n::t(color.label()).into(),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let badge = Badge {
                        color: Some(color),
                        ..badge
                    };
                    this.set_space_badge(index, badge, cx)
                })),
            );
        }
        let island = ui::island(cx)
            .occlude()
            .w(rems(PICKER_WIDTH / 16.))
            .p_3()
            .gap_2()
            .on_mouse_down_out(
                cx.listener(|this, _, window, cx| this.close_badge_picker(window, cx)),
            )
            .child(
                h_flex()
                    .gap_2p5()
                    .pb_1()
                    .child(
                        div()
                            .size(rems(36. / 16.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .bg(hue.opacity(0.18))
                            .child(mark.glyph(15., 2, hue)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(ui::fg())
                                    .child(mark.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ui::faint())
                                    .child(crate::shared::i18n::t("Colors follow the theme")),
                            ),
                    )
                    .when(!badge.is_default(), |d| {
                        d.child(
                            Button::new("badge-reset")
                                .ghost()
                                .xsmall()
                                .label(crate::shared::i18n::t("Reset"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_space_badge(index, Badge::default(), cx)
                                })),
                        )
                    }),
            )
            .child(ui::section_label(crate::shared::i18n::t("Icon")))
            .child(icons)
            .child(
                div()
                    .pt_1()
                    .child(ui::section_label(crate::shared::i18n::t("Color"))),
            )
            .child(colors);
        Some(
            deferred(
                anchored()
                    .position(position)
                    .snap_to_window_with_margin(px(8.))
                    .child(ui::popover_in("workspace-badge-in", island)),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
