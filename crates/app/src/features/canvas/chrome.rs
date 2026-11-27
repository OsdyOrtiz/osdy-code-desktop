//! The page element and what floats over it, in Tern's layout: the tool
//! bar at the bottom, the zoom pill bottom-left (undo, redo, zoom, fit,
//! minimap, more), the style panel top-right (stroke and fill colors, fill
//! style, opacity, pen type, size) and the minimap.

use std::path::PathBuf;

use gentle_core::canvas::{Color, Controls, Fill, Kind, OPACITIES, Pen, Rect, SIZES, Shape};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;

use super::paint::{self, Affine, Chrome, Item};
use super::pointer::{ERASER, Gesture};
use super::*;
use crate::shared::ui;

const MINIMAP_W: f32 = 180.;
const MINIMAP_H: f32 = 120.;
/// The tool bar's and zoom pill's widths (px), to tell when they'd meet.
const TOOLBAR_W: f32 = 540.;
const PILL_W: f32 = 350.;

/// Shapes drawn as elements rather than paths.
fn is_element(shape: &Shape) -> bool {
    matches!(
        shape.kind,
        Kind::Text { .. } | Kind::Note { .. } | Kind::Image { .. }
    )
}

/// Black or white text, whichever reads on `bg`.
fn on(bg: Color) -> Hsla {
    let c = bg.rgb(paint::light());
    let (r, g, b) = ((c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff);
    let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if lum > 150. {
        rgb(0x1d1f23).into()
    } else {
        rgb(0xf4f4f5).into()
    }
}

impl CanvasView {
    pub(super) fn render_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let camera = self.doc.camera;
        let cam = Affine::camera(&camera);
        let (w, h) = self.size();
        let view = camera
            .visible(w.max(1.), h.max(1.))
            .expand(64. / camera.zoom);

        // What the gesture does to the selection while it lasts.
        let mut moving = None;
        let mut resizing = None;
        let mut erasing: &[u64] = &[];
        match &self.gesture {
            Some(Gesture::Move {
                delta, moved: true, ..
            }) => moving = Some(*delta),
            Some(Gesture::Resize { from, to, .. }) => resizing = Some((*from, *to)),
            Some(Gesture::Erase { hits, .. }) => erasing = hits,
            _ => {}
        }
        let editing = self.editing.as_ref().and_then(|e| match e.target {
            EditTarget::Shape(id) => Some(id),
            EditTarget::New(_) => None,
        });
        let placed = |id: u64| -> Affine {
            if self.selected.contains(&id) {
                if let Some(d) = moving {
                    return cam.after_move(d);
                }
                if let Some((from, to)) = &resizing {
                    return cam.after_resize(from, to);
                }
            }
            cam
        };

        let mut layers: Vec<AnyElement> = Vec::new();
        let cam_for_grid = camera;
        layers.push(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint::grid(bounds, &cam_for_grid, window),
            )
            .absolute()
            .size_full()
            .into_any_element(),
        );
        {
            let mut items: Vec<Item> = Vec::new();
            let flush = |items: &mut Vec<Item>, layers: &mut Vec<AnyElement>| {
                if items.is_empty() {
                    return;
                }
                let items = std::mem::take(items);
                let cache = self.geo.clone();
                layers.push(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            paint::paint_items(&items, &cache, bounds.origin, window)
                        },
                    )
                    .absolute()
                    .size_full()
                    .into_any_element(),
                );
            };
            for shape in &self.doc.shapes {
                let at = placed(shape.id);
                // Tessellated once, on first sight; its box comes with it.
                let bounds = self
                    .geo
                    .borrow_mut()
                    .entry(shape.id)
                    .or_insert_with(|| paint::build(shape))
                    .bounds;
                if at == cam && !view.intersects(&bounds) {
                    continue;
                }
                let fade = if erasing.contains(&shape.id) {
                    0.25
                } else {
                    1.
                };
                if is_element(shape) {
                    flush(&mut items, &mut layers);
                    layers.push(self.shape_element(shape, at, fade, editing == Some(shape.id)));
                    continue;
                }
                items.push(Item {
                    id: shape.id,
                    style: shape.style,
                    highlighter: matches!(
                        shape.kind,
                        Kind::Ink {
                            highlighter: true,
                            ..
                        }
                    ),
                    at,
                    fade,
                });
                // A label sits on its shape, under whatever comes later.
                if let Kind::Rect { rect, label } | Kind::Ellipse { rect, label } = &shape.kind
                    && !label.is_empty()
                    && editing != Some(shape.id)
                {
                    flush(&mut items, &mut layers);
                    let r = at.rect(rect);
                    let font = shape.style.font_size() * at.sy.abs();
                    layers.push(
                        div()
                            .absolute()
                            .left(px(r.x))
                            .top(px(r.y))
                            .w(px(r.w))
                            .h(px(r.h))
                            .flex()
                            .items_center()
                            .justify_center()
                            .px(px(8. * at.sx.abs()))
                            .overflow_hidden()
                            .text_center()
                            .text_size(px(font))
                            .line_height(px(font * 1.3))
                            .text_color(
                                paint::color(shape.style.color).opacity(shape.style.alpha() * fade),
                            )
                            .child(label.clone())
                            .into_any_element(),
                    );
                }
            }
            flush(&mut items, &mut layers);
        }

        let chrome = self.chrome(cam, moving, resizing);
        let entity = cx.entity();
        let cell = self.bounds.clone();
        let gesturing = self.gesture.is_some();
        layers.push(
            canvas(
                move |bounds, _, _| cell.set(bounds),
                move |bounds, _, window, _| {
                    chrome.paint(bounds, window);
                    if gesturing {
                        // While a gesture lasts, follow the pointer anywhere
                        // in the window, and end it wherever it's released.
                        let e = entity.clone();
                        window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                e.update(cx, |this, cx| this.drag(ev.position, ev.modifiers, cx));
                            }
                        });
                        let e = entity.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                e.update(cx, |this, cx| this.mouse_up(window, cx));
                            }
                        });
                    }
                },
            )
            .absolute()
            .size_full()
            .into_any_element(),
        );
        if !self.laser.is_empty() {
            ui::request_animation_frame(window);
        }

        let cursor = if matches!(self.gesture, Some(Gesture::Pan { .. })) {
            CursorStyle::ClosedHand
        } else if self.space_pan || self.tool == Tool::Hand {
            CursorStyle::OpenHand
        } else {
            match self.tool {
                Tool::Select | Tool::Lasso => CursorStyle::Arrow,
                Tool::Text => CursorStyle::IBeam,
                _ => CursorStyle::Crosshair,
            }
        };

        div()
            .id("canvas-page")
            .key_context(if self.editing.is_some() {
                "GentleCanvasEditing"
            } else {
                CONTEXT
            })
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(ui::panel())
            .cursor(cursor)
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::duplicate))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::delete))
            .on_key_down(cx.listener(Self::key_down))
            .on_key_up(cx.listener(|this, e: &KeyUpEvent, _, cx| this.key_up(e, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| this.hover_move(e, cx)))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered && this.gesture.is_none() {
                    this.hover = None;
                    cx.notify();
                }
            }))
            .on_scroll(cx.listener(Self::scroll))
            .on_pinch(cx.listener(|this, e: &PinchEvent, _, cx| this.pinch(e, cx)))
            .drag_over::<ExternalPaths>(|style, _, _, _| style.bg(ui::surface_hover()))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                let at = this.page(window.mouse_position());
                this.insert_images(paths.paths().to_vec(), Some(at), cx);
            }))
            .children(layers)
            .children(self.editor(cam, cx))
            .child(self.toolbar(cx))
            .child(self.zoom_pill(cx))
            .when(self.shows_style_panel(), |d| d.child(self.style_panel(cx)))
            .when(self.minimap, |d| d.child(self.minimap_card(cam, cx)))
            .when_some(self.broken.clone(), |d, msg| {
                d.child(
                    div()
                        .absolute()
                        .top_3()
                        .left_3()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(ui::popover_bg())
                        .text_sm()
                        .text_color(ui::danger())
                        .child(msg.render()),
                )
            })
    }

    /// Selection boxes, the live stroke and shape, marquee, lasso, eraser
    /// and laser for this frame.
    fn chrome(&mut self, cam: Affine, moving: Option<P>, resizing: Option<(Rect, Rect)>) -> Chrome {
        let mut chrome = Chrome {
            camera: cam,
            ..Chrome::default()
        };
        match &self.gesture {
            Some(Gesture::Ink {
                points,
                highlighter,
            }) => {
                chrome.live = paint::ink_path(points, &self.style, *highlighter, false)
                    .map(|p| (p, self.style, *highlighter));
            }
            Some(Gesture::Create {
                tool,
                start,
                end,
                moved: true,
            }) => {
                let kind = match tool {
                    Tool::Rect => Kind::Rect {
                        rect: Rect::from_corners(*start, *end),
                        label: String::new(),
                    },
                    Tool::Ellipse => Kind::Ellipse {
                        rect: Rect::from_corners(*start, *end),
                        label: String::new(),
                    },
                    Tool::Line => Kind::Line { a: *start, b: *end },
                    _ => Kind::Arrow { a: *start, b: *end },
                };
                chrome.preview = Some((
                    paint::build(&Shape {
                        id: 0,
                        kind,
                        style: self.style,
                    }),
                    self.style,
                ));
            }
            Some(Gesture::Box { start, end, .. }) => {
                chrome.marquee = Some(cam.rect(&Rect::from_corners(*start, *end)));
            }
            Some(Gesture::Lasso { points }) => {
                chrome.lasso = points.iter().map(|p| cam.apply(*p)).collect();
            }
            _ => {}
        }
        if self.editing.is_none() && !self.selected.is_empty() {
            let at = match (moving, resizing) {
                (Some(d), _) => cam.after_move(d),
                (_, Some((from, to))) => cam.after_resize(&from, &to),
                _ => cam,
            };
            if self.selected.len() > 1 {
                chrome.boxes = self
                    .doc
                    .shapes
                    .iter()
                    .filter(|s| self.selected.contains(&s.id))
                    .map(|s| at.rect(&s.bounds()))
                    .collect();
            }
            chrome.frame = self.selection_bounds().map(|b| at.rect(&b));
            chrome.handles = !matches!(self.gesture, Some(Gesture::Move { .. }));
        }
        if let Some(hover) = self.hover {
            match self.tool {
                Tool::Eraser => chrome.eraser = Some((hover, ERASER)),
                Tool::Laser if self.gesture.is_none() => chrome.eraser = Some((hover, 3.)),
                _ => {}
            }
        }
        chrome.laser = self
            .laser_trail()
            .into_iter()
            .map(|(p, age)| (cam.apply(p), age))
            .collect();
        chrome
    }

    /// A text, note or image on the page.
    fn shape_element(&self, shape: &Shape, at: Affine, fade: f32, editing: bool) -> AnyElement {
        let alpha = shape.style.alpha() * fade;
        let zoom = at.sy.abs();
        match &shape.kind {
            Kind::Text { at: pos, text, .. } => {
                let [x, y] = at.apply(*pos);
                let font = shape.font_size() * zoom;
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .when(editing, |d| d.invisible())
                    .flex()
                    .flex_col()
                    .text_size(px(font))
                    .line_height(px(font * 1.3))
                    .text_color(paint::color(shape.style.color).opacity(alpha))
                    .children(text.split('\n').map(|line| {
                        div().whitespace_nowrap().child(if line.is_empty() {
                            " ".to_string()
                        } else {
                            line.to_string()
                        })
                    }))
                    .into_any_element()
            }
            Kind::Note { rect, text } => {
                let r = at.rect(rect);
                let bg = shape.style.color;
                let font = shape.style.font_size() * zoom;
                div()
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .opacity(alpha)
                    .rounded(px(6. * zoom))
                    .bg(paint::color(bg))
                    .shadow(vec![BoxShadow {
                        color: hsla(0., 0., 0., 0.28),
                        offset: point(px(0.), px(4. * zoom)),
                        blur_radius: px(14. * zoom),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .p(px(14. * zoom))
                    .overflow_hidden()
                    .text_size(px(font))
                    .line_height(px(font * 1.3))
                    .text_color(on(bg))
                    .when(!editing, |d| d.child(text.clone()))
                    .into_any_element()
            }
            Kind::Image { rect, path } => {
                let r = at.rect(rect);
                img(PathBuf::from(path))
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .opacity(alpha)
                    .object_fit(ObjectFit::Fill)
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    /// The text field over the text, note or shape label being typed into.
    fn editor(&self, cam: Affine, cx: &mut Context<Self>) -> Option<AnyElement> {
        let edit = self.editing.as_ref()?;
        let (kind, style) = match &edit.target {
            EditTarget::Shape(id) => {
                let s = self.doc.get(*id)?;
                (s.kind.clone(), s.style)
            }
            EditTarget::New(kind) => (kind.clone(), self.style),
        };
        let zoom = cam.sx;
        let lines = edit.input.read(cx).value().split('\n').count().max(1) as f32;
        let field = |font: f32| {
            div()
                .text_size(px(font))
                .line_height(px(font * 1.3))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Textarea::new(&edit.input)
                        .appearance(false)
                        .h(px(font * 1.3 * (lines + 1.))),
                )
        };
        let el = match kind {
            Kind::Text { at, scale, .. } => {
                let [x, y] = cam.apply(at);
                let font = style.font_size() * scale * zoom;
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .min_w(px(160.))
                    .text_color(paint::color(style.color))
                    .child(field(font))
            }
            Kind::Note { rect, .. } => {
                let r = cam.rect(&rect);
                let font = style.font_size() * zoom;
                div()
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .rounded(px(6. * zoom))
                    .bg(paint::color(style.color))
                    .p(px(14. * zoom))
                    .text_color(on(style.color))
                    .child(field(font))
            }
            Kind::Rect { rect, .. } | Kind::Ellipse { rect, .. } => {
                let r = cam.rect(&rect);
                let font = style.font_size() * zoom;
                div()
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .flex()
                    .items_center()
                    .px(px(8. * zoom))
                    .text_color(paint::color(style.color))
                    .child(field(font).w_full())
            }
            _ => return None,
        };
        Some(el.into_any_element())
    }

    // ── Tool bar ───────────────────────────────────────────────────────────

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let geo_icon = if self.geo_tool == Tool::Ellipse {
            IconName::Circle
        } else {
            IconName::Square
        };
        let geo_tip = if self.geo_tool == Tool::Ellipse {
            crate::shared::i18n::t("Ellipse (O) · click again for rectangle")
        } else {
            crate::shared::i18n::t("Rectangle (R) · click again for ellipse")
        };
        let tools: [(Tool, IconName, &'static str); 13] = [
            (
                Tool::Select,
                IconName::MousePointer2,
                crate::shared::i18n::t("Select (V)"),
            ),
            (
                Tool::Lasso,
                IconName::Lasso,
                crate::shared::i18n::t("Lasso (S)"),
            ),
            (
                Tool::Hand,
                IconName::Hand,
                crate::shared::i18n::t("Hand (H, or hold Space)"),
            ),
            (Tool::Pen, IconName::Pen, crate::shared::i18n::t("Pen (P)")),
            (
                Tool::Highlighter,
                IconName::Highlighter,
                crate::shared::i18n::t("Highlighter (M)"),
            ),
            (
                Tool::Eraser,
                IconName::Eraser,
                crate::shared::i18n::t("Eraser (E)"),
            ),
            (
                Tool::Laser,
                IconName::WandSparkles,
                crate::shared::i18n::t("Laser pointer (K)"),
            ),
            (self.geo_tool, geo_icon, geo_tip),
            (
                Tool::Arrow,
                IconName::MoveUpRight,
                crate::shared::i18n::t("Arrow (A)"),
            ),
            (
                Tool::Line,
                IconName::Slash,
                crate::shared::i18n::t("Line (L)"),
            ),
            (
                Tool::Text,
                IconName::Type,
                crate::shared::i18n::t("Text (T)"),
            ),
            (
                Tool::Note,
                IconName::StickyNote,
                crate::shared::i18n::t("Sticky note (N)"),
            ),
            (
                Tool::Image,
                IconName::Image,
                crate::shared::i18n::t("Image"),
            ),
        ];
        let current = self.tool;
        h_flex()
            .absolute()
            .bottom_4()
            .left_0()
            .right_0()
            .justify_center()
            .child(
                bar()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(tools.into_iter().map(|(tool, icon, tip)| {
                        let active = current == tool;
                        tool_button(("canvas-tool", tool as usize), icon, active, tip).on_click(
                            cx.listener(move |this, _, window, cx| {
                                let tool = match tool {
                                    Tool::Rect if this.tool == Tool::Rect => Tool::Ellipse,
                                    Tool::Ellipse if this.tool == Tool::Ellipse => Tool::Rect,
                                    t => t,
                                };
                                this.set_tool(tool, window, cx);
                            }),
                        )
                    }))
                    .child(div().w(px(1.)).h(px(18.)).mx_1().bg(ui::border()))
                    .child(
                        tool_button(
                            "canvas-agent",
                            IconName::Bot,
                            self.agent_shown(),
                            crate::shared::i18n::t("Agent: reads, sees and draws on this canvas"),
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_agent(window, cx))),
                    )
                    .child(
                        tool_button(
                            "canvas-tool-lock",
                            if self.tool_lock {
                                IconName::Lock
                            } else {
                                IconName::LockOpen
                            },
                            self.tool_lock,
                            crate::shared::i18n::t("Keep the tool after drawing"),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tool_lock = !this.tool_lock;
                            cx.notify();
                        })),
                    ),
            )
    }

    /// The zoom pill sits beside the centered tool bar, or above it when
    /// the page is too narrow for both in one row.
    fn pill_bottom(&self) -> f32 {
        let (w, _) = self.size();
        if (w - TOOLBAR_W) / 2. < PILL_W + 24. {
            68.
        } else {
            16.
        }
    }

    fn zoom_pill(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = (self.doc.camera.zoom * 100.).round() as i32;
        let weak = cx.entity().downgrade();
        bar()
            .absolute()
            .bottom(px(self.pill_bottom()))
            .left_4()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                tool_button(
                    "canvas-undo",
                    IconName::Undo2,
                    false,
                    crate::shared::i18n::t("Undo"),
                )
                .when(!self.history.can_undo(), |d| d.opacity(0.4))
                .on_click(cx.listener(|this, _, window, cx| this.undo(&Undo, window, cx))),
            )
            .child(
                tool_button(
                    "canvas-redo",
                    IconName::Redo2,
                    false,
                    crate::shared::i18n::t("Redo"),
                )
                .when(!self.history.can_redo(), |d| d.opacity(0.4))
                .on_click(cx.listener(|this, _, window, cx| this.redo(&Redo, window, cx))),
            )
            .child(div().w(px(1.)).h(px(18.)).mx_1().bg(ui::border()))
            .child(
                tool_button(
                    "canvas-zoom-out",
                    IconName::Minus,
                    false,
                    crate::shared::i18n::t("Zoom out (−)"),
                )
                .on_click(cx.listener(|this, _, _, cx| this.zoom_center(0.8, cx))),
            )
            .child(
                div()
                    .id("canvas-zoom-reset")
                    .w(px(48.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(ui::muted())
                    .hover(|s| s.bg(ui::surface_hover()).text_color(ui::fg()))
                    .tooltip(|window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(crate::shared::i18n::t(
                            "Zoom to 100% (⇧0)",
                        ))
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_to(1., cx)))
                    .child(format!("{zoom}%")),
            )
            .child(
                tool_button(
                    "canvas-zoom-in",
                    IconName::Plus,
                    false,
                    crate::shared::i18n::t("Zoom in (=)"),
                )
                .on_click(cx.listener(|this, _, _, cx| this.zoom_center(1.25, cx))),
            )
            .child(
                tool_button(
                    "canvas-fit",
                    IconName::Scan,
                    false,
                    crate::shared::i18n::t("Zoom to fit (⇧1)"),
                )
                .on_click(cx.listener(|this, _, _, cx| this.fit(cx))),
            )
            .child(
                tool_button(
                    "canvas-minimap",
                    IconName::Map,
                    self.minimap,
                    crate::shared::i18n::t("Minimap"),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.minimap = !this.minimap;
                    cx.notify();
                })),
            )
            .child(div().w(px(1.)).h(px(18.)).mx_1().bg(ui::border()))
            .child(
                div().child(
                    gpui_kit::component::button::Button::new("canvas-more")
                        .ghost()
                        .small()
                        .icon(Icon::new(IconName::Ellipsis))
                        .dropdown_menu(move |menu, _, _| {
                            let entry = |label: &'static str,
                                         run: fn(
                                &mut CanvasView,
                                &mut Window,
                                &mut Context<CanvasView>,
                            )| {
                                let weak = weak.clone();
                                PopupMenuItem::new(crate::shared::i18n::t(label)).on_click(
                                    move |_, window, cx| {
                                        let _ = weak.update(cx, |this, cx| run(this, window, cx));
                                    },
                                )
                            };
                            menu.item(entry("Select all", |this, window, cx| {
                                this.select_all(&SelectAll, window, cx)
                            }))
                            .item(entry("Bring to front", |this, _, cx| {
                                this.reorder(true, cx)
                            }))
                            .item(entry("Send to back", |this, _, cx| this.reorder(false, cx)))
                            .item(entry("Zoom to fit", |this, _, cx| this.fit(cx)))
                            .separator()
                            .item(entry("Clear canvas", |this, _, cx| this.clear(cx)))
                        }),
                ),
            )
    }

    // ── Style panel ────────────────────────────────────────────────────────

    /// What the panel can change: what the current tool draws with and
    /// what shows on the selected shapes.
    pub(super) fn style_controls(&self) -> Controls {
        let drawing = match self.tool {
            Tool::Pen => Controls::INK,
            Tool::Rect | Tool::Ellipse => Controls::GEO,
            Tool::Highlighter | Tool::Arrow | Tool::Line | Tool::Text | Tool::Note => {
                Controls::MARK
            }
            Tool::Select | Tool::Lasso | Tool::Hand | Tool::Eraser | Tool::Laser | Tool::Image => {
                Controls::default()
            }
        };
        self.selected
            .iter()
            .filter_map(|id| self.doc.get(*id))
            .fold(drawing, |c, s| c.union(s.kind.controls()))
    }

    /// Shown while drawing or with shapes selected.
    fn shows_style_panel(&self) -> bool {
        self.style_controls() != Controls::default()
    }

    /// The swatches pick the fill color.
    fn picks_fill_color(&self) -> bool {
        self.fill_tab && self.style_controls().fill
    }

    fn style_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style;
        let controls = self.style_controls();
        let fill_tab = self.picks_fill_color();
        let shown = if fill_tab {
            style.fill_color()
        } else {
            style.color
        };
        let row = |label: &'static str| {
            h_flex().gap_1().child(
                div()
                    .w(px(56.))
                    .text_xs()
                    .text_color(ui::muted())
                    .child(label),
            )
        };
        let colors = Color::ALL.chunks(8).enumerate().map(|(r, chunk)| {
            h_flex()
                .gap(px(4.))
                .children(chunk.iter().enumerate().map(|(i, &c)| {
                    let active = c == shown;
                    div()
                        .id(("canvas-color", r * 8 + i))
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .when(active, |d| d.border_2().border_color(ui::accent()))
                        .hover(|d| d.bg(ui::surface_hover()))
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(crate::shared::i18n::t(
                                c.name(),
                            ))
                            .build(window, cx)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let fill = this.picks_fill_color();
                            this.restyle(
                                move |s| {
                                    if fill {
                                        s.fill_color = Some(c);
                                        if s.fill == Fill::None {
                                            s.fill = Fill::Semi;
                                        }
                                    } else {
                                        s.color = c;
                                    }
                                },
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .size(px(16.))
                                .rounded_full()
                                .bg(paint::color(c))
                                .border_1()
                                .border_color(ui::border_strong()),
                        )
                }))
        });
        let fills = Fill::ALL.into_iter().enumerate().map(|(i, f)| {
            let swatch = div()
                .size(px(12.))
                .rounded(px(3.))
                .border_1()
                .border_color(ui::fg().opacity(0.8));
            let swatch = match f {
                Fill::None => swatch,
                Fill::Semi => swatch.bg(ui::fg().opacity(0.3)),
                Fill::Solid => swatch.bg(ui::fg().opacity(0.85)),
                Fill::Pattern => swatch.child(
                    Icon::new(IconName::Hash)
                        .size(px(10.))
                        .text_color(ui::fg().opacity(0.8)),
                ),
            };
            option(("canvas-fill", i), style.fill == f, swatch)
                .on_click(cx.listener(move |this, _, _, cx| this.restyle(move |s| s.fill = f, cx)))
        });
        let opacities = OPACITIES.into_iter().enumerate().map(|(i, o)| {
            option(
                ("canvas-opacity", i),
                style.opacity == o,
                div()
                    .text_size(ui::code_rems(12.))
                    .font_family(ui::mono())
                    .child(o.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.restyle(move |s| s.opacity = o, cx)))
        });
        let ink = paint::color(style.color);
        let pens = Pen::ALL.into_iter().enumerate().map(|(i, p)| {
            option(
                ("canvas-pen", i),
                style.pen == p,
                // The pen's own stroke, as it draws on the page.
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| paint::pen_preview(p, ink, bounds, window),
                )
                .w(px(40.))
                .h(px(18.)),
            )
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(format!(
                    "{}: {}",
                    crate::shared::i18n::t(p.name()),
                    crate::shared::i18n::t(p.hint())
                ))
                .build(window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| this.restyle(move |s| s.pen = p, cx)))
        });
        let sizes = SIZES.iter().enumerate().map(|(i, _)| {
            let d = 4. + i as f32 * 2.2;
            option(
                ("canvas-size", i),
                style.size as usize == i,
                div()
                    .size(px(d))
                    .rounded_full()
                    .bg(if style.size as usize == i {
                        ui::accent()
                    } else {
                        ui::fg().opacity(0.75)
                    }),
            )
            .on_click(
                cx.listener(move |this, _, _, cx| this.restyle(move |s| s.size = i as u8, cx)),
            )
        });
        v_flex()
            .absolute()
            .top_3()
            // Beside the agent's card when it shows.
            .right(px(if self.agent_shown() {
                super::agent::AGENT_W + 24.
            } else {
                12.
            }))
            .w(px(232.))
            .p_2()
            .gap_2()
            .rounded(px(14.))
            .bg(ui::popover_bg())
            .border_1()
            .border_color(ui::border_strong().opacity(0.6))
            .shadow(vec![BoxShadow {
                color: ui::backdrop().opacity(0.3),
                offset: point(px(0.), px(8.)),
                blur_radius: px(24.),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // Only shapes with an inside have a fill to pick a color for.
            .when(controls.fill, |d| {
                d.child(
                    ui::segmented(
                        "canvas-style-tab",
                        &[
                            crate::shared::i18n::t("Stroke"),
                            crate::shared::i18n::t("Fill"),
                        ],
                        Some(usize::from(fill_tab)),
                        {
                            let weak = cx.entity().downgrade();
                            move |i, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.fill_tab = *i == 1;
                                    cx.notify();
                                });
                            }
                        },
                    )
                    .w_full()
                    .justify_center(),
                )
            })
            .when(controls.color || controls.fill, |d| {
                d.child(v_flex().gap(px(2.)).px_1().children(colors))
            })
            .when(controls.fill, |d| {
                d.child(row(crate::shared::i18n::t("Fill")).children(fills))
            })
            .when(controls.opacity, |d| {
                d.child(row(crate::shared::i18n::t("Opacity")).children(opacities))
            })
            .when(controls.pen, |d| {
                d.child(row(crate::shared::i18n::t("Pen")).children(pens))
            })
            .when(controls.size, |d| {
                d.child(row(crate::shared::i18n::t("Size")).children(sizes))
            })
    }

    // ── Minimap ────────────────────────────────────────────────────────────

    /// Scale and page origin mapping the content (and the view) into the minimap.
    pub(super) fn minimap_fit(&self) -> Option<(f32, P)> {
        let (w, h) = self.size();
        let view = self.doc.camera.visible(w, h);
        let all = self
            .doc
            .content_bounds()
            .map_or(view, |c| c.union(&view))
            .expand(40.);
        let fit = (MINIMAP_W / all.w).min(MINIMAP_H / all.h);
        if !fit.is_finite() || fit <= 0. {
            return None;
        }
        let ox = all.x - (MINIMAP_W / fit - all.w) / 2.;
        let oy = all.y - (MINIMAP_H / fit - all.h) / 2.;
        Some((fit, [ox, oy]))
    }

    fn minimap_card(&self, _cam: Affine, cx: &mut Context<Self>) -> impl IntoElement {
        let (w, h) = self.size();
        let view = self.doc.camera.visible(w, h);
        let fit = self.minimap_fit();
        let boxes: Vec<(Rect, Hsla)> = self
            .doc
            .shapes
            .iter()
            .map(|s| (s.bounds(), paint::color(s.style.color)))
            .collect();
        let cell = self.minimap_bounds.clone();
        div()
            .id("canvas-minimap")
            .absolute()
            .bottom(px(self.pill_bottom() + 56.))
            .left_4()
            .w(px(MINIMAP_W))
            .h(px(MINIMAP_H))
            .rounded(px(10.))
            .overflow_hidden()
            .bg(ui::popover_bg())
            .border_1()
            .border_color(ui::border_strong().opacity(0.6))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.minimap_jump(e.position);
                    this.gesture = Some(Gesture::Minimap);
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    move |bounds, _, _| cell.set(bounds),
                    move |bounds, _, window, _| {
                        let Some((fit, [ox, oy])) = fit else { return };
                        let map = |r: &Rect| {
                            Bounds::new(
                                bounds.origin + point(px((r.x - ox) * fit), px((r.y - oy) * fit)),
                                size(px((r.w * fit).max(1.5)), px((r.h * fit).max(1.5))),
                            )
                        };
                        for (r, color) in &boxes {
                            window.paint_quad(fill(map(r), color.opacity(0.7)));
                        }
                        window.paint_quad(quad(
                            map(&view),
                            px(3.),
                            ui::accent().opacity(0.08),
                            px(1.),
                            ui::accent(),
                            BorderStyle::Solid,
                        ));
                    },
                )
                .size_full(),
            )
    }

    /// Removes every shape (one undo step).
    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        if self.doc.shapes.is_empty() {
            return;
        }
        self.record();
        self.doc.shapes.clear();
        self.touched_all(cx);
    }
}

/// The floating bars' surface.
fn bar() -> Div {
    h_flex()
        .p(px(4.))
        .gap(px(2.))
        .rounded(px(14.))
        .bg(ui::popover_bg())
        .border_1()
        .border_color(ui::border_strong().opacity(0.6))
        .shadow(vec![BoxShadow {
            color: ui::backdrop().opacity(0.3),
            offset: point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(0.),
            inset: false,
        }])
}

fn tool_button(
    id: impl Into<ElementId>,
    icon: IconName,
    active: bool,
    tip: &'static str,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(10.))
        .cursor_pointer()
        .when(active, |d| d.bg(ui::accent().opacity(0.16)))
        .when(!active, |d| d.hover(|s| s.bg(ui::surface_hover())))
        .child(Icon::new(icon).size(px(16.)).text_color(if active {
            ui::accent()
        } else {
            ui::fg().opacity(0.85)
        }))
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
        })
}

/// One choice in a style row.
fn option(id: impl Into<ElementId>, active: bool, child: impl IntoElement) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .cursor_pointer()
        .text_color(if active {
            ui::accent()
        } else {
            ui::fg().opacity(0.8)
        })
        .when(active, |d| d.bg(ui::accent().opacity(0.14)))
        .when(!active, |d| d.hover(|s| s.bg(ui::surface_hover())))
        .child(child)
}
