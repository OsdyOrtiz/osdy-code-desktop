//! Narrow workspace rail. It uses the same session and workspace model as
//! the expanded navigator, without changing the right-hand Files panel.

use super::super::menu::WorkspaceMenuSource;
use super::*;

impl Workspace {
    pub(super) fn render_compact_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = v_flex()
            .id("compact-workspaces")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .w_full()
            .items_center()
            .gap_1();

        for space in self.sidebar_spaces() {
            let index = space.index;
            let tip = space.tooltip();
            let color = space.mark.hue();
            let row = h_flex()
                .id(("compact-workspace", index))
                .relative()
                .size(rems(34. / 16.))
                .flex_none()
                .justify_center()
                .rounded_md()
                .bg(color.opacity(0.12))
                .border_1()
                .border_color(if space.current {
                    color
                } else {
                    gpui_kit::transparent_black()
                })
                .hover(|d| d.bg(ui::surface_hover()))
                .cursor_pointer()
                .when(!self.popups_open(), |d| {
                    d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.current != index {
                        this.switch_workspace(index, window, cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.open_workspace_menu(
                            index,
                            WorkspaceMenuSource::Chip,
                            event.position,
                            window,
                            cx,
                        );
                    }),
                )
                .child(space.mark.glyph(13., 2, color))
                .children(
                    space
                        .dot()
                        .map(|dot| div().absolute().top(px(2.)).right(px(2.)).child(dot)),
                );
            list = list.child(row);
            // The compact rail deliberately ignores the expanded sidebar's
            // folding state. Every workspace exposes every session.
            let mut number = 0;
            for tab in self.sidebar_tabs(index) {
                if tab.panes > 1 {
                    let model = &self.spaces[index].tabs[tab.index];
                    let entries: Vec<_> = match &model.state {
                        TabState::Built { tree, .. } => tree
                            .leaves()
                            .into_iter()
                            .filter_map(|id| {
                                let pane = self.panes.get(&id)?;
                                Some((
                                    Some(id),
                                    pane.title.clone(),
                                    pane.item.kind,
                                    Light::of(pane.needs_you, pane.running, pane.background),
                                    pane.background,
                                ))
                            })
                            .collect(),
                        TabState::Unbuilt(saved) => saved
                            .layout
                            .leaves()
                            .iter()
                            .map(|pane| {
                                (
                                    None,
                                    saved_title(pane),
                                    pane.kind,
                                    None,
                                    Background::default(),
                                )
                            })
                            .collect(),
                    };
                    let tab_id = tab.id;
                    for (leaf, (pane, title, kind, light, background)) in
                        entries.into_iter().enumerate()
                    {
                        number += 1;
                        let selected = tab.shown && pane.is_some() && pane == tab.pane;
                        let color = match light {
                            Some(light) => ui::light_color(light),
                            None if selected => ui::fg(),
                            None => ui::muted(),
                        };
                        let mut tip = format!("{} · {} · {}", space.name, number, title);
                        if let Some(background) = background.describe() {
                            tip.push_str(&format!(" · {background}"));
                        }
                        let tip: SharedString = tip.into();
                        list = list.child(
                            h_flex()
                                .id(SharedString::from(format!("compact-pane-{tab_id}-{leaf}")))
                                .relative()
                                .size(rems(34. / 16.))
                                .flex_none()
                                .gap_px()
                                .justify_center()
                                .rounded_md()
                                .cursor_pointer()
                                .when(selected, |d| d.bg(ui::raised()))
                                .hover(|d| d.bg(ui::surface_hover()))
                                .when(!self.popups_open(), |d| {
                                    d.tooltip(move |window, cx| {
                                        Tooltip::new(tip.clone()).build(window, cx)
                                    })
                                })
                                .child(Icon::new(kind.icon()).xsmall().text_color(color))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(ui::mono())
                                        .text_color(color)
                                        .child(number.to_string()),
                                )
                                .children(light.map(|light| {
                                    div()
                                        .absolute()
                                        .top(px(2.))
                                        .right(px(2.))
                                        .child(ui::light_dot(light, 5.))
                                }))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Some(pane) = pane {
                                        this.reveal_pane(pane, window, cx);
                                    } else if let Some(tab_index) = this
                                        .spaces
                                        .get(index)
                                        .and_then(|s| s.tabs.iter().position(|t| t.id == tab_id))
                                    {
                                        if let TabState::Unbuilt(saved) =
                                            &mut this.spaces[index].tabs[tab_index].state
                                        {
                                            saved.focused = leaf;
                                        }
                                        this.spaces[index].active = tab_index;
                                        this.switch_workspace(index, window, cx);
                                    }
                                })),
                        );
                    }
                    continue;
                }

                number += 1;
                let tip: SharedString =
                    format!("{} · {} · {}", space.name, number, tab.tooltip()).into();
                let row = h_flex()
                    .id(("compact-tab", tab.id))
                    .relative()
                    .size(rems(34. / 16.))
                    .flex_none()
                    .gap_px()
                    .justify_center()
                    .rounded_md()
                    .when(tab.shown, |d| d.bg(ui::raised()))
                    .hover(|d| d.bg(ui::surface_hover()))
                    .when(!self.popups_open(), |d| {
                        d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    })
                    .child(Icon::new(tab.kind.icon()).xsmall().text_color(tab.tone()))
                    .child(
                        div()
                            .text_xs()
                            .font_family(ui::mono())
                            .text_color(tab.tone())
                            .child(number.to_string()),
                    )
                    .children(
                        tab.status_dot()
                            .map(|dot| div().absolute().top(px(2.)).right(px(2.)).child(dot)),
                    );
                list = list.child(self.tab_row_behavior(row, &tab, cx));
            }
            list = list.child(
                h_flex()
                    .id(("compact-add-session", index))
                    .size(rems(30. / 16.))
                    .flex_none()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|d| d.bg(ui::surface_hover()))
                    .when(!self.popups_open(), |d| {
                        d.tooltip(|window, cx| {
                            Tooltip::new("New agent or terminal").build(window, cx)
                        })
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_workspace_add_menu(index, event.position, window, cx);
                        }),
                    )
                    .child(Icon::new(IconName::Plus).small().text_color(ui::muted())),
            );
            list = list.child(
                div()
                    .flex_none()
                    .w(rems(24. / 16.))
                    .h(px(1.))
                    .bg(ui::border()),
            );
        }

        v_flex()
            .size_full()
            .items_center()
            .gap_1()
            .py_2()
            .child(
                Button::new("compact-expand")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(IconName::PanelLeftOpen))
                    .tooltip("Expand workspace sidebar")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar_compact(cx))),
            )
            .child(list)
            .child(
                Button::new("compact-new-workspace")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(IconName::FolderPlus))
                    .tooltip("New workspace")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_workspace_folder(None, window, cx);
                    })),
            )
            .into_any_element()
    }
}
