//! The canvas the diagram-like panel kinds share (sequence, erd, state,
//! timeline), in the diagram card's React Flow look: the dotted grid,
//! pan (drag the background, scroll when expanded), zoom (⌘/Ctrl-scroll,
//! pinch, the controls), fit, the minimap, the details island, and a
//! floating text field for renames. Each kind paints its lines as
//! [`Shape`]s and puts its cards on top as elements.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gentle_core::genui::layout::{self, EdgePath, Rect};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _, v_flex};
use gpui_kit::*;

use super::diagram::{MAX_ZOOM, MIN_ZOOM, READABLE_ZOOM};
use super::look;
use super::motion::{self, ViewTween};
use super::paint::{self, MiniMap, minimap_fit};
use super::panel::{Body, PanelView};
use super::pointer::ZOOM_STEP;
use crate::shared::{backdrop, ui};

/// The canvas' height in the conversation card (px at 100 %): with the
/// header and footer, the diagram card's 460.
pub(super) const CANVAS_HEIGHT: f32 = 384.;
const MINIMAP_W: f32 = 168.;
const MINIMAP_H: f32 = 108.;
/// A press that moves less than this (px) is a click, not a drag.
pub(super) const CLICK_SLOP: f32 = 3.;

/// Pan, zoom and what the pointer does to them.
pub(super) struct Canvas {
    /// Px from the canvas' top-left.
    pub pan: Point<f32>,
    /// × the interface size.
    pub zoom: f32,
    fitted_for: Option<Size<Pixels>>,
    /// The user panned or zoomed: a resize no longer refits.
    touched: bool,
    tween: Option<ViewTween>,
    /// The canvas' and the minimap's bounds last frame.
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    minimap: Rc<Cell<Bounds<Pixels>>>,
    /// What the kind drew last frame (diagram units).
    content: Rect,
    /// Panning: where the press was, the pan then, whether it moved.
    panning: Option<(Point<Pixels>, Point<f32>, bool)>,
    minimap_drag: bool,
    /// A rename in progress.
    pub edit: Option<CanvasEdit>,
}

/// A text field over the canvas: `target` says what it renames (the kind
/// reads it), `at` where it sits (diagram units).
pub(super) struct CanvasEdit {
    pub target: String,
    pub at: Rect,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            pan: Point::default(),
            zoom: 1.,
            fitted_for: None,
            touched: false,
            tween: None,
            bounds: Rc::new(Cell::new(Bounds::default())),
            minimap: Rc::new(Cell::new(Bounds::default())),
            content: Rect::default(),
            panning: None,
            minimap_drag: false,
            edit: None,
        }
    }

    /// Pixels per diagram unit.
    pub fn scale(&self, rem: Pixels) -> f32 {
        self.zoom * f32::from(rem) / 16.
    }

    /// A window point in diagram units.
    pub fn to_world(&self, position: Point<Pixels>, rem: Pixels) -> layout::Point {
        let local = position - self.bounds.get().origin;
        let s = self.scale(rem);
        layout::Point {
            x: (f32::from(local.x) - self.pan.x) / s,
            y: (f32::from(local.y) - self.pan.y) / s,
        }
    }

    /// A diagram point on the canvas (px from its top-left).
    pub fn at(&self, x: f32, y: f32, rem: Pixels) -> (Pixels, Pixels) {
        let s = self.scale(rem);
        (px(x * s + self.pan.x), px(y * s + self.pan.y))
    }

    /// The canvas in diagram units.
    pub fn visible(&self, rem: Pixels) -> Rect {
        let s = self.scale(rem);
        let size = self.bounds.get().size;
        Rect {
            x: -self.pan.x / s,
            y: -self.pan.y / s,
            w: f32::from(size.width) / s,
            h: f32::from(size.height) / s,
        }
    }

    /// The pan and zoom fitting `content`, at most 100 % and at least
    /// `floor`; past the floor the view starts at the content's top-left.
    fn fit_target(&self, floor: f32, rem: Pixels) -> Option<(Point<f32>, f32)> {
        let size = self.bounds.get().size;
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        if w < 1. || h < 1. {
            return None;
        }
        let content = self.content.inflate(24.);
        let unit = f32::from(rem) / 16.;
        let zoom = if content.w <= 0. || content.h <= 0. {
            1.
        } else {
            (w / content.w).min(h / content.h) / unit
        }
        .clamp(floor, 1.);
        let s = zoom * unit;
        let axis = |view: f32, start: f32, length: f32| {
            if length * s <= view {
                view / 2. - (start + length / 2.) * s
            } else {
                -start * s
            }
        };
        Some((
            point(axis(w, content.x, content.w), axis(h, content.y, content.h)),
            zoom,
        ))
    }

    /// Each frame, before drawing `content`: advance the view's tween, fit
    /// the first frame and on resizes the user didn't override. Whether
    /// the view still eases.
    pub fn prepare(&mut self, content: Rect, rem: Pixels) -> bool {
        self.content = content;
        let mut tweening = false;
        if let Some(tween) = &self.tween {
            let ((pan, zoom), done) = tween.at(Instant::now());
            (self.pan, self.zoom) = (pan, zoom);
            if done {
                self.tween = None;
            }
            tweening = !done;
        }
        let size = self.bounds.get().size;
        let refit = self.fitted_for.is_none()
            || (!self.touched && self.tween.is_none() && self.fitted_for != Some(size));
        if refit
            && size.width > px(1.)
            && size.height > px(1.)
            && let Some((pan, zoom)) = self.fit_target(READABLE_ZOOM, rem)
        {
            (self.pan, self.zoom) = (pan, zoom);
            self.fitted_for = Some(size);
        }
        tweening
    }

    fn ease_to(&mut self, target: (Point<f32>, f32), cx: &App) {
        if cx.reduce_motion() {
            self.tween = None;
            (self.pan, self.zoom) = target;
        } else {
            self.tween = Some(ViewTween::new(
                (self.pan, self.zoom),
                target,
                Instant::now(),
            ));
        }
    }

    pub fn fit_animated(&mut self, rem: Pixels, cx: &App) {
        if let Some(target) = self.fit_target(MIN_ZOOM, rem) {
            self.touched = false;
            self.fitted_for = Some(self.bounds.get().size);
            self.ease_to(target, cx);
        }
    }

    /// The zoom buttons: around the canvas' center, from where a running
    /// tween was heading.
    pub fn zoom_center(&mut self, factor: f32, rem: Pixels, cx: &App) {
        let (pan, zoom) = self
            .tween
            .as_ref()
            .map_or((self.pan, self.zoom), ViewTween::target);
        let size = self.bounds.get().size;
        let anchor = point(f32::from(size.width) / 2., f32::from(size.height) / 2.);
        let unit = f32::from(rem) / 16.;
        let world = point(
            (anchor.x - pan.x) / (zoom * unit),
            (anchor.y - pan.y) / (zoom * unit),
        );
        let zoom = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let s = zoom * unit;
        self.touched = true;
        self.ease_to(
            (point(anchor.x - world.x * s, anchor.y - world.y * s), zoom),
            cx,
        );
    }

    /// Zoom by `factor` keeping the point under `position` still.
    fn zoom_at(&mut self, factor: f32, position: Point<Pixels>, rem: Pixels) {
        self.tween = None;
        let before = self.to_world(position, rem);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let s = self.scale(rem);
        let local = position - self.bounds.get().origin;
        self.pan = point(
            f32::from(local.x) - before.x * s,
            f32::from(local.y) - before.y * s,
        );
        self.touched = true;
    }

    fn navigate_minimap(&mut self, position: Point<Pixels>, rem: Pixels) {
        let map = self.minimap.get();
        let content = self.content.union(&self.visible(rem));
        let (k, offset) = minimap_fit(content, map.size);
        if k <= 0. {
            return;
        }
        let local = position - map.origin;
        let world = layout::Point {
            x: (f32::from(local.x) - offset.x) / k,
            y: (f32::from(local.y) - offset.y) / k,
        };
        let s = self.scale(rem);
        let size = self.bounds.get().size;
        self.tween = None;
        self.pan = point(
            f32::from(size.width) / 2. - world.x * s,
            f32::from(size.height) / 2. - world.y * s,
        );
        self.touched = true;
    }
}

/// Whether the pointer moved past a click since `from`.
pub(super) fn moved(from: Point<Pixels>, to: Point<Pixels>) -> bool {
    let d = to - from;
    f32::from(d.x).hypot(f32::from(d.y)) >= CLICK_SLOP
}

// ── Shapes ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Head {
    None,
    /// An open chevron (`look::chevron`), stroked at the curve's width:
    /// every arrow on the canvases ends this way, as on the diagram; a
    /// message's kind shows in its color and dash instead.
    Arrow,
}

/// What a kind paints under its elements (diagram units; stroke widths
/// in px at 100 %).
pub(super) enum Shape {
    Box {
        rect: Rect,
        fill: Hsla,
        border: Option<Hsla>,
        /// The border's width in px (a hairline is 1, a container's 1.5).
        border_width: f32,
        radius: f32,
        dashed: bool,
    },
    Line {
        from: layout::Point,
        to: layout::Point,
        color: Hsla,
        width: f32,
        dash: Option<[f32; 2]>,
    },
    Curve {
        path: EdgePath,
        color: Hsla,
        width: f32,
        dash: Option<[f32; 2]>,
        head: Head,
    },
    /// Filled.
    Polygon {
        points: Vec<layout::Point>,
        color: Hsla,
    },
    /// A connection being drawn, pointer-following.
    Rubber {
        from: layout::Point,
        to: layout::Point,
    },
}

struct Scene {
    shapes: Vec<Shape>,
    scale: f32,
    pan: Point<f32>,
}

impl Scene {
    fn at(&self, origin: Point<Pixels>, p: layout::Point) -> Point<Pixels> {
        point(
            origin.x + px(p.x * self.scale + self.pan.x),
            origin.y + px(p.y * self.scale + self.pan.y),
        )
    }

    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let origin = bounds.origin;
        let s = self.scale;
        let stroke = s.clamp(0.4, 1.5);
        paint::grid(bounds, s, self.pan, window);
        for shape in &self.shapes {
            match shape {
                Shape::Box {
                    rect,
                    fill,
                    border,
                    border_width,
                    radius,
                    dashed,
                } => window.paint_quad(quad(
                    Bounds::new(
                        self.at(
                            origin,
                            layout::Point {
                                x: rect.x,
                                y: rect.y,
                            },
                        ),
                        size(px(rect.w * s), px(rect.h * s)),
                    ),
                    px(radius * s),
                    *fill,
                    px(if border.is_some() { *border_width } else { 0. }),
                    border.unwrap_or(*fill),
                    if *dashed {
                        BorderStyle::Dashed
                    } else {
                        BorderStyle::Solid
                    },
                )),
                Shape::Line {
                    from,
                    to,
                    color,
                    width,
                    dash,
                } => {
                    let points = [self.at(origin, *from), self.at(origin, *to)];
                    let mut path = PathBuilder::stroke(px(width * stroke));
                    match dash {
                        Some([on, off]) => {
                            paint::dash_polyline(&mut path, &points, on * stroke, off * stroke, 0.)
                        }
                        None => {
                            path.move_to(points[0]);
                            path.line_to(points[1]);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, *color);
                    }
                }
                Shape::Curve {
                    path,
                    color,
                    width,
                    dash,
                    head,
                } => self.curve(origin, path, *color, width * stroke, *dash, *head, window),
                Shape::Polygon { points, color } => {
                    let points: Vec<Point<Pixels>> =
                        points.iter().map(|p| self.at(origin, *p)).collect();
                    let mut path = PathBuilder::fill();
                    path.add_polygon(&points, true);
                    if let Ok(path) = path.build() {
                        window.paint_path(path, *color);
                    }
                }
                Shape::Rubber { from, to } => {
                    let mut path = PathBuilder::stroke(px(1.5)).dash_array(&[px(5.), px(4.)]);
                    path.move_to(self.at(origin, *from));
                    path.line_to(self.at(origin, *to));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, ui::accent());
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn curve(
        &self,
        origin: Point<Pixels>,
        path: &EdgePath,
        color: Hsla,
        width: f32,
        dash: Option<[f32; 2]>,
        head: Head,
        window: &mut Window,
    ) {
        let stroke = self.scale.clamp(0.4, 1.5);
        let dir = path.end_direction();
        let end = path.end();
        let mut points: Vec<Point<Pixels>> = Vec::new();
        for (i, seg) in path.segments.iter().enumerate() {
            for k in usize::from(i > 0)..=16 {
                let t = k as f32 / 16.;
                let u = 1. - t;
                let (a, b, c, d) = (u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t);
                points.push(self.at(
                    origin,
                    layout::Point {
                        x: a * seg.from.x + b * seg.c1.x + c * seg.c2.x + d * seg.to.x,
                        y: a * seg.from.y + b * seg.c1.y + c * seg.c2.y + d * seg.to.y,
                    },
                ));
            }
        }
        let mut line = PathBuilder::stroke(px(width));
        match dash {
            Some([on, off]) => {
                paint::dash_polyline(&mut line, &points, on * stroke, off * stroke, 0.)
            }
            None => {
                line.move_to(points[0]);
                for p in &points[1..] {
                    line.line_to(*p);
                }
            }
        }
        if let Ok(line) = line.build() {
            window.paint_path(line, color);
        }
        if head == Head::Arrow {
            let [left, tip, right] = look::chevron(self.at(origin, end), (dir.x, dir.y), stroke);
            let mut arrow = PathBuilder::stroke(px(width));
            arrow.move_to(left);
            arrow.line_to(tip);
            arrow.line_to(right);
            if let Ok(arrow) = arrow.build() {
                window.paint_path(arrow, color);
            }
        }
    }
}

/// A straight path from `a` to `b` (one cubic), for [`Shape::Curve`].
pub(super) fn straight(a: layout::Point, b: layout::Point) -> EdgePath {
    EdgePath {
        segments: vec![layout::Cubic {
            from: a,
            c1: a,
            c2: b,
            to: b,
        }],
    }
}

/// What a kind hands the canvas each frame.
pub(super) struct Drawn {
    pub shapes: Vec<Shape>,
    /// Cards, labels and handles, placed with [`Canvas::at`].
    pub children: Vec<AnyElement>,
    /// The minimap's boxes.
    pub minimap: Vec<(Rect, Hsla)>,
    /// The selection's details, top-right.
    pub details: Option<AnyElement>,
    pub cursor: Option<CursorStyle>,
    /// Shown when there's nothing to draw.
    pub empty: Option<&'static str>,
}

impl PanelView {
    pub(super) fn canvas(&self) -> Option<&Canvas> {
        match &self.body {
            Some(Body::Sequence(s)) => Some(&s.canvas),
            Some(Body::Erd(s)) => Some(&s.canvas),
            Some(Body::State(s)) => Some(&s.canvas),
            Some(Body::Timeline(s)) => Some(&s.canvas),
            _ => None,
        }
    }

    pub(super) fn canvas_mut(&mut self) -> Option<&mut Canvas> {
        match &mut self.body {
            Some(Body::Sequence(s)) => Some(&mut s.canvas),
            Some(Body::Erd(s)) => Some(&mut s.canvas),
            Some(Body::State(s)) => Some(&mut s.canvas),
            Some(Body::Timeline(s)) => Some(&mut s.canvas),
            _ => None,
        }
    }

    /// Ask for the next frame while the view eases.
    pub(super) fn canvas_frames(&self, tweening: bool, window: &mut Window, cx: &mut App) {
        if let Some(delay) = motion::frame_delay(
            tweening,
            false,
            window.is_window_active(),
            cx.reduce_motion(),
            backdrop::motion_now(cx),
        ) {
            ui::request_frame_in(delay, window, cx);
        }
    }

    /// The canvas element: grid and shapes painted, the kind's elements on
    /// top, the rename field, controls, minimap and details.
    pub(super) fn render_canvas(
        &self,
        drawn: Drawn,
        rem: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(c) = self.canvas() else {
            return div().into_any_element();
        };
        let scene = Scene {
            shapes: drawn.shapes,
            scale: c.scale(rem),
            pan: c.pan,
        };
        let cell = c.bounds.clone();
        let painter = canvas(
            move |bounds, window, _| {
                let old = cell.get();
                cell.set(bounds);
                if (old.size.width - bounds.size.width).abs() > px(1.)
                    || (old.size.height - bounds.size.height).abs() > px(1.)
                {
                    ui::request_animation_frame(window);
                }
            },
            move |bounds, _, window, _| scene.paint(bounds, window),
        )
        .absolute()
        .size_full();
        let panning = c.panning.is_some_and(|(_, _, moved)| moved);
        let mut body = div()
            .id("genui-canvas")
            .relative()
            .flex_1()
            .min_h_0()
            .h(rems(CANVAS_HEIGHT / 16.))
            .overflow_hidden()
            .bg(ui::well())
            .cursor(if panning {
                CursorStyle::ClosedHand
            } else {
                drawn.cursor.unwrap_or(CursorStyle::OpenHand)
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::canvas_down))
            .on_mouse_move(cx.listener(Self::canvas_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.canvas_up(window, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.canvas_up(window, cx)),
            )
            .on_scroll(cx.listener(Self::canvas_scroll))
            .on_pinch(cx.listener(|this, event: &PinchEvent, window, cx| {
                let rem = window.rem_size();
                if let Some(c) = this.canvas_mut() {
                    c.zoom_at(1. + event.delta, event.position, rem);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(painter)
            .children(drawn.children);
        if let Some(edit) = &c.edit {
            let (x, y) = c.at(edit.at.x, edit.at.y + edit.at.h / 2., rem);
            body = body.child(
                div()
                    .absolute()
                    .left(x)
                    .top(y - rems(15. / 16.).to_pixels(rem))
                    .w(px(edit.at.w * c.scale(rem)).max(rems(160. / 16.).to_pixels(rem)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        ui::field()
                            .h(rems(30. / 16.))
                            .bg(ui::popover_bg())
                            .border_color(ui::accent().opacity(0.6))
                            .child(Input::new(&edit.input).appearance(false).small()),
                    ),
            );
        }
        if let Some(empty) = drawn.empty {
            body = body.child(
                v_flex()
                    .absolute()
                    .inset_0()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(Icon::new(IconName::Workflow).text_color(ui::faint()))
                    .child(div().text_sm().text_color(ui::muted()).child(empty)),
            );
        }
        let minimap =
            (!drawn.minimap.is_empty()).then(|| self.canvas_minimap(drawn.minimap, rem, cx));
        body.child(self.canvas_controls(cx))
            .children(minimap)
            .children(drawn.details.map(|details| {
                div()
                    .absolute()
                    .top_2()
                    .right_2()
                    .w(rems(232. / 16.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(details)
            }))
            .into_any_element()
    }

    /// Zoom in, out and fit, floating bottom-left.
    fn canvas_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                control("genui-canvas-zoom-in", IconName::ZoomIn, "Zoom in").on_click(cx.listener(
                    |this, _, window, cx| {
                        let rem = window.rem_size();
                        if let Some(c) = this.canvas_mut() {
                            c.zoom_center(ZOOM_STEP, rem, cx);
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                control("genui-canvas-zoom-out", IconName::ZoomOut, "Zoom out").on_click(
                    cx.listener(|this, _, window, cx| {
                        let rem = window.rem_size();
                        if let Some(c) = this.canvas_mut() {
                            c.zoom_center(1. / ZOOM_STEP, rem, cx);
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                control("genui-canvas-fit", IconName::Scan, "Fit the whole view").on_click(
                    cx.listener(|this, _, window, cx| {
                        let rem = window.rem_size();
                        if let Some(c) = this.canvas_mut() {
                            c.fit_animated(rem, cx);
                        }
                        cx.notify();
                    }),
                ),
            )
    }

    /// The minimap, bottom-right: press or drag to move the view there.
    fn canvas_minimap(
        &self,
        boxes: Vec<(Rect, Hsla)>,
        rem: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(c) = self.canvas() else {
            return div().into_any_element();
        };
        let view = c.visible(rem);
        let map = MiniMap {
            nodes: boxes,
            content: c.content.union(&view),
            view,
        };
        let cell = c.minimap.clone();
        div()
            .id("genui-canvas-minimap")
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
                    cx.stop_propagation();
                    let rem = window.rem_size();
                    if let Some(c) = this.canvas_mut() {
                        c.navigate_minimap(event.position, rem);
                        c.minimap_drag = true;
                    }
                    cx.notify();
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
            .into_any_element()
    }

    /// A press on the background: the kind's (a lifeline, a link) or a pan.
    fn canvas_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.finish_canvas_edit(true, cx);
        let handled = match &self.body {
            Some(Body::Sequence(_)) => self.sequence_down(event, window, cx),
            Some(Body::Erd(_)) => self.erd_down(event, window, cx),
            Some(Body::State(_)) => self.state_down(event, window, cx),
            Some(Body::Timeline(_)) => self.timeline_down(event, window, cx),
            _ => false,
        };
        if !handled && let Some(c) = self.canvas_mut() {
            c.tween = None;
            c.panning = Some((event.position, c.pan, false));
        }
        cx.notify();
    }

    fn canvas_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let rem = window.rem_size();
        let Some(c) = self.canvas_mut() else { return };
        if let Some((from, pan, moving)) = &mut c.panning {
            *moving |= moved(*from, event.position);
            let d = event.position - *from;
            c.pan = point(pan.x + f32::from(d.x), pan.y + f32::from(d.y));
            c.touched = true;
            cx.notify();
            return;
        }
        if c.minimap_drag {
            c.navigate_minimap(event.position, rem);
            cx.notify();
            return;
        }
        let pressed = event.pressed_button == Some(MouseButton::Left);
        match &self.body {
            Some(Body::Sequence(_)) => self.sequence_move(event, pressed, window, cx),
            Some(Body::Erd(_)) => self.erd_move(event, pressed, window, cx),
            Some(Body::State(_)) => self.state_move(event, pressed, window, cx),
            Some(Body::Timeline(_)) => self.timeline_move(event, pressed, window, cx),
            _ => {}
        }
    }

    fn canvas_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.canvas_mut() else { return };
        c.minimap_drag = false;
        if let Some((_, _, moving)) = c.panning.take() {
            // A click on the background clears the selection.
            if !moving {
                self.canvas_select(None, cx);
            }
            cx.notify();
            return;
        }
        match &self.body {
            Some(Body::Sequence(_)) => self.sequence_up(window, cx),
            Some(Body::Erd(_)) => self.erd_up(window, cx),
            Some(Body::State(_)) => self.state_up(window, cx),
            Some(Body::Timeline(_)) => self.timeline_up(window, cx),
            _ => {}
        }
        cx.notify();
    }

    fn canvas_scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let rem = window.rem_size();
        let delta = event.delta.pixel_delta(rem);
        let expanded = self.expanded;
        let zoom = event.modifiers.secondary(cx);
        let Some(c) = self.canvas_mut() else { return };
        if zoom {
            let factor = (1. + f32::from(delta.y) / 300.).clamp(0.5, 2.);
            c.zoom_at(factor, event.position, rem);
        } else if expanded {
            c.tween = None;
            c.pan.x += f32::from(delta.x);
            c.pan.y += f32::from(delta.y);
            c.touched = true;
        } else {
            // In the conversation a plain wheel scrolls the conversation.
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// The selection, as the kind's key (`None`: nothing).
    pub(super) fn canvas_select(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        match &mut self.body {
            Some(Body::Sequence(s)) => s.selected = key,
            Some(Body::Erd(s)) => s.selected = key,
            Some(Body::State(s)) => s.selected = key,
            Some(Body::Timeline(s)) => s.selected = key,
            _ => return,
        }
        cx.notify();
    }

    /// Open the rename field for `target` (the kind's key) over `at`.
    pub(super) fn start_canvas_edit(
        &mut self,
        target: String,
        text: String,
        at: Rect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.finish_canvas_edit(true, cx);
                    window.focus(&this.focus, cx);
                }
                InputEvent::Blur => this.finish_canvas_edit(true, cx),
                _ => {}
            },
        );
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        if let Some(c) = self.canvas_mut() {
            c.edit = Some(CanvasEdit {
                target,
                at,
                input,
                _subscription: subscription,
            });
        }
        cx.notify();
    }

    /// Close the rename field, applying it when `keep`.
    pub(super) fn finish_canvas_edit(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(edit) = self.canvas_mut().and_then(|c| c.edit.take()) else {
            return;
        };
        cx.notify();
        if !keep {
            return;
        }
        let text = edit.input.read(cx).value().trim().to_string();
        match &self.body {
            Some(Body::Sequence(_)) => self.sequence_text(&edit.target, text, cx),
            Some(Body::Erd(_)) => self.erd_text(&edit.target, text, cx),
            Some(Body::State(_)) => self.state_text(&edit.target, text, cx),
            Some(Body::Timeline(_)) => self.timeline_text(&edit.target, text, cx),
            _ => {}
        }
    }

    /// Keys the canvas kinds share: Escape closes a rename or clears the
    /// selection; ⌘/Ctrl-0 fits. Whether it was handled.
    pub(super) fn canvas_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let editing = self.canvas().is_some_and(|c| c.edit.is_some());
        match keystroke.key.as_str() {
            "escape" if editing => {
                self.finish_canvas_edit(false, cx);
                window.focus(&self.focus, cx);
                true
            }
            "0" if keystroke.modifiers.secondary(cx) && !self.typing(window, cx) => {
                let rem = window.rem_size();
                if let Some(c) = self.canvas_mut() {
                    c.fit_animated(rem, cx);
                }
                cx.notify();
                true
            }
            _ => false,
        }
    }
}

/// The details island's frame: a title row (an icon badge in `tone`, as on
/// the element's card), then `rows`.
pub(super) fn details(
    icon: IconName,
    tone: Hsla,
    title: String,
    rows: Vec<AnyElement>,
    cx: &App,
) -> AnyElement {
    ui::island(cx)
        .flex()
        .flex_col()
        .p_2()
        .gap_1p5()
        .text_size(rems(12. / 16.))
        .child(
            gpui_kit::component::h_flex()
                .gap_1p5()
                .min_w_0()
                .child(look::badge(tone, icon, 20., 1.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ui::fg())
                        .child(title),
                ),
        )
        .children(rows)
        .into_any_element()
}

/// A row of choice pills (the current one in `accent`), each with its
/// `debug_selector` `"{id}-{value}"`.
pub(super) fn pills(
    id: &'static str,
    choices: &[&'static str],
    current: &str,
    on_pick: impl Fn(&mut PanelView, &'static str, &mut Context<PanelView>) + Clone + 'static,
    cx: &mut Context<PanelView>,
) -> AnyElement {
    gpui_kit::component::h_flex()
        .flex_wrap()
        .gap_1()
        .children(choices.iter().map(|&choice| {
            let on = choice == current;
            let pick = on_pick.clone();
            div()
                .id((id, choice.as_ptr() as usize))
                .debug_selector(move || format!("{id}-{choice}"))
                .px_1p5()
                .rounded_full()
                .cursor_pointer()
                .border_1()
                .border_color(if on {
                    ui::accent().opacity(0.6)
                } else {
                    ui::border()
                })
                .bg(if on {
                    ui::accent().opacity(0.14)
                } else {
                    ui::surface()
                })
                .text_size(rems(11. / 16.))
                .text_color(if on { ui::accent() } else { ui::muted() })
                .hover(|d| d.text_color(ui::fg()))
                .child(crate::shared::i18n::t(choice))
                .on_click(cx.listener(move |this, _, _, cx| pick(this, choice, cx)))
        }))
        .into_any_element()
}

/// The island's actions: ghost buttons in a row.
pub(super) fn actions(buttons: Vec<Button>) -> AnyElement {
    gpui_kit::component::h_flex()
        .flex_wrap()
        .gap_0p5()
        .children(buttons)
        .into_any_element()
}

/// A ghost button for [`actions`].
pub(super) fn action(id: &'static str, icon: IconName, label: &'static str) -> Button {
    Button::new(id)
        .debug_selector(|| id.into())
        .ghost()
        .xsmall()
        .icon(Icon::new(icon))
        .label(crate::shared::i18n::t(label))
}
