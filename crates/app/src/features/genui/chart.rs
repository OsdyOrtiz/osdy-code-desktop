//! The chart panel: bar, line, area, pie/donut and scatter charts painted
//! natively (`PathBuilder`, `paint_quad`), laid out by
//! `gentle_core::genui::chart`. Values grow in when shown and tween to the
//! agent's next version; hover shows a value, click one to ask about it.
//! Read-only: it sends no changes.

use std::cell::Cell;
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

use gentle_core::genui::chart::{Axes, Chart, ChartType, Geometry, Shape, mid_angle, short_number};
use gentle_core::genui::layout::{Point as PlotPoint, Rect};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::motion::Tween;
use super::panel::{Body, PanelView};
use crate::shared::ui;

/// Room around the plot (px at 100 %) for tick labels and titles.
const LEFT: f32 = 44.;
const BOTTOM: f32 = 24.;
const TOP: f32 = 10.;
const RIGHT: f32 = 14.;
/// How close (px) the pointer must be to a point to pick it.
const REACH: f32 = 12.;

fn series_color(i: usize) -> Hsla {
    let palette = [
        ui::info(),
        ui::accent(),
        ui::success(),
        ui::warning(),
        ui::champagne(),
        ui::petal(),
        ui::danger(),
        ui::muted(),
    ];
    palette[i % palette.len()]
}

pub(super) struct ChartState {
    /// The agent's latest version.
    pub chart: Chart,
    /// Where the tween to `chart` started.
    from: Chart,
    tween: Tween,
    /// The last frame's progress, so a new version starts from what shows.
    progress: f32,
    /// First showing: pie and donut sweep open.
    opening: bool,
    pub hover: Option<usize>,
    pub selected: Option<(usize, usize)>,
    pointer: Point<Pixels>,
    /// The plot's bounds last frame (written while painting).
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl ChartState {
    pub fn new(chart: Chart) -> Self {
        let mut tween = Tween::default();
        tween.start();
        Self {
            from: chart.zeroed(),
            chart,
            tween,
            progress: 0.,
            opening: true,
            hover: None,
            selected: None,
            pointer: Point::default(),
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    /// A newer version tweens from what is drawn now.
    pub fn rebase(&mut self, chart: Chart) {
        self.from = Chart::lerp(&self.from, &self.chart, self.progress);
        self.chart = chart;
        self.opening = false;
        self.tween.start();
        self.hover = None;
        self.selected = None;
    }
}

/// The plot area inside the canvas (px, from its top-left).
fn plot_rect(size: Size<Pixels>, rem: f32, round: bool) -> Rect {
    let (w, h) = (f32::from(size.width), f32::from(size.height));
    if round {
        let pad = 12. * rem;
        return Rect {
            x: pad,
            y: pad,
            w: (w - pad * 2.).max(0.),
            h: (h - pad * 2.).max(0.),
        };
    }
    Rect {
        x: LEFT * rem,
        y: TOP * rem,
        w: (w - (LEFT + RIGHT) * rem).max(0.),
        h: (h - (TOP + BOTTOM) * rem).max(0.),
    }
}

/// What one frame paints.
struct Frame {
    chart: Chart,
    axes: Option<Axes>,
    geometry: Geometry,
    plot: Rect,
    hover: Option<usize>,
    selected: Option<(usize, usize)>,
    rem: f32,
}

impl Frame {
    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let o = bounds.origin;
        let at = |p: PlotPoint| point(o.x + px(p.x), o.y + px(p.y));
        let plot = self.plot;
        if let Some(axes) = &self.axes {
            for tick in &axes.y.ticks {
                let y = plot.y + plot.h * (1. - axes.y.fraction(*tick));
                let mut line = PathBuilder::stroke(px(1.));
                line.move_to(at(PlotPoint { x: plot.x, y }));
                line.line_to(at(PlotPoint {
                    x: plot.x + plot.w,
                    y,
                }));
                if let Ok(line) = line.build() {
                    window.paint_path(line, ui::border());
                }
            }
            if let Some(x_axis) = &axes.x {
                for tick in &x_axis.ticks {
                    let x = plot.x + plot.w * x_axis.fraction(*tick);
                    let mut line = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(4.)]);
                    line.move_to(at(PlotPoint { x, y: plot.y }));
                    line.line_to(at(PlotPoint {
                        x,
                        y: plot.y + plot.h,
                    }));
                    if let Ok(line) = line.build() {
                        window.paint_path(line, ui::border());
                    }
                }
            }
        }
        let emphasis = |i: usize| self.hover == Some(i);
        let picked = |series: usize, index: usize| self.selected == Some((series, index));
        // Areas under their lines, then the lines.
        if matches!(self.chart.kind, ChartType::Line | ChartType::Area) {
            for (si, line) in self.geometry.lines.iter().enumerate() {
                let color = series_color(si);
                if self.chart.kind == ChartType::Area && line.len() > 1 {
                    let mut area = PathBuilder::fill();
                    let mut outline: Vec<Point<Pixels>> = line.iter().map(|p| at(*p)).collect();
                    outline.extend(self.geometry.floors[si].iter().rev().map(|p| at(*p)));
                    area.add_polygon(&outline, true);
                    if let Ok(area) = area.build() {
                        window.paint_path(area, color.opacity(0.18));
                    }
                }
                if line.len() > 1 {
                    let mut stroke = PathBuilder::stroke(px(2. * self.rem.max(0.75)));
                    stroke.move_to(at(line[0]));
                    for p in &line[1..] {
                        stroke.line_to(at(*p));
                    }
                    if let Ok(stroke) = stroke.build() {
                        window.paint_path(stroke, color);
                    }
                }
            }
        }
        let dots = self.geometry.lines.first().is_none_or(|l| l.len() <= 60);
        for (i, mark) in self.geometry.marks.iter().enumerate() {
            let color = match self.chart.kind {
                ChartType::Pie | ChartType::Donut => series_color(mark.index),
                _ => series_color(mark.series),
            };
            let strong = emphasis(i) || picked(mark.series, mark.index);
            match &mark.shape {
                Shape::Bar(rect) => {
                    let r = px((3. * self.rem).min(rect.w / 2.));
                    window.paint_quad(quad(
                        Bounds::new(
                            at(PlotPoint {
                                x: rect.x,
                                y: rect.y,
                            }),
                            size(px(rect.w), px(rect.h)),
                        ),
                        Corners {
                            top_left: r,
                            top_right: r,
                            bottom_right: px(0.),
                            bottom_left: px(0.),
                        },
                        color.opacity(if strong { 1. } else { 0.82 }),
                        px(if picked(mark.series, mark.index) {
                            2.
                        } else {
                            0.
                        }),
                        ui::fg(),
                        BorderStyle::Solid,
                    ));
                }
                Shape::Dot(p) => {
                    let scatter = self.chart.kind == ChartType::Scatter;
                    if !scatter && !dots && !strong {
                        continue;
                    }
                    let radius = if strong {
                        5.
                    } else if scatter {
                        4.
                    } else {
                        3.
                    } * self.rem;
                    let center = at(*p);
                    window.paint_quad(quad(
                        Bounds::new(
                            point(center.x - px(radius), center.y - px(radius)),
                            size(px(radius * 2.), px(radius * 2.)),
                        ),
                        px(radius),
                        color.opacity(if scatter && !strong { 0.8 } else { 1. }),
                        px(if strong { 2. } else { 0. }),
                        ui::surface(),
                        BorderStyle::Solid,
                    ));
                }
                Shape::Slice {
                    center,
                    radius,
                    inner,
                    start,
                    end,
                } => {
                    if end - start <= 0.001 {
                        continue;
                    }
                    // Hover lifts a slice out a little.
                    let lift = if strong { 6. * self.rem } else { 0. };
                    let mid = mid_angle(*start, *end);
                    let c = PlotPoint {
                        x: center.x + mid.cos() * lift,
                        y: center.y + mid.sin() * lift,
                    };
                    let steps = (((end - start) / TAU) * 120.).ceil().max(2.) as usize;
                    let arc = |r: f32, a: f32| {
                        // Clockwise from 12 o'clock.
                        let a = a - PI / 2.;
                        at(PlotPoint {
                            x: c.x + a.cos() * r,
                            y: c.y + a.sin() * r,
                        })
                    };
                    let mut outline: Vec<Point<Pixels>> = (0..=steps)
                        .map(|k| arc(*radius, start + (end - start) * k as f32 / steps as f32))
                        .collect();
                    if *inner > 0. {
                        outline.extend(
                            (0..=steps).rev().map(|k| {
                                arc(*inner, start + (end - start) * k as f32 / steps as f32)
                            }),
                        );
                    } else {
                        outline.push(at(c));
                    }
                    let mut fill = PathBuilder::fill();
                    fill.add_polygon(&outline, true);
                    if let Ok(fill) = fill.build() {
                        window.paint_path(fill, color.opacity(if strong { 1. } else { 0.85 }));
                    }
                    // A hairline of the card between slices.
                    let mut edge = PathBuilder::stroke(px(1.5));
                    edge.add_polygon(&outline, true);
                    if let Ok(edge) = edge.build() {
                        window.paint_path(edge, ui::surface());
                    }
                }
            }
        }
    }
}

fn state(this: &mut PanelView) -> Option<&mut ChartState> {
    match &mut this.body {
        Some(Body::Chart(s)) => Some(s),
        _ => None,
    }
}

impl PanelView {
    fn chart_state(&self) -> Option<&ChartState> {
        match &self.body {
            Some(Body::Chart(s)) => Some(s),
            _ => None,
        }
    }

    /// The marks laid out for the plot's size last frame.
    fn chart_geometry(&self, rem: f32) -> Option<(Chart, Option<Axes>, Geometry, Rect)> {
        let s = self.chart_state()?;
        let shown = Chart::lerp(&s.from, &s.chart, s.progress);
        let axes = Axes::of(&s.chart);
        let plot = plot_rect(s.bounds.get().size, rem, s.chart.kind.is_round());
        let sweep = if s.opening { s.progress } else { 1. };
        let geometry = Geometry::new(&shown, axes.as_ref(), plot, sweep);
        Some((shown, axes, geometry, plot))
    }

    fn chart_pointer(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rem = f32::from(window.rem_size()) / 16.;
        let Some((_, _, geometry, _)) = self.chart_geometry(rem) else {
            return;
        };
        let Some(s) = state(self) else { return };
        let local = event.position - s.bounds.get().origin;
        let hover = geometry.hit(
            PlotPoint {
                x: f32::from(local.x),
                y: f32::from(local.y),
            },
            REACH * rem,
        );
        s.pointer = local;
        if hover != s.hover || hover.is_some() {
            s.hover = hover;
            cx.notify();
        }
    }

    fn chart_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rem = f32::from(window.rem_size()) / 16.;
        let Some((_, _, geometry, _)) = self.chart_geometry(rem) else {
            return;
        };
        window.focus(&self.focus, cx);
        let Some(s) = state(self) else { return };
        s.selected = s
            .hover
            .and_then(|i| geometry.marks.get(i))
            .map(|m| (m.series, m.index));
        cx.notify();
    }

    /// ←/→ walk the selected series' values; Escape clears.
    pub(super) fn chart_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(s) = state(self) else { return false };
        let Some((series, index)) = s.selected else {
            return false;
        };
        let count = s
            .chart
            .series
            .get(series)
            .map_or(0, |s| s.values.len().max(s.points.len()));
        s.selected = match keystroke.key.as_str() {
            "left" => Some((series, index.saturating_sub(1))),
            "right" => Some((series, (index + 1).min(count.saturating_sub(1)))),
            "escape" => None,
            _ => return false,
        };
        cx.notify();
        true
    }

    fn ask_point(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.chart_state() else { return };
        let Some((title, value)) = s.selected.and_then(|(si, i)| s.chart.mark_text(si, i)) else {
            return;
        };
        self.ask(&format!("{title} ({value})"), cx);
    }

    pub(super) fn render_chart(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rem = f32::from(window.rem_size()) / 16.;
        let progress = match state(self) {
            Some(s) => s.tween.progress(window, cx),
            None => return div().into_any_element(),
        };
        if let Some(s) = state(self) {
            s.progress = progress;
        }
        let Some((shown, axes, geometry, plot)) = self.chart_geometry(rem) else {
            return div().into_any_element();
        };
        let Some(s) = self.chart_state() else {
            return div().into_any_element();
        };
        let target = &s.chart;
        let r = |v: f32| px(v);

        // Legend: series, or a pie's labels.
        let legend: Vec<(String, Hsla)> = if target.kind.is_round() {
            target
                .labels
                .iter()
                .enumerate()
                .map(|(i, l)| (l.clone(), series_color(i)))
                .collect()
        } else {
            target
                .series
                .iter()
                .enumerate()
                .map(|(i, s)| (s.name.clone(), series_color(i)))
                .collect()
        };
        let show_legend = legend.len() > 1 || target.kind.is_round();

        let mut labels: Vec<AnyElement> = Vec::new();
        let text = |size: f32| {
            div()
                .absolute()
                .text_size(rems(size / 16.))
                .text_color(ui::faint())
        };
        if let Some(axes) = &axes {
            for tick in &axes.y.ticks {
                let y = plot.y + plot.h * (1. - axes.y.fraction(*tick));
                labels.push(
                    text(10.5)
                        .left(r(0.))
                        .w(r(plot.x - 6. * rem))
                        .top(r(y - 7. * rem))
                        .flex()
                        .justify_end()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(10.5))
                        .child(short_number(*tick))
                        .into_any_element(),
                );
            }
            match &axes.x {
                Some(x_axis) => {
                    for tick in &x_axis.ticks {
                        let x = plot.x + plot.w * x_axis.fraction(*tick);
                        labels.push(
                            text(10.5)
                                .left(r(x - 30. * rem))
                                .w(r(60. * rem))
                                .top(r(plot.y + plot.h + 5. * rem))
                                .flex()
                                .justify_center()
                                .font_family(ui::mono())
                                .text_size(ui::code_rems(10.5))
                                .child(short_number(*tick))
                                .into_any_element(),
                        );
                    }
                }
                None => {
                    // Skip labels that would collide.
                    let n = geometry.columns.len().max(1);
                    let room = plot.w / n as f32;
                    let step = ((56. * rem) / room.max(1.)).ceil().max(1.) as usize;
                    for (i, x) in geometry.columns.iter().enumerate().step_by(step) {
                        let width = room * step as f32;
                        labels.push(
                            text(10.5)
                                .left(r(x - width / 2.))
                                .w(r(width))
                                .top(r(plot.y + plot.h + 5. * rem))
                                .flex()
                                .justify_center()
                                .truncate()
                                .child(target.labels.get(i).cloned().unwrap_or_default())
                                .into_any_element(),
                        );
                    }
                }
            }
        }

        // The hovered value.
        let tooltip = s
            .hover
            .and_then(|i| geometry.marks.get(i))
            .and_then(|mark| {
                let (title, value) = shown.mark_text(mark.series, mark.index)?;
                let color = if target.kind.is_round() {
                    series_color(mark.index)
                } else {
                    series_color(mark.series)
                };
                let bounds = s.bounds.get();
                let flip = f32::from(s.pointer.x) > f32::from(bounds.size.width) - 180. * rem;
                Some(
                    ui::island(cx)
                        .absolute()
                        .top(s.pointer.y + px(12. * rem))
                        .when(flip, |d| {
                            d.right(bounds.size.width - s.pointer.x + px(12. * rem))
                        })
                        .when(!flip, |d| d.left(s.pointer.x + px(12. * rem)))
                        .px_2p5()
                        .py_1p5()
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            h_flex()
                                .gap_1p5()
                                .text_size(rems(11. / 16.))
                                .text_color(ui::muted())
                                .child(div().size(rems(8. / 16.)).rounded_full().bg(color))
                                .child(title),
                        )
                        .child(
                            div()
                                .font_family(ui::mono())
                                .text_size(ui::code_rems(12.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(ui::fg())
                                .child(value),
                        ),
                )
            });

        // The selected value, to ask about.
        let details = s.selected.and_then(|(si, i)| {
            let (title, value) = target.mark_text(si, i)?;
            Some(
                ui::island(cx)
                    .absolute()
                    .top_2()
                    .right_2()
                    .p_3()
                    .gap_2()
                    .flex()
                    .flex_col()
                    .w(rems(220. / 16.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_size(rems(12. / 16.))
                            .text_color(ui::muted())
                            .child(title),
                    )
                    .child(
                        div()
                            .font_family(ui::mono())
                            .text_size(ui::code_rems(14.))
                            .text_color(ui::fg())
                            .child(value),
                    )
                    .child(
                        h_flex().gap_1().child(
                            div().debug_selector(|| "genui-chart-ask".into()).child(
                                Button::new("genui-chart-ask")
                                    .ghost()
                                    .xsmall()
                                    .text_color(ui::muted())
                                    .icon(Icon::new(IconName::MessageSquare))
                                    .label(crate::shared::i18n::t("Ask about this"))
                                    .on_click(cx.listener(|this, _, _, cx| this.ask_point(cx))),
                            ),
                        ),
                    ),
            )
        });

        let frame = Frame {
            chart: shown,
            axes,
            geometry,
            plot,
            hover: s.hover,
            selected: s.selected,
            rem,
        };
        let cell = s.bounds.clone();
        let painter = canvas(
            move |bounds, window, _| {
                let old = cell.get();
                cell.set(bounds);
                // The marks were laid out for the last size: lay them out
                // again for this one on the next frame.
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

        let titles = h_flex()
            .px_3()
            .pt_2()
            .gap_3()
            .min_h(rems(22. / 16.))
            .text_size(rems(11. / 16.))
            .text_color(ui::muted())
            .children(
                target
                    .y_title
                    .clone()
                    .map(|t| div().font_weight(FontWeight::MEDIUM).child(t)),
            )
            .child(div().flex_1())
            .when(show_legend, |d| {
                d.child(
                    h_flex()
                        .flex_wrap()
                        .justify_end()
                        .gap_x_3()
                        .gap_y_0p5()
                        .children(legend.into_iter().map(|(name, color)| {
                            h_flex()
                                .gap_1p5()
                                .child(div().size(rems(8. / 16.)).rounded_full().bg(color))
                                .child(name)
                        })),
                )
            });
        v_flex()
            .size_full()
            .child(titles)
            .child(
                div()
                    .id("genui-chart")
                    .debug_selector(|| "genui-chart".into())
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .cursor(if s.hover.is_some() {
                        CursorStyle::PointingHand
                    } else {
                        CursorStyle::Arrow
                    })
                    .on_mouse_move(cx.listener(Self::chart_pointer))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !hovered && let Some(s) = state(this) {
                            s.hover = None;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(|this, _, window, cx| this.chart_click(window, cx)))
                    .child(painter)
                    .children(labels)
                    .children(tooltip)
                    .children(details),
            )
            .when_some(target.x_title.clone(), |d, t| {
                d.child(
                    div()
                        .pb_1p5()
                        .flex()
                        .justify_center()
                        .text_size(rems(11. / 16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::muted())
                        .child(t),
                )
            })
            .into_any_element()
    }
}
