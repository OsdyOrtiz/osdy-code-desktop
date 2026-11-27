//! Drawing a diagram surface, React Flow style: a header with the editing
//! tools, a dotted canvas with group boxes, edges, node cards and edge
//! labels, floating controls (bottom-left), a minimap (bottom-right) and the
//! selection's details (top-right, `details.rs`), and a footer with the
//! pending edits; and the conversation rows around it: a surface being
//! drawn, one that failed, older versions and the user's sent edits.

use std::time::Instant;

use gentle_core::genui::diagram::{Direction, EdgeStyle, Node, NodeKind, Status};
use gentle_core::genui::layout::{self, GROUP_PAD, Rect};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, Size, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::diagram::{DiagramEvent, DiagramView, Drag, READABLE_ZOOM, Selection, SendState};
use super::look;
use super::motion::{self, MARCH_SPEED};
use super::paint::{self, EdgeDraw, EdgeLook, Frame, MiniMap};
use super::pointer::ZOOM_STEP;
use crate::shared::{backdrop, ui};

/// The card's height in the conversation (px at 100 %): a fixed size, so
/// the surface can be a cached view that only redraws when it changes.
pub(super) const CARD_HEIGHT: f32 = 460.;
/// Below this scale node descriptions, the kind · tech line and edge labels
/// are left out: smaller than ~4 px they are noise, not text.
const DETAIL_ZOOM: f32 = 0.35;
/// The minimap's size (px at 100 %).
const MINIMAP_W: f32 = 168.;
const MINIMAP_H: f32 = 108.;
/// Where a group's tag sits inside its box, from the top-left corner
/// (diagram units).
const GROUP_TAG_INSET: f32 = 10.;

fn kind_icon(kind: NodeKind) -> IconName {
    match kind {
        NodeKind::Service => IconName::Server,
        NodeKind::Database => IconName::Database,
        NodeKind::Queue => IconName::Layers,
        NodeKind::Cache => IconName::Zap,
        NodeKind::Client => IconName::Monitor,
        NodeKind::External => IconName::Globe,
        NodeKind::Storage => IconName::HardDrive,
        NodeKind::Function => IconName::SquareFunction,
        NodeKind::Note => IconName::StickyNote,
    }
}

/// A kind's default tone (Eraser's palette per kind); a node's `color`
/// overrides it (`node_tone`).
fn kind_color(kind: NodeKind) -> Hsla {
    match kind {
        NodeKind::Service => ui::info(),
        NodeKind::Function => ui::purple(),
        NodeKind::Database => ui::success(),
        NodeKind::Storage => ui::teal(),
        NodeKind::Queue => ui::warning(),
        NodeKind::Cache => ui::danger(),
        NodeKind::Client => ui::accent(),
        NodeKind::External => ui::muted(),
        NodeKind::Note => ui::champagne(),
    }
}

/// The tone a node wears everywhere (card, badge, minimap, details).
pub(super) fn node_tone(node: &Node) -> Hsla {
    look::tone(node.color, kind_color(node.kind))
}

/// A node's badge icon: its `icon`, else its kind's.
pub(super) fn node_icon(node: &Node) -> IconName {
    node.icon
        .and_then(look::icon)
        .unwrap_or_else(|| kind_icon(node.kind))
}

pub(super) fn status_color(status: Status) -> Hsla {
    match status {
        Status::Planned => ui::info(),
        Status::Existing => ui::success(),
        Status::Deprecated => ui::faint(),
        Status::Risk => ui::danger(),
    }
}

pub(super) fn style_name(style: EdgeStyle) -> &'static str {
    match style {
        EdgeStyle::Sync => crate::shared::i18n::t("sync call"),
        EdgeStyle::Async => crate::shared::i18n::t("async"),
        EdgeStyle::Data => crate::shared::i18n::t("data flow"),
        EdgeStyle::Dependency => crate::shared::i18n::t("dependency"),
    }
}

/// Where nodes and edges show this frame: the layout, or on their way to
/// it while the agent's update eases in.
struct Shown {
    nodes: Vec<(Rect, f32)>,
    edges: Vec<(layout::EdgePath, f32)>,
    /// Each edge's label center (None: no label), moving with its edge.
    labels: Vec<Option<layout::Point>>,
    /// Removed by the update, fading out: (index into the arrival's lists).
    leaving: f32,
}

impl Render for DiagramView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(fields) = &mut self.fields {
            super::sync_placeholders(
                fields
                    .tech
                    .iter()
                    .map(|input| (input, super::diagram::TECH_PLACEHOLDER))
                    .chain([(&fields.description, "Description")]),
                &mut fields.language,
                window,
                cx,
            );
        }
        if let Some(note) = &self.note {
            crate::shared::i18n::sync_placeholder(
                note,
                super::NOTE_PLACEHOLDER,
                &mut self.note_language,
                window,
                cx,
            );
        }
        let rem = window.rem_size();
        let now = Instant::now();
        let mut tweening = self.advance_view(now);
        let size = self.bounds.get().size;
        let refit = self.viewport.fitted_for.is_none()
            || (!self.viewport.touched
                && self.view_tween.is_none()
                && self.viewport.fitted_for != Some(size));
        if refit && size.width > px(1.) && size.height > px(1.) {
            self.fit(READABLE_ZOOM, rem);
        }
        if self.arrival.as_ref().is_some_and(|a| a.done(now)) {
            self.arrival = None;
        }
        tweening |= self.arrival.is_some();
        self.sync_fields(window, cx);
        let shown = self.shown(now);

        // Ask for the next frame only while something moves.
        let reduce = cx.reduce_motion();
        let motion_now = backdrop::motion_now(cx);
        let active = window.is_window_active();
        let marches = motion::may_move(active, reduce, motion_now);
        let visible = self.visible(rem);
        let marching_on_screen = self
            .working
            .edges
            .iter()
            .zip(&shown.edges)
            .any(|(e, (p, _))| paint::animated(e.style) && p.bounds().intersects(&visible));
        if let Some(delay) =
            motion::frame_delay(tweening, marching_on_screen, active, reduce, motion_now)
        {
            ui::request_frame_in(delay, window, cx);
        }
        let march = marches.then(|| ui::live_clock() * MARCH_SPEED);

        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .size_full()
            .rounded_xl()
            .bg(ui::surface())
            .border_1()
            .border_color(ui::border())
            .overflow_hidden()
            .child(self.render_header(cx))
            .child(self.render_canvas(rem, &shown, march, cx))
            .child(self.render_footer(cx))
    }
}

impl DiagramView {
    fn shown(&self, now: Instant) -> Shown {
        let Some(arrival) = &self.arrival else {
            return Shown {
                nodes: self.layout.nodes.iter().map(|r| (*r, 1.)).collect(),
                edges: self.layout.edges.iter().map(|p| (p.clone(), 1.)).collect(),
                labels: self.layout.labels.clone(),
                leaving: 0.,
            };
        };
        let p = arrival.progress(now);
        Shown {
            nodes: self
                .working
                .nodes
                .iter()
                .zip(&self.layout.nodes)
                .map(|(n, r)| arrival.node(&n.id, *r, p))
                .collect(),
            edges: self
                .working
                .edges
                .iter()
                .zip(&self.layout.edges)
                .map(|(e, path)| arrival.edge(&e.id, path, p))
                .collect(),
            labels: self
                .working
                .edges
                .iter()
                .zip(&self.layout.labels)
                .map(|(e, label)| arrival.label(&e.id, *label, p))
                .collect(),
            leaving: 1. - p,
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = |id: &'static str, icon: IconName, tip: &'static str| {
            Button::new(id)
                .debug_selector(|| id.into())
                .ghost()
                .xsmall()
                .text_color(ui::muted())
                .icon(Icon::new(icon))
                .tooltip(crate::shared::i18n::t(tip))
        };
        let nodes = self.working.nodes.len();
        let edges = self.working.edges.len();
        let has_nodes = self
            .selection
            .as_ref()
            .is_some_and(|s| !s.nodes().is_empty());
        let (direction_icon, direction_tip) = match self.working.direction {
            Direction::LeftRight => (
                IconName::ArrowRight,
                "Left to right (switch to top to bottom)",
            ),
            Direction::TopBottom => (
                IconName::ArrowDown,
                "Top to bottom (switch to left to right)",
            ),
        };
        h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .min_w_0()
            .child(Icon::new(IconName::Workflow).small().text_color(ui::info()))
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(rems(13. / 16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui::champagne())
                    .child(if self.title.is_empty() {
                        crate::shared::i18n::t("Diagram").to_string()
                    } else {
                        self.title.clone()
                    }),
            )
            .child(ui::chip(format!("v{}", self.version), ui::info()).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(12.))
                    .text_color(ui::muted())
                    .child(format!(
                        "{} · {}",
                        super::count(nodes, "{count} node", "{count} nodes"),
                        super::count(edges, "{count} edge", "{count} edges")
                    )),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_0p5()
                    .child(
                        tool("genui-add", IconName::Plus, "Add node")
                            .disabled(self.locked)
                            .on_click(cx.listener(|this, _, window, cx| this.add_node(window, cx))),
                    )
                    .child(
                        tool("genui-group", IconName::Group, "Group the selected nodes")
                            .disabled(self.locked || !has_nodes)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.group_selection(window, cx)),
                            ),
                    )
                    .child(
                        tool("genui-direction", direction_icon, direction_tip)
                            .disabled(self.locked)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_direction(cx))),
                    )
                    .child(
                        tool("genui-undo", IconName::Undo2, "Undo")
                            .disabled(!self.can_undo())
                            .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
                    )
                    .child(
                        tool("genui-redo", IconName::Redo2, "Redo")
                            .disabled(!self.can_redo())
                            .on_click(cx.listener(|this, _, _, cx| this.redo(cx))),
                    )
                    .child(super::share::tools(
                        &self.env,
                        "diagram",
                        DiagramView::source,
                        cx,
                    ))
                    .child(
                        tool(
                            "genui-expand",
                            if self.expanded {
                                IconName::Minimize2
                            } else {
                                IconName::Maximize2
                            },
                            if self.expanded {
                                "Back to the conversation"
                            } else {
                                "Expand"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.emit(if this.expanded {
                                DiagramEvent::Collapse
                            } else {
                                DiagramEvent::Expand
                            })
                        })),
                    ),
            )
    }

    fn render_canvas(
        &self,
        rem: Pixels,
        shown: &Shown,
        march: Option<f32>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let s = self.scale(rem);
        let pan = self.viewport.pan;
        let visible = self.visible(rem).inflate(40.);
        let at = |x: f32, y: f32| (px(x * s + pan.x), px(y * s + pan.y));
        let selected_edge = match &self.selection {
            Some(Selection::Edge(id)) => self.working.edges.iter().position(|e| &e.id == id),
            _ => None,
        };
        let selected_group = match &self.selection {
            Some(Selection::Group(id)) => self.working.groups.iter().position(|g| &g.id == id),
            _ => None,
        };
        let connect = match &self.drag {
            Some(Drag::Connect { node, to, .. }) => self
                .working
                .node_index(node)
                .map(|ix| (self.layout.nodes[ix].center(), *to)),
            _ => None,
        };
        let select = match &self.drag {
            Some(Drag::Select { from, to }) => Some(Rect {
                x: from.x.min(to.x),
                y: from.y.min(to.y),
                w: (to.x - from.x).abs(),
                h: (to.y - from.y).abs(),
            }),
            _ => None,
        };
        // Group boxes follow their members while they ease in, in their tone.
        let groups: Vec<(Rect, Hsla, bool)> = self
            .working
            .groups
            .iter()
            .enumerate()
            .filter_map(|(gi, g)| {
                let target = self.layout.groups.get(gi).copied().flatten()?;
                let members = self
                    .working
                    .nodes
                    .iter()
                    .zip(&shown.nodes)
                    .filter(|(n, _)| n.group.as_deref() == Some(g.id.as_str()))
                    .map(|(_, (r, _))| *r)
                    .reduce(|a, b| a.union(&b));
                let rect = match (self.arrival.is_some(), members) {
                    (true, Some(union)) => {
                        let mut r = union.inflate(GROUP_PAD);
                        r.y -= layout::GROUP_HEADER;
                        r.h += layout::GROUP_HEADER;
                        r
                    }
                    _ => target,
                };
                let tone = look::tone(g.color, ui::muted());
                Some((rect, tone, selected_group == Some(gi)))
            })
            .collect();
        let group_rects: Vec<Option<Rect>> = {
            let mut it = groups.iter();
            self.layout
                .groups
                .iter()
                .map(|g| g.and_then(|_| it.next().map(|(r, _, _)| *r)))
                .collect()
        };
        let mut edges: Vec<EdgeDraw> = self
            .arrival
            .iter()
            .flat_map(|a| a.leaving_edges.iter())
            .map(|(path, style, color)| EdgeDraw {
                path: path.clone(),
                style: *style,
                tone: color.map(ui::tint),
                alpha: shown.leaving,
                look: EdgeLook::Normal,
            })
            .collect();
        edges.extend(self.working.edges.iter().zip(&shown.edges).enumerate().map(
            |(ix, (e, (path, alpha)))| EdgeDraw {
                path: path.clone(),
                style: e.style,
                tone: e.color.map(ui::tint),
                alpha: *alpha,
                look: if selected_edge == Some(ix) {
                    EdgeLook::Selected
                } else if self.hover_edge == Some(ix) {
                    EdgeLook::Hovered
                } else {
                    EdgeLook::Normal
                },
            },
        ));
        let frame = Frame {
            groups,
            edges,
            scale: s,
            pan,
            march,
            connect,
            select,
        };
        let cell = self.bounds.clone();
        let painter = canvas(
            move |bounds, window, _| {
                let old = cell.get();
                cell.set(bounds);
                // The fit and the visible edges were worked out for the
                // last size: render again for this one on the next frame
                // (notifying from prepaint would draw this frame stale).
                if (old.size.width - bounds.size.width).abs() > px(1.)
                    || (old.size.height - bounds.size.height).abs() > px(1.)
                {
                    ui::request_animation_frame(window);
                }
            },
            move |bounds, _, window, _| frame.paint(bounds, window),
        )
        .absolute()
        .size_full();

        let mut body = div()
            .id("genui-canvas")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .bg(ui::well())
            .cursor(match self.drag {
                Some(Drag::Pan { .. }) => CursorStyle::ClosedHand,
                Some(Drag::Connect { .. } | Drag::Select { .. }) => CursorStyle::Crosshair,
                _ if self.hover_edge.is_some() => CursorStyle::PointingHand,
                _ => CursorStyle::OpenHand,
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.press_background(event, window, cx)
                }),
            )
            .on_mouse_move(cx.listener(Self::pointer_moved))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.released(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.released(cx)),
            )
            .on_scroll(cx.listener(Self::scrolled))
            .on_pinch(cx.listener(Self::pinched))
            .child(painter);

        // Group titles: a tag in the group's tone inside the box's top-left
        // padding (the layout reserves `GROUP_HEADER` for it). Press to
        // select and drag the group, double-click to rename.
        for (gi, (group, rect)) in self.working.groups.iter().zip(&group_rects).enumerate() {
            let Some(rect) = rect.filter(|r| r.intersects(&visible)) else {
                continue;
            };
            let (x, y) = at(rect.x + GROUP_TAG_INSET, rect.y + GROUP_TAG_INSET);
            let id = group.id.clone();
            let tone = if selected_group == Some(gi) {
                ui::accent()
            } else {
                look::tone(group.color, ui::muted())
            };
            body = body.child(
                look::group_tag(tone, &group.label, None, s)
                    .id(("genui-group", gi))
                    .absolute()
                    .left(x)
                    .top(y)
                    .max_w(px((rect.w - 2. * GROUP_TAG_INSET) * s))
                    .truncate()
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |d| d.border_color(tone))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.press_group(&id, event, window, cx)
                        }),
                    ),
            );
        }

        // Edge labels: the phrase on its run, up to two lines, in a pill on
        // the canvas color so the line doesn't run through the text; the
        // style (async, data flow, dependency) under it in `ui::mono()`. The layout
        // placed each in a box of its estimated size, clear of the others.
        if s >= DETAIL_ZOOM {
            for (ix, ((edge, (_, alpha)), spot)) in self
                .working
                .edges
                .iter()
                .zip(&shown.edges)
                .zip(&shown.labels)
                .enumerate()
            {
                let (Some(label), Some(p)) = (edge.label.as_ref().filter(|l| !l.is_empty()), *spot)
                else {
                    continue;
                };
                if !visible.contains(p) {
                    continue;
                }
                let (width, height) = layout::label_size(label, edge.style != EdgeStyle::Sync);
                let (x, y) = at(p.x - width / 2., p.y - height / 2.);
                let highlighted = selected_edge == Some(ix) || self.hover_edge == Some(ix);
                let style =
                    (edge.style != EdgeStyle::Sync && s >= 0.5).then(|| style_name(edge.style));
                body = body.child(
                    v_flex()
                        .absolute()
                        .left(x)
                        .top(y)
                        .w(px(width * s))
                        .h(px(height * s))
                        .items_center()
                        .justify_center()
                        .opacity(*alpha)
                        .child(
                            look::label_pill(s)
                                .flex()
                                .flex_col()
                                .max_w_full()
                                .items_center()
                                .when(highlighted, |d| d.text_color(ui::accent()))
                                .child(
                                    div()
                                        .max_w_full()
                                        .overflow_hidden()
                                        .line_clamp(2)
                                        .text_center()
                                        .child(label.clone()),
                                )
                                .when_some(style, |d, style| {
                                    d.child(
                                        div()
                                            .font_family(ui::mono())
                                            .text_size(px(9. * s * ui::code_scale()))
                                            .line_height(px(12. * s * ui::code_scale()))
                                            .text_color(ui::faint())
                                            .child(style),
                                    )
                                }),
                        ),
                );
            }
        }

        // Nodes the update removed, fading out where they were.
        if let Some(arrival) = &self.arrival {
            for (node, rect) in &arrival.leaving_nodes {
                if !rect.intersects(&visible) {
                    continue;
                }
                let (x, y) = at(rect.x, rect.y);
                body = body.child(
                    node_card(node, *rect, s, shown.leaving)
                        .absolute()
                        .left(x)
                        .top(y),
                );
            }
        }

        let selected_nodes = self.selection.as_ref().map(|s| s.nodes()).unwrap_or(&[]);
        for (ix, node) in self.working.nodes.iter().enumerate() {
            let (rect, alpha) = shown.nodes[ix];
            if !rect.intersects(&visible) {
                continue;
            }
            let (x, y) = at(rect.x, rect.y);
            let selected = selected_nodes.contains(&node.id);
            body = body.child(self.render_node(ix, x, y, rect, alpha, s, selected, cx));
        }

        if let Some(rename) = &self.rename {
            let place = match &rename.target {
                Selection::Node(id) => self.working.node_index(id).map(|ix| self.layout.nodes[ix]),
                Selection::Edge(id) => {
                    self.working
                        .edges
                        .iter()
                        .position(|e| &e.id == id)
                        .map(|ix| {
                            let p = self.layout.labels[ix]
                                .unwrap_or_else(|| self.layout.edges[ix].label_at());
                            Rect {
                                x: p.x - layout::NODE_W / 2.,
                                y: p.y - 14.,
                                w: layout::NODE_W,
                                h: 28.,
                            }
                        })
                }
                Selection::Group(id) => self
                    .working
                    .groups
                    .iter()
                    .position(|g| &g.id == id)
                    .and_then(|gi| self.layout.groups[gi])
                    .map(|r| Rect {
                        x: r.x + GROUP_PAD / 2.,
                        y: r.y,
                        w: layout::NODE_W,
                        h: layout::GROUP_HEADER,
                    }),
                Selection::Nodes(_) => None,
            };
            if let Some(rect) = place {
                let (x, y) = at(rect.x, rect.y + rect.h / 2.);
                body = body.child(
                    div()
                        .absolute()
                        .left(x)
                        .top(y - rems(15. / 16.).to_pixels(rem))
                        .w(px(rect.w * s).max(rems(160. / 16.).to_pixels(rem)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            ui::field()
                                .h(rems(30. / 16.))
                                .bg(ui::popover_bg())
                                .border_color(ui::accent().opacity(0.6))
                                .child(Input::new(&rename.input).appearance(false).small()),
                        ),
                );
            }
        }

        if self.working.nodes.is_empty() {
            body =
                body.child(
                    v_flex()
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(Icon::new(IconName::Workflow).text_color(ui::faint()))
                        .child(div().text_sm().text_color(ui::muted()).child(
                            crate::shared::i18n::t("Nothing drawn yet. Add a node with +."),
                        )),
                );
        }
        if let Some(error) = &self.error {
            body = body.child(
                div().absolute().left_2().right_2().top_2().child(
                    ui::card()
                        .px_3()
                        .py_2()
                        .gap_1()
                        .bg(ui::popover_bg())
                        .border_color(ui::danger().opacity(0.35))
                        .child(
                            h_flex()
                                .gap_2()
                                .text_size(rems(12. / 16.))
                                .text_color(ui::danger())
                                .child(Icon::new(IconName::CircleAlert).xsmall())
                                .child(crate::shared::i18n::t(
                                    "The agent's latest version can't be drawn",
                                )),
                        )
                        .child(
                            div()
                                .font_family(ui::mono())
                                .text_size(ui::code_rems(11.))
                                .text_color(ui::muted())
                                .whitespace_normal()
                                .child(error.clone()),
                        ),
                ),
            );
        }
        body.child(self.render_controls(cx))
            .children(self.render_minimap(rem, shown, cx))
            .children(self.render_details(cx))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_node(
        &self,
        ix: usize,
        x: Pixels,
        y: Pixels,
        rect: Rect,
        alpha: f32,
        s: f32,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let node = &self.working.nodes[ix];
        let id = node.id.clone();
        let dragging = selected && matches!(&self.drag, Some(Drag::Nodes { moved: true, .. }));
        let lr = self.working.direction == Direction::LeftRight;
        let handle = |outgoing: bool| {
            let id = node.id.clone();
            let size = 10. * s;
            let along = if lr { rect.h } else { rect.w } * s / 2. - size / 2.;
            div()
                .id((
                    if outgoing {
                        "genui-handle-out"
                    } else {
                        "genui-handle-in"
                    },
                    ix,
                ))
                .absolute()
                .size(px(size))
                .rounded_full()
                .bg(ui::surface())
                .border_2()
                .border_color(ui::accent())
                .cursor(CursorStyle::Crosshair)
                .map(|d| match (lr, outgoing) {
                    (true, true) => d.right(px(-size / 2.)).top(px(along)),
                    (true, false) => d.left(px(-size / 2.)).top(px(along)),
                    (false, true) => d.bottom(px(-size / 2.)).left(px(along)),
                    (false, false) => d.top(px(-size / 2.)).left(px(along)),
                })
                .when(!selected || self.locked, |d| {
                    d.opacity(0.).when(!self.locked, |d| {
                        d.group_hover("genui-node", |d| d.opacity(1.))
                    })
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.press_handle(&id, outgoing, event, window, cx)
                    }),
                )
        };
        let tone = node_tone(node);
        let card = node_card(node, rect, s, alpha)
            .id(("genui-node", ix))
            .group("genui-node")
            .absolute()
            .left(x)
            .top(y)
            .when(selected, |d| d.border_color(ui::accent()))
            .when(dragging, |d| d.shadow_lg())
            .cursor(if self.locked {
                CursorStyle::PointingHand
            } else {
                CursorStyle::OpenHand
            })
            .hover(move |d| d.bg(look::hover_fill(tone)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.press_node(&id, event, window, cx)
                }),
            )
            // The selection ring fades in around the card.
            .when(selected, |d| {
                d.child(ui::popover_in(
                    ("genui-ring", ix),
                    div()
                        .absolute()
                        .inset(px(-4. * s))
                        .rounded(px((look::CARD_RADIUS + 4.) * s))
                        .border_2()
                        .border_color(ui::accent().opacity(0.35)),
                ))
            })
            .when_some(node.code_ref.clone().filter(|_| s >= 0.5), |d, code_ref| {
                let selector = format!("genui-node-ref-{}", node.id);
                let found = matches!(
                    self.env.ref_state(&code_ref),
                    super::share::RefState::Found(_) | super::share::RefState::Unknown
                );
                d.child(
                    div()
                        .id(("genui-node-ref", ix))
                        .debug_selector(move || selector)
                        .absolute()
                        .top(px(4. * s))
                        .right(px(4. * s))
                        .size(px(18. * s))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5. * s))
                        .cursor_pointer()
                        .text_color(if found { ui::accent() } else { ui::faint() })
                        .hover(|d| d.bg(ui::surface_hover()))
                        .child(
                            Icon::new(if found {
                                IconName::FileCode
                            } else {
                                IconName::FileX
                            })
                            .size(px(12. * s)),
                        )
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(DiagramEvent::OpenRef(code_ref.clone()));
                        })),
                )
            })
            .child(handle(false))
            .child(handle(true));
        card.into_any_element()
    }

    /// React Flow's controls: zoom, fit and lock, floating bottom-left.
    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let control = |id: &'static str, icon: IconName, tip: &'static str| {
            Button::new(id)
                .debug_selector(|| id.into())
                .ghost()
                .xsmall()
                .text_color(ui::muted())
                .icon(Icon::new(icon))
                .tooltip(crate::shared::i18n::t(tip))
        };
        v_flex()
            .absolute()
            .left_2()
            .bottom_2()
            .p_0p5()
            .gap_0p5()
            .rounded_lg()
            .bg(ui::popover_bg())
            .border_1()
            .border_color(ui::border())
            .shadow_sm()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                control("genui-zoom-in", IconName::ZoomIn, "Zoom in").on_click(cx.listener(
                    |this, _, window, cx| {
                        this.zoom_center(ZOOM_STEP, window.rem_size(), cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                control("genui-zoom-out", IconName::ZoomOut, "Zoom out").on_click(cx.listener(
                    |this, _, window, cx| {
                        this.zoom_center(1. / ZOOM_STEP, window.rem_size(), cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                control("genui-fit", IconName::Scan, "Fit the whole diagram").on_click(
                    cx.listener(|this, _, window, cx| {
                        this.fit_animated(window.rem_size(), cx);
                        cx.notify();
                    }),
                ),
            )
            .child(
                control(
                    "genui-lock",
                    if self.locked {
                        IconName::Lock
                    } else {
                        IconName::LockOpen
                    },
                    if self.locked {
                        "Unlock editing"
                    } else {
                        "Lock: view only"
                    },
                )
                .when(self.locked, |b| b.text_color(ui::accent()))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_lock(cx))),
            )
    }

    /// The minimap, bottom-right: press or drag to move the view there.
    fn render_minimap(
        &self,
        rem: Pixels,
        shown: &Shown,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.working.nodes.is_empty() {
            return None;
        }
        let view = self.visible(rem);
        let map = MiniMap {
            nodes: self
                .working
                .nodes
                .iter()
                .zip(&shown.nodes)
                .map(|(n, (r, _))| (*r, node_tone(n).opacity(0.7)))
                .collect(),
            content: self.layout.bounds.union(&view),
            view,
        };
        let cell = self.minimap.clone();
        Some(
            div()
                .id("genui-minimap")
                .absolute()
                .right_2()
                .bottom_2()
                .w(rems(MINIMAP_W / 16.))
                .h(rems(MINIMAP_H / 16.))
                .rounded_lg()
                .bg(ui::popover_bg())
                .border_1()
                .border_color(ui::border())
                .shadow_sm()
                .overflow_hidden()
                .cursor(CursorStyle::PointingHand)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        this.press_minimap(event, window, cx)
                    }),
                )
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(
                    canvas(
                        move |bounds, _, _| cell.set(bounds),
                        move |bounds, _, window, _| map.paint(bounds, window),
                    )
                    .size_full(),
                )
                .into_any_element(),
        )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let footer = h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .min_h(rems(38. / 16.))
            .border_t_1()
            .border_color(ui::border())
            .text_size(rems(12. / 16.));
        if let Some(refused) = &self.refused {
            return footer
                .text_color(ui::danger())
                .child(Icon::new(IconName::CircleAlert).xsmall())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(super::diagnostic::render(refused)),
                );
        }
        if self.pending > 0 {
            let sending = matches!(self.send, SendState::Sending { .. });
            let failed = match &self.send {
                SendState::Failed(error) => Some(error.clone()),
                _ => None,
            };
            let retry = failed.is_some();
            return footer
                .child(ui::chip(
                    super::count(self.pending, "{count} change", "{count} changes"),
                    ui::accent(),
                ))
                .child(match (&failed, &self.note) {
                    (Some(error), _) => h_flex()
                        .id("genui-send-error")
                        .debug_selector(|| "genui-send-error".into())
                        .flex_1()
                        .min_w_0()
                        .gap_1p5()
                        .text_color(ui::danger())
                        .child(Icon::new(IconName::CircleAlert).xsmall().flex_shrink_0())
                        .child(div().min_w_0().truncate().child(error.clone()))
                        .tooltip({
                            let error = error.clone();
                            move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(error.clone())
                                    .build(window, cx)
                            }
                        })
                        .into_any_element(),
                    (None, Some(note)) => ui::field()
                        .flex_1()
                        .min_w_0()
                        .h(rems(26. / 16.))
                        .child(Input::new(note).appearance(false).xsmall())
                        .into_any_element(),
                    (None, None) => div()
                        .id("genui-add-note")
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .cursor_pointer()
                        .text_color(ui::faint())
                        .hover(|d| d.text_color(ui::muted()))
                        .child(crate::shared::i18n::t("Add a note…"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.note_input(window, cx);
                            if let Some(note) = &this.note {
                                note.update(cx, |state, cx| state.focus(window, cx));
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                })
                .child(
                    Button::new("genui-discard")
                        .debug_selector(|| "genui-discard".into())
                        .ghost()
                        .xsmall()
                        .label(crate::shared::i18n::t("Discard"))
                        .disabled(sending)
                        .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
                )
                .child(
                    Button::new("genui-send")
                        .debug_selector(|| "genui-send".into())
                        .primary()
                        .xsmall()
                        .icon(Icon::new(if retry {
                            IconName::RefreshCw
                        } else {
                            IconName::Send
                        }))
                        .label(crate::shared::i18n::t(if retry {
                            "Retry"
                        } else {
                            "Send to agent"
                        }))
                        .loading(sending)
                        .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
                );
        }
        match &self.send {
            SendState::Sent => footer
                .text_color(ui::success())
                .child(Icon::new(IconName::CircleCheck).xsmall())
                .child(crate::shared::i18n::t("Sent to the agent")),
            _ => footer.text_color(ui::faint()).child(
                div().flex_1().min_w_0().truncate().child(if self.locked {
                    crate::shared::i18n::t("View only · unlock to edit")
                } else {
                    crate::shared::i18n::t("Drag to move · double-click to rename · drag ● to connect · shift-drag to select · ⌘/Ctrl-scroll to zoom")
                }),
            ),
        }
    }
}

/// A node's card, read top to bottom: its icon badge and title, what the
/// node does (its description, at most two lines) and `KIND · tech` in
/// `ui::mono()`. The card wears the node's tone (`node_tone`): tinted fill, a
/// hairline and the solid badge. Notes and external systems sit flat (no
/// shadow), a note's description in italics. Status changes the hairline
/// (risk), dashes it (planned) or dims the card (deprecated), and puts a dot
/// by the title. `alpha` is the card's fade (easing in or out).
fn node_card(node: &Node, rect: Rect, s: f32, alpha: f32) -> Div {
    let kind = node.kind;
    let note = kind == NodeKind::Note;
    let flat = note || kind == NodeKind::External;
    let tone = node_tone(node);
    let meta = match node.tech.as_deref().filter(|t| !t.is_empty()) {
        Some(tech) => format!(
            "{} · {tech}",
            crate::shared::i18n::t(kind.name()).to_uppercase()
        ),
        None => crate::shared::i18n::t(kind.name()).to_uppercase(),
    };
    let description = node
        .description
        .clone()
        .filter(|d| !d.trim().is_empty() && s >= DETAIL_ZOOM);
    let dim = if node.status == Some(Status::Deprecated) {
        0.55
    } else {
        1.
    };
    let header = h_flex()
        .flex_shrink_0()
        .w_full()
        .min_w_0()
        .gap(px(8. * s))
        .child(look::badge(tone, node_icon(node), look::BADGE, s))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13. * s))
                .line_height(px(16. * s))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::fg())
                .child(node.label.clone()),
        )
        .when_some(node.status.filter(|_| s >= 0.6), |d, status| {
            d.child(
                div()
                    .flex_shrink_0()
                    .size(px(7. * s))
                    .rounded_full()
                    .bg(status_color(status)),
            )
        });
    // 88 units tall: 8 + badge 26 + 4 + two 13-unit lines + 4 + meta 12 + 8.
    look::card(tone, s)
        .relative()
        .flex()
        .flex_col()
        .w(px(rect.w * s))
        .h(px(rect.h * s))
        .px(px(12. * s))
        .py(px(8. * s))
        .gap(px(4. * s))
        .when(flat, |d| d.shadow(Vec::new()))
        .when(node.status == Some(Status::Risk), |d| {
            d.border_color(ui::danger().opacity(0.7))
        })
        .when(node.status == Some(Status::Planned), |d| d.border_dashed())
        .opacity(alpha * dim)
        .child(header)
        .when_some(description, |d, text| {
            d.child(
                div()
                    .flex_shrink_0()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .line_clamp(2)
                    .when(note, |d| d.italic())
                    .text_size(px(11. * s))
                    .line_height(px(13. * s))
                    .text_color(ui::muted())
                    .child(text),
            )
        })
        .when(s >= DETAIL_ZOOM, |d| {
            d.child(
                div()
                    .mt_auto()
                    .flex_shrink_0()
                    .w_full()
                    .truncate()
                    .font_family(ui::mono())
                    .text_size(px(9.5 * s * ui::code_scale()))
                    .line_height(px(12. * s * ui::code_scale()))
                    .text_color(ui::faint())
                    .child(meta),
            )
        })
}

// ── Conversation rows ──────────────────────────────────────────────────────

fn row(icon: IconName, color: Hsla, title: String, note: Option<String>) -> Div {
    ui::card()
        .flex_row()
        .w_full()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .child(Icon::new(icon).small().text_color(color))
        .child(
            div()
                .min_w_0()
                .flex_shrink(1.)
                .truncate()
                .text_size(rems(13. / 16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ui::champagne())
                .child(title),
        )
        .children(note.map(|note| {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(rems(12. / 16.))
                .text_color(ui::muted())
                .child(note)
        }))
}

/// The agent is still sending the surface.
pub(super) fn drawing_card(title: &str) -> AnyElement {
    row(
        IconName::Workflow,
        ui::info(),
        title.to_string(),
        Some("Drawing…".into()),
    )
    .child(ui::spinner(Size::XSmall, ui::info()))
    .into_any_element()
}

/// The tool call failed: the extension's message (what the agent saw).
pub(super) fn failed_card(title: &str, error: &str) -> AnyElement {
    ui::card()
        .w_full()
        .border_color(ui::danger().opacity(0.35))
        .child(
            row(IconName::Workflow, ui::danger(), title.to_string(), None)
                .border_0()
                .bg(transparent_black()),
        )
        .child(
            div()
                .px_3()
                .pb_2()
                .font_family(ui::mono())
                .text_size(ui::code_rems(12.))
                .text_color(ui::muted())
                .whitespace_normal()
                .child(error.to_string()),
        )
        .into_any_element()
}

/// An older version of a surface shown again further down.
pub(super) fn superseded_row(title: &str, version: u64, closed: bool) -> AnyElement {
    row(
        IconName::Workflow,
        ui::faint(),
        title.to_string(),
        Some(if closed {
            crate::shared::i18n::t("closed by the agent").into()
        } else {
            crate::shared::i18n::t("v{version} · updated below")
                .replace("{version}", &version.to_string())
        }),
    )
    .opacity(0.7)
    .into_any_element()
}

/// The surface is open in place of the conversation.
pub(super) fn expanded_row(surface: super::Surface, title: &str) -> AnyElement {
    row(
        IconName::Workflow,
        ui::info(),
        title.to_string(),
        Some(crate::shared::i18n::t("open in the expanded view").into()),
    )
    .child(
        Button::new("genui-collapse")
            .ghost()
            .xsmall()
            .icon(Icon::new(IconName::Minimize2))
            .label(crate::shared::i18n::t("Back here"))
            .on_click(move |_, _, cx| surface.emit(DiagramEvent::Collapse, cx)),
    )
    .into_any_element()
}

/// The surface is pinned to the conversation's side panel.
pub(super) fn pinned_row(surface: super::Surface, title: &str) -> AnyElement {
    row(
        IconName::Pin,
        ui::info(),
        title.to_string(),
        Some(crate::shared::i18n::t("pinned to the side panel").into()),
    )
    .child(
        Button::new("genui-unpin-here")
            .debug_selector(|| "genui-unpin-here".into())
            .ghost()
            .xsmall()
            .icon(Icon::new(IconName::PinOff))
            .label(crate::shared::i18n::t("Back here"))
            .on_click(move |_, _, cx| surface.emit(DiagramEvent::Pin(false), cx)),
    )
    .into_any_element()
}

/// The user's sent edits (the extension's `gentle-ui.event` message), a
/// template they inserted (once its surface is gone) or their request to
/// add a surface to the docs.
pub(super) fn event_card(details: &serde_json::Value) -> AnyElement {
    let title = details["title"]
        .as_str()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| crate::shared::i18n::t("Diagram"));
    if details["request"] == "docs" {
        return row(
            IconName::BookOpen,
            ui::accent(),
            crate::shared::i18n::t("You asked to add {title} to the docs")
                .replace("{title}", title),
            details["format"]
                .as_str()
                .map(|f| crate::shared::i18n::t("as {format}").replace("{format}", f)),
        )
        .into_any_element();
    }
    let summary: Vec<&str> = details["summary"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str())
        .collect();
    const SHOWN: usize = 6;
    let more = summary.len().saturating_sub(SHOWN);
    let note = details["note"].as_str().filter(|n| !n.trim().is_empty());
    ui::card()
        .w_full()
        .child(
            row(
                IconName::Pencil,
                ui::accent(),
                match details["kind"].as_str() {
                    _ if details["inserted"] == true => {
                        crate::shared::i18n::t("You inserted {title} from a template")
                            .replace("{title}", title)
                    }
                    Some("html") => {
                        crate::shared::i18n::t("You, from {title}").replace("{title}", title)
                    }
                    Some("review") => {
                        crate::shared::i18n::t("You reviewed {title}").replace("{title}", title)
                    }
                    _ => crate::shared::i18n::t("You edited {title}").replace("{title}", title),
                },
                None,
            )
            .border_0()
            .bg(transparent_black())
            .child(div().flex_1())
            .when_some(details["version"].as_u64(), |d, v| {
                d.child(ui::chip(format!("v{v}"), ui::info()))
            }),
        )
        .child(
            v_flex()
                .px_3()
                .pb_2()
                .gap_0p5()
                .text_size(rems(12. / 16.))
                .text_color(ui::muted())
                .children(summary.iter().take(SHOWN).map(|line| {
                    h_flex()
                        .gap_1p5()
                        .child(div().text_color(ui::faint()).child("•"))
                        .child(div().min_w_0().whitespace_normal().child(line.to_string()))
                }))
                .when(more > 0, |d| {
                    d.child(
                        div().text_color(ui::faint()).child(
                            crate::shared::i18n::t("… {count} more")
                                .replace("{count}", &more.to_string()),
                        ),
                    )
                })
                .when_some(note, |d, note| {
                    d.child(
                        div()
                            .pt_1()
                            .text_color(ui::fg())
                            .whitespace_normal()
                            .child(format!("“{note}”")),
                    )
                }),
        )
        .into_any_element()
}
