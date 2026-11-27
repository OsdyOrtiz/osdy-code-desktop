//! The selection's details, over the canvas' top-right corner: a node's
//! kind, status, group, tech and description (editable unless the view is
//! locked), an edge's style and label, a group's description, or several
//! nodes at once; with Rename, Group, Ask about this and Delete.

use gentle_core::genui::changes::NodePatch;
use gentle_core::genui::diagram::{EdgeStyle, NodeKind, Status};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::diagram::{DiagramView, Selection};
use super::view::{node_icon, node_tone, status_color, style_name};
use crate::shared::ui;

impl DiagramView {
    pub(super) fn render_details(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.drag.is_some() {
            return None;
        }
        let selection = self.selection.as_ref()?;
        let editable = !self.locked;
        let action = |id: &'static str, icon: IconName, label: &'static str| {
            Button::new(id)
                .debug_selector(|| id.into())
                .ghost()
                .xsmall()
                .text_color(ui::muted())
                .icon(Icon::new(icon))
                .label(crate::shared::i18n::t(label))
        };
        let rename = action("genui-rename", IconName::Pencil, "Rename").on_click(cx.listener(
            |this, _, window, cx| {
                if let Some(selection) = this.selection.clone() {
                    this.start_rename(selection, window, cx);
                }
            },
        ));
        let ask = action("genui-ask", IconName::MessageSquare, "Ask about this")
            .on_click(cx.listener(|this, _, _, cx| this.ask(cx)));
        let delete = action("genui-delete", IconName::Trash, "Delete")
            .on_click(cx.listener(|this, _, _, cx| this.delete_selection(cx)));
        let text = |text: String| {
            div()
                .text_size(rems(12. / 16.))
                .text_color(ui::muted())
                .whitespace_normal()
                .child(text)
        };
        let title = |icon: Icon, label: String| {
            h_flex().gap_2().child(icon).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ui::fg())
                    .child(label),
            )
        };
        let mut panel = ui::island(cx)
            .id("genui-details")
            .absolute()
            .top_2()
            .right_2()
            .w(rems(260. / 16.))
            .max_h(rems(320. / 16.))
            .overflow_y_scroll()
            .p_3()
            .gap_2()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll(|_, _, cx| cx.stop_propagation());
        let weak = cx.entity().downgrade();
        let mut actions = h_flex().gap_1().flex_wrap();
        match selection {
            Selection::Node(id) => {
                let node = self.working.node(id)?;
                panel = panel.child(title(
                    Icon::new(node_icon(node))
                        .small()
                        .text_color(node_tone(node)),
                    node.label.clone(),
                ));
                if editable {
                    let (kind, status, group) = (node.kind, node.status, node.group.clone());
                    let pick =
                        |field: &'static str,
                         label: String,
                         menu: Box<dyn Fn(PopupMenu) -> PopupMenu>| {
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .w(rems(76. / 16.))
                                        .text_size(rems(11. / 16.))
                                        .text_color(ui::faint())
                                        .child(field),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("genui-field-{field}")))
                                        .debug_selector(|| format!("genui-field-{field}"))
                                        .ghost()
                                        .xsmall()
                                        .label(label)
                                        .dropdown_caret(true)
                                        .dropdown_menu(move |m, _, _| menu(m)),
                                )
                        };
                    let item = {
                        let weak = weak.clone();
                        move |label: String, checked: bool, patch: NodePatch, node: String| {
                            let weak = weak.clone();
                            PopupMenuItem::new(SharedString::from(label))
                                .checked(checked)
                                .on_click(move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.patch_node(&node, patch.clone(), cx)
                                    });
                                })
                        }
                    };
                    let kinds = {
                        let item = item.clone();
                        let node = id.clone();
                        move |mut menu: PopupMenu| {
                            for k in NodeKind::ALL {
                                menu = menu.item(item(
                                    crate::shared::i18n::t(k.name()).to_string(),
                                    k == kind,
                                    NodePatch {
                                        kind: Some(k),
                                        ..NodePatch::default()
                                    },
                                    node.clone(),
                                ));
                            }
                            menu
                        }
                    };
                    let statuses = {
                        let item = item.clone();
                        let node = id.clone();
                        move |mut menu: PopupMenu| {
                            for s in [None].into_iter().chain(Status::ALL.map(Some)) {
                                menu = menu.item(item(
                                    crate::shared::i18n::t(s.map_or("none", Status::name))
                                        .to_string(),
                                    s == status,
                                    NodePatch {
                                        status: Some(s),
                                        ..NodePatch::default()
                                    },
                                    node.clone(),
                                ));
                            }
                            menu
                        }
                    };
                    let groups: Vec<(String, String)> = self
                        .working
                        .groups
                        .iter()
                        .map(|g| (g.id.clone(), g.label.clone()))
                        .collect();
                    let group_menu = {
                        let item = item.clone();
                        let node = id.clone();
                        let weak = weak.clone();
                        let group = group.clone();
                        move |mut menu: PopupMenu| {
                            menu = menu.item(item(
                                crate::shared::i18n::t("No group").into(),
                                group.is_none(),
                                NodePatch {
                                    group: Some(None),
                                    ..NodePatch::default()
                                },
                                node.clone(),
                            ));
                            for (gid, label) in &groups {
                                menu = menu.item(item(
                                    label.clone(),
                                    group.as_ref() == Some(gid),
                                    NodePatch {
                                        group: Some(Some(gid.clone())),
                                        ..NodePatch::default()
                                    },
                                    node.clone(),
                                ));
                            }
                            let weak = weak.clone();
                            let node = node.clone();
                            menu.separator().item(
                                PopupMenuItem::new(crate::shared::i18n::t("New group…")).on_click(
                                    move |_, window, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.new_group_for(&node, window, cx)
                                        });
                                    },
                                ),
                            )
                        }
                    };
                    let group_label = group
                        .as_deref()
                        .map(|g| self.working.group(g).map_or(g, |g| g.label.as_str()))
                        .unwrap_or_else(|| crate::shared::i18n::t("none"))
                        .to_string();
                    panel = panel
                        .child(pick(
                            "kind",
                            crate::shared::i18n::t(kind.name()).to_string(),
                            Box::new(kinds),
                        ))
                        .child(pick(
                            "status",
                            crate::shared::i18n::t(status.map_or("none", Status::name)).to_string(),
                            Box::new(statuses),
                        ))
                        .child(pick("group", group_label, Box::new(group_menu)));
                    if let Some(fields) = self.fields.as_ref().filter(|f| f.target == *selection) {
                        if let Some(tech) = &fields.tech {
                            panel = panel.child(field_input(tech));
                        }
                        panel = panel.child(field_input(&fields.description));
                    }
                } else {
                    let group = node
                        .group
                        .as_deref()
                        .map(|g| self.working.group(g).map_or(g, |g| g.label.as_str()));
                    panel = panel
                        .child(
                            h_flex()
                                .gap_1()
                                .flex_wrap()
                                .child(ui::chip(
                                    crate::shared::i18n::t(node.kind.name()),
                                    node_tone(node),
                                ))
                                .when_some(node.status, |d, status| {
                                    d.child(ui::chip(
                                        crate::shared::i18n::t(status.name()),
                                        status_color(status),
                                    ))
                                })
                                .when_some(group, |d, group| {
                                    d.child(ui::chip(group.to_string(), ui::muted()))
                                }),
                        )
                        .when_some(node.tech.clone(), |d, tech| {
                            d.child(
                                div()
                                    .font_family(ui::mono())
                                    .text_size(ui::code_rems(11.))
                                    .text_color(ui::muted())
                                    .child(tech),
                            )
                        })
                        .when_some(node.description.clone(), |d, description| {
                            d.child(text(description))
                        });
                }
                if let Some(code_ref) = &node.code_ref {
                    panel = panel.child(super::refs::chip(
                        format!("genui-details-ref-{}", node.id),
                        code_ref,
                        &self.env,
                        cx,
                    ));
                }
                panel = panel.child(
                    div()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(10.5))
                        .text_color(ui::faint())
                        .child(node.id.clone()),
                );
                actions = actions
                    .when(editable, |d| d.child(rename))
                    .child(ask)
                    .when(editable, |d| d.child(delete));
            }
            Selection::Nodes(ids) => {
                panel = panel.child(title(
                    Icon::new(IconName::SquareDashed)
                        .small()
                        .text_color(ui::accent()),
                    super::count(ids.len(), "{count} nodes", "{count} nodes"),
                ));
                panel = panel.child(text(
                    ids.iter()
                        .filter_map(|id| self.working.node(id).map(|n| n.label.clone()))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
                actions = actions
                    .when(editable, |d| {
                        d.child(
                            action("genui-group-selection", IconName::Group, "Group").on_click(
                                cx.listener(|this, _, window, cx| this.group_selection(window, cx)),
                            ),
                        )
                    })
                    .child(ask)
                    .when(editable, |d| d.child(delete));
            }
            Selection::Edge(id) => {
                let edge = self.working.edge(id)?;
                let name = |node: &str| {
                    self.working
                        .node(node)
                        .map_or(node.to_string(), |n| n.label.clone())
                };
                panel = panel.child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg())
                        .whitespace_normal()
                        .child(format!("{} → {}", name(&edge.from), name(&edge.to))),
                );
                if editable {
                    let current = edge.style;
                    let edge_id = edge.id.clone();
                    let weak = weak.clone();
                    panel = panel.child(
                        Button::new("genui-style")
                            .debug_selector(|| "genui-style".into())
                            .ghost()
                            .xsmall()
                            .label(style_name(current))
                            .dropdown_caret(true)
                            .dropdown_menu(move |mut menu, _, _| {
                                for style in EdgeStyle::ALL {
                                    let weak = weak.clone();
                                    let edge = edge_id.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(style_name(style))
                                            .checked(style == current)
                                            .on_click(move |_, _, cx| {
                                                let _ = weak.update(cx, |this, cx| {
                                                    this.set_edge_style(&edge, style, cx)
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    );
                } else {
                    panel = panel.child(ui::chip(style_name(edge.style), ui::info()));
                }
                panel = panel.when_some(edge.label.clone(), |d, label| d.child(text(label)));
                actions = actions
                    .when(editable, |d| d.child(rename))
                    .child(ask)
                    .when(editable, |d| d.child(delete));
            }
            Selection::Group(id) => {
                let group = self.working.group(id)?;
                let members = self
                    .working
                    .nodes
                    .iter()
                    .filter(|n| n.group.as_deref() == Some(id.as_str()))
                    .count();
                panel = panel
                    .child(title(
                        Icon::new(IconName::Group).small().text_color(ui::muted()),
                        group.label.clone(),
                    ))
                    .child(text(super::count(members, "{count} node", "{count} nodes")));
                match self.fields.as_ref().filter(|f| f.target == *selection) {
                    Some(fields) => panel = panel.child(field_input(&fields.description)),
                    None => {
                        panel = panel.when_some(group.description.clone(), |d, description| {
                            d.child(text(description))
                        })
                    }
                }
                actions =
                    actions
                        .when(editable, |d| d.child(rename))
                        .child(ask)
                        .when(editable, |d| {
                            d.child(
                                action("genui-delete", IconName::Ungroup, "Ungroup").on_click(
                                    cx.listener(|this, _, _, cx| this.delete_selection(cx)),
                                ),
                            )
                        });
            }
        }
        Some(panel.child(actions).into_any_element())
    }
}

/// A details text field, committed on Enter or when it loses focus.
fn field_input(input: &Entity<gpui_kit::component::input::InputState>) -> Div {
    ui::field()
        .h(rems(28. / 16.))
        .child(Input::new(input).appearance(false).xsmall())
}
