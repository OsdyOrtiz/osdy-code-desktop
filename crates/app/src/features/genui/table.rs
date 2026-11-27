//! The table panel: a filter field, sortable column headers and rows the
//! user selects (and, when the agent allows it, edits in place). Sorting
//! and filtering only change the view; the selection and cell edits are
//! the changes sent to the agent.

use std::collections::HashMap;
use std::time::Duration;

use gentle_core::genui::edits::Edits;
use gentle_core::genui::table::{
    self, Align, Change, ColumnType, Sort, Table, Tone, cell_text, link_url, status_tone,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::motion;
use super::panel::{Body, CHROME, MAX_HEIGHT, PanelView};
use crate::shared::ui;

/// A row's height (px at 100 %): rows glide by whole rows when re-sorted.
const ROW_H: f32 = 32.;
/// The Code column's width (px at 100 %), when rows point at code.
const REF_W: f32 = 132.;
const HEAD_H: f32 = 30.;
const TOOLBAR_H: f32 = 40.;

pub(super) struct CellEdit {
    pub row: String,
    pub column: String,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

pub(super) struct TableState {
    pub edits: Edits<Table>,
    /// The view's sort (starts as the agent's).
    pub sort: Option<Sort>,
    pub filter: Option<Entity<InputState>>,
    filter_language: gentle_core::i18n::Language,
    pub filter_text: String,
    pub editing: Option<CellEdit>,
    _filter_subscription: Option<Subscription>,
}

impl TableState {
    pub fn new(table: Table) -> Self {
        Self {
            sort: table.sort.clone(),
            edits: Edits::new(table),
            filter: None,
            filter_language: crate::shared::i18n::language(),
            filter_text: String::new(),
            editing: None,
            _filter_subscription: None,
        }
    }

    pub fn rebase(&mut self, table: Table) {
        if table.sort != self.edits.base().sort {
            self.sort = table.sort.clone();
        }
        self.edits.rebase(table);
        if let Some(edit) = &self.editing
            && self.edits.working().row(&edit.row).is_none()
        {
            self.editing = None;
        }
    }

    pub fn height(&self) -> f32 {
        let rows = self.edits.working().rows.len().clamp(2, 10) as f32;
        (TOOLBAR_H + HEAD_H + rows * ROW_H + 4.).min(MAX_HEIGHT - CHROME)
    }

    fn visible(&self) -> Vec<usize> {
        self.edits
            .working()
            .visible_rows(self.sort.as_ref(), &self.filter_text)
    }
}

fn tone_color(tone: Tone) -> Hsla {
    match tone {
        Tone::Success => ui::success(),
        Tone::Danger => ui::danger(),
        Tone::Warning => ui::warning(),
        Tone::Info => ui::info(),
        Tone::Muted => ui::muted(),
    }
}

/// A column's width (px at 100 %): typed columns are as wide as their
/// values; text columns share the rest (all share it when none is text).
fn sized<E: Styled>(element: E, kind: ColumnType, flexible: bool) -> E {
    let width = match kind {
        _ if flexible => None,
        ColumnType::Number => Some(96.),
        ColumnType::Date => Some(112.),
        ColumnType::Status => Some(136.),
        ColumnType::Link => Some(110.),
        ColumnType::Text => None,
    };
    match width {
        Some(w) => element.w(rems(w / 16.)).flex_shrink_0(),
        None => element.flex_1().min_w_0(),
    }
}

fn state(this: &mut PanelView) -> Option<&mut TableState> {
    match &mut this.body {
        Some(Body::Table(s)) => Some(s),
        _ => None,
    }
}

impl PanelView {
    fn table(&self) -> Option<&TableState> {
        match &self.body {
            Some(Body::Table(s)) => Some(s),
            _ => None,
        }
    }

    fn table_edit(&mut self, change: Change, cx: &mut Context<Self>) -> bool {
        self.edit_with(cx, |body| match body {
            Body::Table(s) => Some(s.edits.edit(&change)),
            _ => None,
        })
    }

    /// Header click: ascending, descending, then the agent's order.
    pub(super) fn cycle_sort(&mut self, column: &str, cx: &mut Context<Self>) {
        let Some(s) = state(self) else { return };
        let before = s.visible();
        s.sort = match &s.sort {
            Some(sort) if sort.column == column && !sort.descending => Some(Sort {
                column: column.to_string(),
                descending: true,
            }),
            Some(sort) if sort.column == column => None,
            _ => Some(Sort {
                column: column.to_string(),
                descending: false,
            }),
        };
        let after = s.visible();
        let places = |order: &[usize], table: &Table| -> HashMap<String, (f32, f32)> {
            order
                .iter()
                .enumerate()
                .map(|(i, &ix)| (table.rows[ix].id.clone(), (0., i as f32 * ROW_H)))
                .collect()
        };
        let (old, new) = {
            let table = s.edits.working();
            (places(&before, table), places(&after, table))
        };
        self.glides.shift_all(&old, &new);
        cx.notify();
    }

    pub(super) fn toggle_row(&mut self, row: &str, cx: &mut Context<Self>) {
        let Some(s) = self.table() else { return };
        let table = s.edits.working();
        if !table.selectable {
            return;
        }
        let mut rows = table.selection.clone();
        match rows.iter().position(|r| r == row) {
            Some(i) => {
                rows.remove(i);
            }
            None => rows.push(row.to_string()),
        }
        self.table_edit(Change::Select { rows }, cx);
    }

    /// The header checkbox: select every shown row, or clear the selection.
    fn toggle_all(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.table() else { return };
        let table = s.edits.working();
        let shown: Vec<String> = s
            .visible()
            .into_iter()
            .map(|ix| table.rows[ix].id.clone())
            .collect();
        let all = !shown.is_empty() && shown.iter().all(|id| table.selection.contains(id));
        let rows = if all {
            table
                .selection
                .iter()
                .filter(|id| !shown.contains(id))
                .cloned()
                .collect()
        } else {
            let mut rows = table.selection.clone();
            rows.extend(shown.into_iter().filter(|id| !table.selection.contains(id)));
            rows
        };
        self.table_edit(Change::Select { rows }, cx);
    }

    pub(super) fn start_cell_edit(
        &mut self,
        row: &str,
        column: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(s) = self.table() else { return };
        let table = s.edits.working();
        if !table.editable {
            return;
        }
        let current = cell_text(table.cell(row, column));
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.finish_cell_edit(true, cx);
                    window.focus(&this.focus, cx);
                }
                InputEvent::Blur => this.finish_cell_edit(true, cx),
                _ => {}
            },
        );
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        if let Some(s) = state(self) {
            s.editing = Some(CellEdit {
                row: row.to_string(),
                column: column.to_string(),
                input,
                _subscription: subscription,
            });
        }
        cx.notify();
    }

    fn finish_cell_edit(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(edit) = state(self).and_then(|s| s.editing.take()) else {
            return;
        };
        cx.notify();
        if !keep {
            return;
        }
        let text = edit.input.read(cx).value().to_string();
        let Some(s) = self.table() else { return };
        let table = s.edits.working();
        let value = match table.value_from_text(&edit.column, &text) {
            Ok(value) => value,
            Err(error) => {
                self.refused = Some(crate::shared::i18n::Message::raw(error));
                return;
            }
        };
        if table.cell(&edit.row, &edit.column) == &value {
            return;
        }
        self.table_edit(
            Change::EditCell {
                row: edit.row,
                column: edit.column,
                value,
            },
            cx,
        );
    }

    pub(super) fn table_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if keystroke.key == "escape" && self.table().is_some_and(|s| s.editing.is_some()) {
            self.finish_cell_edit(false, cx);
            window.focus(&self.focus, cx);
            return true;
        }
        false
    }

    fn ensure_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = state(self) else { return };
        if let Some(filter) = &s.filter {
            crate::shared::i18n::sync_placeholder(
                filter,
                "Filter rows…",
                &mut s.filter_language,
                window,
                cx,
            );
            return;
        }
        s.filter_language = crate::shared::i18n::language();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::shared::i18n::t("Filter rows…"))
        });
        let subscription = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                if let Some(s) = state(this) {
                    s.filter_text = text;
                }
                cx.notify();
            }
        });
        if let Some(s) = state(self) {
            s.filter = Some(input);
            s._filter_subscription = Some(subscription);
        }
    }

    pub(super) fn render_table(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.ensure_filter(window, cx);
        let animate = motion::allowed(window, cx);
        let Some(s) = self.table() else {
            return div().into_any_element();
        };
        let table = s.edits.working();
        let visible = s.visible();
        let selectable = table.selectable;
        let flexible = !table.columns.iter().any(|c| c.kind == ColumnType::Text);
        let all_selected = !visible.is_empty()
            && visible
                .iter()
                .all(|&ix| table.selection.contains(&table.rows[ix].id));

        let toolbar = h_flex()
            .h(rems(TOOLBAR_H / 16.))
            .px_3()
            .gap_2()
            .flex_shrink_0()
            .children(s.filter.as_ref().map(|filter| {
                ui::field()
                    .flex_1()
                    .max_w(rems(280. / 16.))
                    .h(rems(28. / 16.))
                    .child(Icon::new(IconName::Search).xsmall().text_color(ui::faint()))
                    .child(Input::new(filter).appearance(false).xsmall())
            }))
            .child(div().flex_1())
            .child(
                div()
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(11.))
                    .text_color(ui::faint())
                    .child(if visible.len() == table.rows.len() {
                        crate::shared::i18n::t("{count} rows")
                            .replace("{count}", &table.rows.len().to_string())
                    } else {
                        crate::shared::i18n::t("{visible} of {total} rows")
                            .replace("{visible}", &visible.len().to_string())
                            .replace("{total}", &table.rows.len().to_string())
                    }),
            );

        let sort = s.sort.clone();
        let header = h_flex()
            .h(rems(HEAD_H / 16.))
            .px_3()
            .gap_2()
            .flex_shrink_0()
            .bg(ui::well())
            .text_size(rems(11. / 16.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(ui::muted())
            .when(selectable, |d| {
                d.child(
                    div().w(rems(18. / 16.)).flex_shrink_0().child(
                        Checkbox::new("genui-table-all")
                            .xsmall()
                            .checked(all_selected)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx))),
                    ),
                )
            })
            .children(table.columns.iter().enumerate().map(|(i, column)| {
                let key = column.key.clone();
                let arrow = sort.as_ref().filter(|s| s.column == column.key).map(|s| {
                    if s.descending {
                        IconName::ArrowDown
                    } else {
                        IconName::ArrowUp
                    }
                });
                let selector = format!("genui-th-{}", column.key);
                h_flex()
                    .id(("genui-th", i))
                    .debug_selector(move || selector)
                    .map(|d| sized(d, column.kind, flexible))
                    .gap_1()
                    .cursor_pointer()
                    .when(column.align == Align::Right, |d| d.justify_end())
                    .when(column.align == Align::Center, |d| d.justify_center())
                    .hover(|d| d.text_color(ui::fg()))
                    .when(arrow.is_some(), |d| d.text_color(ui::fg()))
                    .child(div().min_w_0().truncate().child(column.label.clone()))
                    .children(arrow.map(|a| Icon::new(a).xsmall().text_color(ui::accent())))
                    .on_click(cx.listener(move |this, _, _, cx| this.cycle_sort(&key, cx)))
            }))
            .when(table.rows.iter().any(|r| r.code_ref.is_some()), |d| {
                d.child(
                    div()
                        .w(rems(REF_W / 16.))
                        .flex_shrink_0()
                        .child(crate::shared::i18n::t("Code")),
                )
            })
            .child(div().w(rems(22. / 16.)).flex_shrink_0());

        // Only the rows in view are built: rows have one fixed height.
        let rows = if visible.is_empty() {
            div()
                .id("genui-table-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    div()
                        .p_4()
                        .text_sm()
                        .text_color(ui::muted())
                        .child(crate::shared::i18n::t("No rows match the filter.")),
                )
                .into_any_element()
        } else {
            uniform_list(
                "genui-table-rows",
                visible.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    range
                        .map(|i| this.render_row(visible[i], animate, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .flex_1()
            .min_h_0()
            .into_any_element()
        };
        v_flex()
            .size_full()
            .child(toolbar)
            .child(header)
            .child(rows)
            .into_any_element()
    }

    fn render_row(&self, ix: usize, animate: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.table() else {
            return div().into_any_element();
        };
        let table = s.edits.working();
        let row = &table.rows[ix];
        let selected = table.selection.contains(&row.id);
        let flexible = !table.columns.iter().any(|c| c.kind == ColumnType::Text);
        let id = row.id.clone();
        let ask_id = row.id.clone();
        let selector = format!("genui-row-{}", row.id);
        let mut element = h_flex()
            .id(SharedString::from(format!("genui-row-{}", row.id)))
            .debug_selector(move || selector)
            .group("genui-row")
            .w_full()
            .h(rems(ROW_H / 16.))
            .flex_shrink_0()
            .px_3()
            .gap_2()
            .border_b_1()
            .border_color(ui::border().opacity(0.6))
            .text_size(rems(12.5 / 16.))
            // In an editable table a click belongs to the cell (double-click
            // edits); rows are selected with their checkbox.
            .when(table.selectable && !table.editable, |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_row(&id, cx)))
            })
            .hover(|d| d.bg(ui::surface_hover()));
        if selected {
            element = element.bg(ui::accent().opacity(0.10));
        }
        if table.selectable {
            let id = row.id.clone();
            let selector = format!("genui-check-{}", row.id);
            element = element.child(
                div()
                    .debug_selector(move || selector)
                    .w(rems(18. / 16.))
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Checkbox::new(SharedString::from(format!("genui-check-{}", row.id)))
                            .xsmall()
                            .checked(selected)
                            .on_click(cx.listener(move |this, _, _, cx| this.toggle_row(&id, cx))),
                    ),
            );
        }
        for column in &table.columns {
            let editing = s
                .editing
                .as_ref()
                .filter(|e| e.row == row.id && e.column == column.key);
            let cell = match editing {
                Some(edit) => ui::field()
                    .w_full()
                    .h(rems(26. / 16.))
                    .border_color(ui::accent().opacity(0.6))
                    .child(Input::new(&edit.input).appearance(false).xsmall())
                    .into_any_element(),
                None => self.render_cell(&row.id, column, cx),
            };
            let (row_id, key) = (row.id.clone(), column.key.clone());
            let cell_selector = format!("genui-cell-{}-{}", row.id, column.key);
            element = element.child(
                div()
                    .id(SharedString::from(format!(
                        "genui-cell-{}-{}",
                        row.id, column.key
                    )))
                    .debug_selector(move || cell_selector)
                    .map(|d| sized(d, column.kind, flexible))
                    .flex()
                    .items_center()
                    .when(column.align == Align::Right, |d| d.justify_end())
                    .when(column.align == Align::Center, |d| d.justify_center())
                    .child(cell)
                    .when(table.editable, |d| {
                        d.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            if event.click_count() >= 2 {
                                cx.stop_propagation();
                                this.start_cell_edit(&row_id, &key, window, cx);
                            }
                        }))
                    }),
            );
        }
        if table.rows.iter().any(|r| r.code_ref.is_some()) {
            element = element.child(div().w(rems(REF_W / 16.)).flex_shrink_0().children(
                row.code_ref.as_ref().map(|code_ref| {
                    super::refs::chip(format!("genui-row-ref-{}", row.id), code_ref, &self.env, cx)
                }),
            ));
        }
        let label = table.row_label(&row.id);
        element = element.child(
            div()
                .id(SharedString::from(format!("genui-ask-{}", row.id)))
                .w(rems(22. / 16.))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .opacity(0.)
                .group_hover("genui-row", |d| d.opacity(1.))
                .cursor_pointer()
                .text_color(ui::muted())
                .hover(|d| d.text_color(ui::fg()))
                .child(Icon::new(IconName::MessageSquare).xsmall())
                .tooltip(|window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(crate::shared::i18n::t(
                        "Ask about this row",
                    ))
                    .build(window, cx)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.ask(&format!("the row “{label}” ({ask_id})"), cx);
                })),
        );
        let place = self.glides.at(&row.id);
        let element = motion::placed(element, place);
        if animate && selected {
            // The selection washes in.
            element
                .with_animation(
                    SharedString::from(format!("genui-row-on-{}", row.id)),
                    Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
                    |d, t| d.bg(ui::accent().opacity(0.10 * t)),
                )
                .into_any_element()
        } else {
            element.into_any_element()
        }
    }

    fn render_cell(&self, row: &str, column: &table::Column, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.table() else {
            return div().into_any_element();
        };
        let value = s.edits.working().cell(row, &column.key);
        let text = cell_text(value);
        if text.is_empty() {
            return div().text_color(ui::faint()).child("—").into_any_element();
        }
        match column.kind {
            ColumnType::Status => ui::chip(text.clone(), tone_color(status_tone(&text)))
                .max_w_full()
                .truncate()
                .into_any_element(),
            ColumnType::Number => div()
                .font_family(ui::mono())
                .text_size(ui::code_rems(12.5))
                .text_color(ui::fg())
                .truncate()
                .child(text)
                .into_any_element(),
            ColumnType::Date => div()
                .font_family(ui::mono())
                .text_size(ui::code_rems(12.5))
                .text_color(ui::muted())
                .truncate()
                .child(text)
                .into_any_element(),
            ColumnType::Link => match link_url(value).map(str::to_string) {
                Some(url) => div()
                    .id(SharedString::from(format!(
                        "genui-link-{row}-{}",
                        column.key
                    )))
                    .min_w_0()
                    .truncate()
                    .text_color(ui::accent())
                    .cursor_pointer()
                    .hover(|d| d.underline())
                    .child(text)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.stop_propagation();
                        cx.open_url(&url);
                    }))
                    .into_any_element(),
                None => div().truncate().child(text).into_any_element(),
            },
            ColumnType::Text => div()
                .min_w_0()
                .truncate()
                .text_color(ui::fg())
                .child(text)
                .into_any_element(),
        }
    }
}
