//! `Render` for the workspace window.

use super::*;

/// The title bar's height; overlay screens open below it.
pub(super) const TITLE_BAR_HEIGHT: f32 = 36.;

/// Paint and clip the content inside the width allocated by the row.
fn sidebar_surface(side: Side, content: impl IntoElement) -> Div {
    div()
        .size_full()
        .debug_selector(|| match side {
            Side::Tabs => "tabs-slide-surface".into(),
            Side::Files => "files-slide-surface".into(),
        })
        .rounded_xl()
        .bg(ui::panel())
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .overflow_hidden()
        .child(content)
}

fn drag_saved_width(
    action: Option<gentle_core::workspaces::SidebarDrag>,
    pointer: f32,
    window: f32,
    saved: f32,
) -> Option<f32> {
    use gentle_core::workspaces::shown_sidebar_width;
    let Some(gentle_core::workspaces::SidebarDrag::Width(w)) = action else {
        return None;
    };
    // The window-share cap can render a sidebar narrower than its saved width.
    // A drag that still sits at or beyond that rendered edge is not asking to
    // shrink the saved width, so keep it; anything inside the edge resizes.
    let pinned_at_cap = saved > w && w == shown_sidebar_width(saved, window) && pointer >= w;
    (!pinned_at_cap).then_some(w)
}

impl Workspace {
    fn sidebar_handle(&self, side: Side, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, revision) = match side {
            Side::Tabs => ("tabs-resize-handle", self.tabs_sidebar_motion.revision),
            Side::Files => ("files-resize-handle", self.files_sidebar_motion.revision),
        };
        let workspace = cx.entity().downgrade();
        ui::resize_handle(
            id,
            self.sidebar_drag_active(side, cx),
            !cx.has_active_drag(),
        )
        .absolute()
        .top_0()
        .bottom_0()
        .when(matches!(side, Side::Tabs), |d| d.right(rems(-6. / 16.)))
        .when(matches!(side, Side::Files), |d| d.left(rems(-6. / 16.)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_drag(
            SidebarDrag::new(side, cx.entity_id(), revision),
            move |drag, _, _, cx| {
                let _ = workspace.update(cx, |this, cx| {
                    this.sidebar_motion_mut(side).active_drag =
                        std::rc::Rc::downgrade(&drag.revision);
                    cx.notify();
                });
                cx.new(|_| Invisible)
            },
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            if event.click_count() == 2 {
                let width = match side {
                    Side::Tabs => &mut this.tabs_sidebar_width,
                    Side::Files => &mut this.files_sidebar_width,
                };
                *width = gentle_core::workspaces::SIDEBAR_DEFAULT_WIDTH;
                this.schedule_save(cx);
                cx.notify();
            }
        }))
        .tooltip(|window, cx| {
            Tooltip::new(crate::shared::i18n::t(
                "Drag to resize · double-click to reset",
            ))
            .build(window, cx)
        })
    }

    fn resize_sidebar(
        &mut self,
        event: &DragMoveEvent<SidebarDrag>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        // The row survives the handle unmounting. A missed mouse-up can
        // leave GPUI's active payload in place even when left is no longer held.
        let drag = event.drag(cx);
        if drag.owner != cx.entity_id() {
            return;
        }
        let scale = f32::from(window.rem_size()) / 16.;
        let x = event.event.position.x;
        let (distance, open, revision) = match drag.side {
            Side::Tabs => (
                x - event.bounds.left(),
                self.tabs_sidebar_visible(),
                self.tabs_sidebar_motion.revision,
            ),
            Side::Files if self.sidebar.is_some() => (
                event.bounds.right() - x,
                self.sidebar_visible,
                self.files_sidebar_motion.revision,
            ),
            Side::Files => return,
        };
        // Manual toggles take precedence over any in-flight gesture.
        if drag.revision.get() != revision {
            return;
        }
        // The row's horizontal padding is 6 px at 100% interface scale.
        let pointer = f32::from(distance) / scale - 6.;
        let window_width = f32::from(window.viewport_size().width) / scale;
        let side = drag.side;
        let held = event.event.pressed_button == Some(MouseButton::Left);
        let action = drag
            .gesture
            .borrow_mut()
            .update(pointer, window_width, open, held);
        let drag_revision = drag.revision.clone();
        let saved = match side {
            Side::Tabs => self.tabs_sidebar_width,
            Side::Files => self.files_sidebar_width,
        };
        if let Some(w) = drag_saved_width(action, pointer, window_width, saved) {
            match side {
                Side::Tabs => self.resize_tab_sidebar(w, cx),
                Side::Files if saved != w => {
                    self.files_sidebar_width = w;
                    self.schedule_save(cx);
                    cx.notify();
                }
                Side::Files => {}
            }
        }
        if !held {
            self.sidebar_motion_mut(side).active_drag = Default::default();
            cx.notify();
        }
        drag_revision.set(match side {
            Side::Tabs => self.tabs_sidebar_motion.revision,
            Side::Files => self.files_sidebar_motion.revision,
        });
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::shared::diagnostics::render("Workspace");
        if self.tab_search_language != crate::shared::i18n::language() {
            for pane in self.panes.values_mut() {
                pane.sync_chrome(cx);
            }
            self.refresh_stage_summaries(cx);
            if let Some((Overlay::Palette(view), _)) = &self.overlay {
                view.update(cx, |palette, cx| {
                    palette.refresh_card_title(|id| self.stage_title(id), cx);
                });
            }
        }
        crate::shared::i18n::sync_placeholder(
            &self.tab_search,
            "Search tabs",
            &mut self.tab_search_language,
            window,
            cx,
        );
        // Follow keyboard focus into panes (a terminal focused by its own click).
        if let Some(tab) = self.tab()
            && let Some(inside) = tab.leaves().into_iter().find(|id| {
                self.panes
                    .get(id)
                    .is_some_and(|p| p.item.focus.contains_focused(window, cx))
            })
            && Some(inside) != tab.focused()
            && let Some(tab) = self.tab_mut()
        {
            tab.state.set_focused(inside);
        }
        let has_files_sidebar = self.sidebar.is_some();
        let vertical_tabs = self.tab_layout == TabLayout::Vertical;
        let window_px_at_100 =
            f32::from(window.viewport_size().width) / (f32::from(window.rem_size()) / 16.);
        let now = std::time::Instant::now();
        let tabs_moving = self.tabs_sidebar_motion.advance(now, cx.reduce_motion());
        let files_moving = self.files_sidebar_motion.advance(now, cx.reduce_motion());
        if tabs_moving || (files_moving && has_files_sidebar) {
            ui::request_animation_frame(window);
        }
        let (tabs_width, tabs_gap) =
            self.visible_sidebar_geometry(Side::Tabs, window_px_at_100, now);
        let (files_width, files_gap) =
            self.visible_sidebar_geometry(Side::Files, window_px_at_100, now);
        let tabs_resizing = self.sidebar_drag_active(Side::Tabs, cx);
        let files_resizing = self.sidebar_drag_active(Side::Files, cx);
        let files_sidebar = self.sidebar.clone().filter(|_| files_width > 0.);
        // Native web views would cover a screen or a dialog opened over
        // them, and swallow the pointer while a pane is dragged over them;
        // device cards stop capturing while covered.
        let covered = self.overlay.is_some()
            || window.has_active_dialog(cx)
            || window.has_active_sheet(cx)
            || cx.has_active_drag()
            || crate::features::browser::popovers_open(cx);
        self.set_stages_covered(covered, cx);
        self.sync_web_views(covered, cx);
        let (devices_docked, devices_floating) = {
            let panel = self.devices.read(cx);
            let docked = panel.mode() == crate::shared::floating::Mode::Docked;
            (panel.visible() && docked, panel.visible() && !docked)
        };
        // Browser cards: floating over the window, or docked in a column at
        // its right, whose divider may be being dragged.
        let floating_stages: Vec<AnyElement> = self
            .stages
            .iter()
            .filter_map(|stage| stage.element(cx))
            .collect();
        let browser_dock = self.render_browser_dock(cx);
        let browser_dock_drag = self.browser_dock_drag_layer(cx);
        // While the surge plays, the window shakes before the rose hatches:
        // the root moves in place and asks for the next frame, so the element
        // tree (and every element's state) stays the same.
        let (shake_x, shake_y) = match self.surge {
            Some((_, started, level)) => {
                ui::request_animation_frame(window);
                crate::shared::surge::shake_at(started.elapsed(), level)
            }
            None => (0., 0.),
        };
        v_flex()
            .size_full()
            .text_color(ui::fg())
            // Only an empty workspace needs the root to own the keyboard: a
            // pane's or an overlay's own focusable elements take it in every
            // other state, and an unconditional grab here would steal focus
            // from a pane when the pointer hits blank chrome.
            .when(self.tab().is_none(), |root| root.track_focus(&self.focus))
            // A click in a web page (a device, the browser card) takes the
            // keyboard away from GPUI; any click that reaches GPUI takes it back.
            .capture_any_mouse_down(|_, window, _| {
                crate::features::browser::reclaim_keyboard(window)
            })
            .on_action(cx.listener(Self::on_new_tab))
            .on_action(cx.listener(Self::on_show_in_pane))
            .on_action(cx.listener(Self::on_close_viewer))
            .on_action(cx.listener(Self::on_open_commit_diff))
            .on_action(cx.listener(Self::on_ask_agent))
            .on_action(cx.listener(Self::on_open_terminal_at))
            .on_action(cx.listener(Self::on_new_terminal_tab))
            .on_action(cx.listener(Self::on_new_canvas_tab))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .on_action(cx.listener(Self::on_split_right))
            .on_action(cx.listener(Self::on_tile_pane))
            .on_action(cx.listener(Self::on_split_down))
            .on_action(cx.listener(Self::on_new_agent_pane))
            .on_action(cx.listener(Self::on_new_terminal_pane))
            .on_action(cx.listener(Self::on_close_pane))
            .on_action(cx.listener(Self::on_reopen_closed))
            .on_action(cx.listener(Self::on_focus_next_pane))
            .on_action(cx.listener(Self::on_focus_prev_pane))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_open_providers))
            .on_action(cx.listener(Self::on_open_sessions))
            .on_action(cx.listener(Self::on_open_palette))
            .on_action(cx.listener(Self::on_open_settings))
            .on_action(cx.listener(Self::on_open_workspaces))
            .on_action(cx.listener(Self::on_open_explorer))
            .on_action(cx.listener(Self::on_close_overlay))
            .key_context("GentleWorkspace")
            .relative()
            .left(px(shake_x))
            .top(px(shake_y))
            // The theme's backdrop, first so everything draws over it.
            .children(crate::shared::backdrop::layer(window, cx))
            .child(
                TitleBar::new()
                    .h(rems(TITLE_BAR_HEIGHT / 16.))
                    .bg(gpui_kit::transparent_black())
                    .child(
                        h_flex()
                            // Fill gpui-component's bar without counting toward
                            // its width: `w_full` there resolves against the
                            // content and pushed the buttons out of the window.
                            .flex_1()
                            .w_0()
                            .min_w_0()
                            .pr_2()
                            .gap_3()
                            .justify_between()
                            .child(
                                h_flex()
                                    .id("title-bar-chip-strip")
                                    .gap_2()
                                    .flex_1()
                                    // Width 0: the tabs take what the buttons leave.
                                    .w_0()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                            cx.stop_propagation();
                                            this.open_empty_area_menu(event.position, window, cx);
                                        }),
                                    )
                                    .child(self.render_workspace_chip(cx))
                                    .when(vertical_tabs, |row| {
                                        row.child(self.new_tab_button("title-new-tab", None, cx))
                                    })
                                    .when(!vertical_tabs, |row| {
                                        row.child(
                                            div()
                                                .flex_none()
                                                .w(px(1.))
                                                .h(rems(14. / 16.))
                                                .bg(ui::border()),
                                        )
                                        .child(self.render_tabs(cx))
                                    }),
                            )
                            .child(
                                h_flex()
                                    .flex_none()
                                    .gap_1()
                                    // Without this, Windows answers clicks on these
                                    // buttons as title-bar drags (HTCAPTION).
                                    .occlude()
                                    .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    // Tern keeps pane layout in the title bar, so
                                    // panes carry no header: one menu acts on the
                                    // focused pane, each entry with its shortcut.
                                    .when(self.tab().is_some(), |d| {
                                        d.child(self.render_layout_menu(cx)).child(
                                            div()
                                                .w(px(1.))
                                                .h(rems(14. / 16.))
                                                .mx_1()
                                                .bg(ui::border()),
                                        )
                                    })
                                    .when_some(
                                        crate::features::app_update::badge(cx),
                                        |d, (label, tip)| {
                                            d.child(
                                                div()
                                                    .id("app-update-pill")
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.8))
                                                    .tooltip(move |window, cx| {
                                                        Tooltip::new(tip.clone()).build(window, cx)
                                                    })
                                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                        cx.stop_propagation()
                                                    })
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.open_app_updates(window, cx)
                                                    }))
                                                    .child(ui::chip(label, ui::accent())),
                                            )
                                        },
                                    )
                                    .children(crate::features::music::title_button(&self.music, cx))
                                    .children(self.render_devices_button(cx))
                                    .children(self.render_tray(cx))
                                    .child({
                                        let weak = cx.entity().downgrade();
                                        div()
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .child(notices::bell(
                                                &self.notices,
                                                Rc::new({
                                                    let weak = weak.clone();
                                                    move |pane, window, cx| {
                                                        let _ = weak.update(cx, |this, cx| {
                                                            this.reveal_pane(pane, window, cx)
                                                        });
                                                    }
                                                }),
                                                Rc::new(move |origin, cx| {
                                                    weak.upgrade()
                                                        .map_or(notices::Reach::Open, |this| {
                                                            this.read(cx).notice_reach(origin)
                                                        })
                                                }),
                                                cx,
                                            ))
                                    })
                                    .when(vertical_tabs, |row| {
                                        row.child(
                                            Button::new("toggle-tab-sidebar")
                                                .ghost()
                                                .xsmall()
                                                .icon(Icon::new(if !self.tabs_sidebar_hidden {
                                                    IconName::PanelLeftClose
                                                } else {
                                                    IconName::PanelLeftOpen
                                                }))
                                                .tooltip(crate::shared::i18n::t(
                                                    if !self.tabs_sidebar_hidden {
                                                        "Hide workspace sidebar"
                                                    } else {
                                                        "Show workspace sidebar"
                                                    },
                                                ))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.toggle_tabs_sidebar(cx)
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("open-palette")
                                            .ghost()
                                            .xsmall()
                                            .icon(Icon::new(IconName::Command))
                                            .tooltip(
                                                crate::shared::i18n::t("Commands ({shortcut})")
                                                    .replace("{shortcut}", &ui::shortcut("K")),
                                            )
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.on_open_palette(&OpenPalette, window, cx)
                                            })),
                                    )
                                    // The screens behind one menu; it's lit while
                                    // one of them is open.
                                    .child(self.render_screens_menu(cx))
                                    .when(has_files_sidebar, |d| {
                                        d.child(
                                            Button::new("toggle-sidebar")
                                                .ghost()
                                                .xsmall()
                                                .icon(Icon::new(if self.sidebar_visible {
                                                    IconName::PanelRightClose
                                                } else {
                                                    IconName::PanelRightOpen
                                                }))
                                                .tooltip(
                                                    crate::shared::i18n::t(
                                                        if self.sidebar_visible {
                                                            "Hide Files sidebar ({shortcut})"
                                                        } else {
                                                            "Show Files sidebar ({shortcut})"
                                                        },
                                                    )
                                                    .replace("{shortcut}", &ui::shortcut("B")),
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.toggle_sidebar(cx)
                                                })),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .px_1p5()
                    .pb_1p5()
                    .gap_1p5()
                    .on_drag_move(cx.listener(
                        |this, event: &DragMoveEvent<SidebarDrag>, window, cx| {
                            this.resize_sidebar(event, window, cx);
                        },
                    ))
                    // The vertical tab navigator remains independent of the Files panel.
                    .when(tabs_width > 0., |row| {
                        row.child(
                            div()
                                .id("vertical-tab-panel")
                                .debug_selector(|| "tabs-slide-row".into())
                                .w(rems(tabs_width / 16.))
                                .mr(rems((tabs_gap - 6.) / 16.))
                                .flex_none()
                                .h_full()
                                .relative()
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        this.open_empty_area_menu(event.position, window, cx);
                                    }),
                                )
                                .child(sidebar_surface(Side::Tabs, self.render_tab_sidebar(cx)))
                                .when(
                                    tabs_resizing
                                        || (vertical_tabs && !self.sidebar_compact && !tabs_moving),
                                    |column| column.child(self.sidebar_handle(Side::Tabs, cx)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .debug_selector(|| "sidebar-adjacent-body".into())
                            .child(self.render_body(window, cx))
                            .child(self.measure_conversation()),
                    )
                    .children(browser_dock)
                    .when(devices_docked, |row| row.child(self.devices.clone()))
                    // Ctrl/Cmd+B toggles only the right Files/Changes/Branches sidebar.
                    .when_some(files_sidebar, |row, view| {
                        row.child(
                            div()
                                .id("files-sidebar-panel")
                                .w(rems(files_width / 16.))
                                .ml(rems((files_gap - 6.) / 16.))
                                .flex_none()
                                .h_full()
                                .relative()
                                .child(sidebar_surface(
                                    Side::Files,
                                    view.cached(StyleRefinement::default().size_full()),
                                ))
                                .when(
                                    files_resizing || (self.sidebar_visible && !files_moving),
                                    |column| column.child(self.sidebar_handle(Side::Files, cx)),
                                ),
                        )
                    }),
            )
            .children(floating_stages)
            .children(self.render_badge_picker(window, cx))
            .children(self.render_context_menu(cx))
            .when(devices_floating, |root| root.child(self.devices.clone()))
            .children(browser_dock_drag)
            .children(self.overlay.as_ref().map(|(overlay, _)| {
                let (view, full_window) = match overlay {
                    Overlay::Providers(view) => (view.clone().into_any_element(), false),
                    Overlay::Sessions(view) => (view.clone().into_any_element(), false),
                    Overlay::Palette(view) => (view.clone().into_any_element(), false),
                    Overlay::Explorer(view) => (view.clone().into_any_element(), false),
                    Overlay::Usage(view) => (view.clone().into_any_element(), false),
                    Overlay::Memory(view) => (view.clone().into_any_element(), false),
                    Overlay::Docs(view) => (view.clone().into_any_element(), false),
                    Overlay::Settings(view) => (view.clone().into_any_element(), false),
                    Overlay::Profiles(view) => (view.clone().into_any_element(), false),
                    Overlay::CodexAccounts(view) => (view.clone().into_any_element(), false),
                    Overlay::Review(view) => (view.clone().into_any_element(), false),
                    // First-run screens still cover the whole window.
                    Overlay::Onboarding(view) => (view.clone().into_any_element(), true),
                    Overlay::Welcome(view) => (view.clone().into_any_element(), true),
                };
                if full_window {
                    return view;
                }
                // Below the title bar, so its buttons can switch screens.
                div()
                    .absolute()
                    .top(rems(TITLE_BAR_HEIGHT / 16.))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(view)
                    .into_any_element()
            }))
            .when_some(self.surge, |root, (run, _, level)| {
                root.child(crate::shared::surge::overlay(run, level))
            })
            .when_some(crate::features::hydration::playing(cx), |root, run| {
                root.child(crate::features::hydration::overlay(run, cx))
            })
            .when_some(crate::features::active_break::playing(cx), |root, run| {
                root.child(
                    div()
                        .absolute()
                        .top(rems(TITLE_BAR_HEIGHT / 16. + 0.75))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(crate::features::active_break::overlay(run, cx)),
                )
            })
    }
}

impl Workspace {
    /// The title bar's device panel button, while "Use devices" is on.
    fn render_devices_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !crate::features::devices::hub(cx).read(cx).prefs().enabled {
            return None;
        }
        let open = self.devices.read(cx).visible();
        Some(
            div()
                // Clicks here are not title-bar drags.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new("title-devices")
                        .ghost()
                        .xsmall()
                        .icon(Icon::new(IconName::Smartphone))
                        .when(open, |b| b.text_color(ui::petal()))
                        .tooltip(crate::shared::i18n::t("Devices: simulators and emulators"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.devices.update(cx, |panel, cx| panel.toggle(cx))
                        })),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::drag_saved_width;
    use gentle_core::workspaces::{SidebarGesture, shown_sidebar_width};

    #[test]
    fn drag_at_window_cap_preserves_saved_width() {
        // Tabs and Files both measure the pointer as a width from their outer
        // edge, so one rule covers both sidebars.
        let mut gesture = SidebarGesture::default();
        let saved = 420.;
        assert_eq!(shown_sidebar_width(saved, 800.), 320.);
        for pointer in [320., 370.] {
            let action = gesture.update(pointer, 800., true, true);
            assert_eq!(drag_saved_width(action, pointer, 800., saved), None);
        }
        let action = gesture.update(300., 800., true, true);
        assert_eq!(drag_saved_width(action, 300., 800., saved), Some(300.));
    }
}
