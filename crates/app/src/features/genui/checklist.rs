//! The checklist panel: rows the user checks off, renames, adds, removes,
//! indents and reorders (dragging the grip, or Alt-↑/↓). Rows have one
//! height, so a reorder glides each row by whole rows. With Run plan on,
//! a bar runs the steps (Run, Run next, Stop: sent at once) and rows show
//! each step's status and a Skip action.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gentle_core::genui::checklist::{Change, Checklist, Item, Run, Status};
use gentle_core::genui::edits::{Editable as _, Edits};
use gentle_core::genui::kanban::drop_index;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::motion;
use super::panel::{Body, CHROME, MAX_HEIGHT, PanelView};
use crate::shared::ui;

/// A row's height and a sub-item's indent (px at 100 %).
const ROW_H: f32 = 32.;
const INDENT: f32 = 26.;
const ADD_H: f32 = 36.;
/// The run bar over a run plan's rows.
const RUN_H: f32 = 36.;
/// A press that moves less than this (px) is a click, not a drag.
const DRAG_SLOP: f32 = 3.;

pub(super) struct TextEdit {
    placeholder: &'static str,
    language: gentle_core::i18n::Language,
    /// The item renamed, or the parent of the item being added (`None`: top level).
    pub target: Option<String>,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

pub(super) struct RowDrag {
    pub item: String,
    pub from: Point<Pixels>,
    pub now: Point<Pixels>,
    pub moved: bool,
    /// Where it would drop: the index among its siblings (without it).
    pub index: usize,
}

pub(super) struct ChecklistState {
    pub edits: Edits<Checklist>,
    pub selected: Option<String>,
    pub renaming: Option<TextEdit>,
    pub adding: Option<TextEdit>,
    pub drag: Option<RowDrag>,
    /// The list's bounds last frame, for drops.
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl ChecklistState {
    pub fn new(list: Checklist) -> Self {
        Self {
            edits: Edits::new(list),
            selected: None,
            renaming: None,
            adding: None,
            drag: None,
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    pub fn rebase(&mut self, list: Checklist) {
        self.edits.rebase(list);
        let working = self.edits.working();
        if self
            .selected
            .as_ref()
            .is_some_and(|id| working.item(id).is_none())
        {
            self.selected = None;
        }
        if self
            .renaming
            .as_ref()
            .and_then(|r| r.target.as_ref())
            .is_some_and(|id| working.item(id).is_none())
        {
            self.renaming = None;
        }
    }

    pub fn height(&self) -> f32 {
        let working = self.edits.working();
        let rows = working.count().max(1) as f32;
        let bar = if working.run.is_some() { RUN_H } else { 0. };
        (rows * ROW_H + ADD_H + bar + 12.).clamp(120., MAX_HEIGHT - CHROME)
    }

    /// What the rows show: the working list, or the drop preview while a
    /// row is dragged.
    fn shown(&self) -> Checklist {
        let working = self.edits.working();
        match &self.drag {
            Some(drag) if drag.moved => {
                let mut preview = working.clone();
                if let Some(place) = working.place(&drag.item) {
                    let parent = place.parent.map(str::to_string);
                    let order = working.moved(&drag.item, parent.as_deref(), drag.index);
                    let _ = preview.apply(&Change::Order {
                        parent,
                        items: order,
                    });
                }
                preview
            }
            _ => working.clone(),
        }
    }
}

/// Each row's place (indent, top) in reading order, px at 100 %.
fn places(list: &Checklist) -> HashMap<String, (f32, f32)> {
    list.flat()
        .into_iter()
        .enumerate()
        .map(|(i, (parent, item))| {
            let x = if parent.is_some() { INDENT } else { 0. };
            (item.id.clone(), (x, i as f32 * ROW_H))
        })
        .collect()
}

fn state(this: &mut PanelView) -> Option<&mut ChecklistState> {
    match &mut this.body {
        Some(Body::Checklist(s)) => Some(s),
        _ => None,
    }
}

impl PanelView {
    fn checklist(&self) -> Option<&ChecklistState> {
        match &self.body {
            Some(Body::Checklist(s)) => Some(s),
            _ => None,
        }
    }

    /// Apply `change`; rows that end up elsewhere glide there.
    pub(super) fn checklist_edit(&mut self, change: Change, cx: &mut Context<Self>) -> bool {
        let before = self.checklist().map(|s| places(s.edits.working()));
        let applied = self.edit_with(cx, |body| match body {
            Body::Checklist(s) => Some(s.edits.edit(&change)),
            _ => None,
        });
        if applied && let (Some(before), Some(s)) = (before, self.checklist()) {
            let after = places(s.edits.working());
            self.glides.shift_all(&before, &after);
        }
        applied
    }

    pub(super) fn toggle_item(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(done) = self
            .checklist()
            .and_then(|s| s.edits.working().item(id))
            .map(|i| !i.done)
        else {
            return;
        };
        self.checklist_edit(
            Change::Toggle {
                item: id.to_string(),
                done,
            },
            cx,
        );
    }

    /// The header's Run plan: turns the plan on or off (sent with the rest).
    pub(super) fn toggle_run_mode(&mut self, cx: &mut Context<Self>) {
        let Some(on) = self.checklist().map(|s| s.edits.working().run.is_some()) else {
            return;
        };
        self.checklist_edit(Change::RunMode { on: !on }, cx);
    }

    /// Run / Run next: the next pending step runs and the request goes to
    /// the agent at once, with any skips and reorders made before it.
    fn run_plan(&mut self, mode: Run, cx: &mut Context<Self>) {
        let next = self
            .checklist()
            .and_then(|s| s.edits.working().next_step())
            .map(|item| item.id.clone());
        let Some(next) = next else {
            self.refused = Some(crate::shared::i18n::Message::new("Nothing pending to run"));
            cx.notify();
            return;
        };
        if self.checklist_edit(
            Change::Run {
                mode,
                item: Some(next),
            },
            cx,
        ) {
            self.send(cx);
        }
    }

    /// Stop: the agent starts no further step; sent at once.
    fn stop_plan(&mut self, cx: &mut Context<Self>) {
        if self.checklist_edit(Change::Stop, cx) {
            self.send(cx);
        }
    }

    /// Skip a step, or take it back.
    fn skip_item(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(status) = self
            .checklist()
            .and_then(|s| s.edits.working().item(id))
            .map(|i| i.status)
        else {
            return;
        };
        let status = if status == Status::Skipped {
            Status::Pending
        } else {
            Status::Skipped
        };
        self.checklist_edit(
            Change::Status {
                item: id.to_string(),
                status,
            },
            cx,
        );
    }

    fn remove_item(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.checklist_edit(
            Change::Remove {
                item: id.to_string(),
            },
            cx,
        ) && let Some(s) = state(self)
            && s.selected.as_deref() == Some(id)
        {
            s.selected = None;
        }
    }

    /// Alt-↑/↓: one place up or down among its siblings.
    fn shift_item(&mut self, id: &str, by: isize, cx: &mut Context<Self>) {
        let Some(s) = self.checklist() else { return };
        let list = s.edits.working();
        let Some(place) = list.place(id) else { return };
        let siblings = list.ids(place.parent).len();
        let Some(index) = place.index.checked_add_signed(by).filter(|i| *i < siblings) else {
            return;
        };
        let parent = place.parent.map(str::to_string);
        let items = list.moved(id, parent.as_deref(), index);
        self.checklist_edit(Change::Order { parent, items }, cx);
    }

    /// Tab: under the previous top-level item (last); Shift-Tab: back to the
    /// top level, right after its parent.
    fn indent_item(&mut self, id: &str, deeper: bool, cx: &mut Context<Self>) {
        let Some(s) = self.checklist() else { return };
        let list = s.edits.working();
        let Some(place) = list.place(id) else { return };
        let change = match (place.parent, deeper) {
            (None, true) => {
                let Some(previous) = place.index.checked_sub(1).map(|i| &list.items[i]) else {
                    return;
                };
                let mut items = list.ids(Some(&previous.id));
                items.push(id.to_string());
                Change::Order {
                    parent: Some(previous.id.clone()),
                    items,
                }
            }
            (Some(parent), false) => {
                let at = list.place(parent).map_or(0, |p| p.index + 1);
                Change::Order {
                    parent: None,
                    items: list.moved(id, None, at),
                }
            }
            _ => return,
        };
        self.checklist_edit(change, cx);
    }

    fn text_edit(
        &mut self,
        target: Option<String>,
        text: String,
        placeholder: &'static str,
        adding: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextEdit {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::shared::i18n::t(placeholder))
                .default_value(text)
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } if adding => this.finish_add(true, window, cx),
                InputEvent::PressEnter { .. } => {
                    this.finish_rename_item(true, cx);
                    window.focus(&this.focus, cx);
                }
                InputEvent::Blur if adding => this.finish_add(false, window, cx),
                InputEvent::Blur => this.finish_rename_item(true, cx),
                _ => {}
            },
        );
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        TextEdit {
            placeholder,
            language: crate::shared::i18n::language(),
            target,
            input,
            _subscription: subscription,
        }
    }

    pub(super) fn start_rename_item(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.locked() {
            return;
        }
        let Some(label) = self
            .checklist()
            .and_then(|s| s.edits.working().item(id))
            .map(|i| i.label.clone())
        else {
            return;
        };
        let edit = self.text_edit(Some(id.to_string()), label, "", false, window, cx);
        if let Some(s) = state(self) {
            s.selected = Some(id.to_string());
            s.renaming = Some(edit);
        }
        cx.notify();
    }

    fn finish_rename_item(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(edit) = state(self).and_then(|s| s.renaming.take()) else {
            return;
        };
        cx.notify();
        let Some(id) = edit.target else { return };
        let text = edit.input.read(cx).value().trim().to_string();
        let unchanged = self
            .checklist()
            .and_then(|s| s.edits.working().item(&id))
            .is_none_or(|i| i.label == text);
        if keep && !text.is_empty() && !unchanged {
            self.checklist_edit(
                Change::Edit {
                    item: id,
                    label: Some(text),
                    note: None,
                },
                cx,
            );
        }
    }

    /// The add field at the bottom (or under `parent`).
    pub(super) fn start_add_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_add_under(None, window, cx);
    }

    fn start_add_under(
        &mut self,
        parent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = if parent.is_some() {
            "New sub-item, Enter to add"
        } else {
            "New item, Enter to add"
        };
        let edit = self.text_edit(parent, String::new(), placeholder, true, window, cx);
        if let Some(s) = state(self) {
            s.adding = Some(edit);
        }
        cx.notify();
    }

    /// Enter adds the item and keeps the field open for the next one.
    fn finish_add(&mut self, keep: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.checklist() else { return };
        let Some(edit) = &s.adding else { return };
        let label = edit.input.read(cx).value().trim().to_string();
        let parent = edit.target.clone();
        let input = edit.input.clone();
        if !keep || label.is_empty() {
            if let Some(s) = state(self) {
                s.adding = None;
            }
            cx.notify();
            return;
        }
        let id = s.edits.working().fresh_id(&label);
        let added = self.checklist_edit(
            Change::Add {
                item: Item::new(id.clone(), label),
                parent,
            },
            cx,
        );
        if added {
            self.glides.appear(&id);
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
    }

    fn select_item(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if let Some(s) = state(self) {
            s.selected = id;
        }
        cx.notify();
    }

    pub(super) fn checklist_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(s) = self.checklist() else {
            return false;
        };
        if keystroke.key == "escape" && (s.renaming.is_some() || s.adding.is_some()) {
            self.finish_rename_item(false, cx);
            if let Some(s) = state(self) {
                s.adding = None;
            }
            window.focus(&self.focus, cx);
            cx.notify();
            return true;
        }
        if self.typing(window, cx) {
            return false;
        }
        let Some(id) = s.selected.clone() else {
            return false;
        };
        if self.locked() && !matches!(keystroke.key.as_str(), "up" | "down" | "escape") {
            return false;
        }
        let alt = keystroke.modifiers.alt;
        match keystroke.key.as_str() {
            "space" => self.toggle_item(&id, cx),
            "enter" => self.start_rename_item(&id, window, cx),
            "backspace" | "delete" => self.remove_item(&id, cx),
            "up" if alt => self.shift_item(&id, -1, cx),
            "down" if alt => self.shift_item(&id, 1, cx),
            "up" | "down" => {
                let flat: Vec<String> = s
                    .edits
                    .working()
                    .flat()
                    .into_iter()
                    .map(|(_, i)| i.id.clone())
                    .collect();
                let at = flat.iter().position(|i| *i == id).unwrap_or(0);
                let next = if keystroke.key == "up" {
                    at.checked_sub(1)
                } else {
                    Some(at + 1).filter(|i| *i < flat.len())
                };
                if let Some(next) = next {
                    self.select_item(Some(flat[next].clone()), cx);
                }
            }
            "tab" => self.indent_item(&id, !keystroke.modifiers.shift, cx),
            "escape" => self.select_item(None, cx),
            _ => return false,
        }
        true
    }

    // ── Dragging a row ─────────────────────────────────────────────────────

    fn press_grip(
        &mut self,
        id: &str,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        window.focus(&self.focus, cx);
        let Some(s) = state(self) else { return };
        let index = s.edits.working().place(id).map_or(0, |p| p.index);
        s.selected = Some(id.to_string());
        s.drag = Some(RowDrag {
            item: id.to_string(),
            from: event.position,
            now: event.position,
            moved: false,
            index,
        });
        cx.notify();
    }

    fn drag_row(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let rem = f32::from(window.rem_size()) / 16.;
        let Some(s) = self.checklist() else { return };
        let Some(drag) = &s.drag else { return };
        let before = places(&s.shown());
        let list = s.edits.working();
        let Some(place) = list.place(&drag.item) else {
            return;
        };
        // Where the pointer is in the list (px at 100 %), and the dragged
        // row's siblings (without it) as slots of rows.
        let y = f32::from(event.position.y - s.bounds.get().origin.y) / rem;
        let flat = places(list);
        let slots: Vec<(f32, f32)> = list
            .ids(place.parent)
            .iter()
            .filter(|i| **i != drag.item)
            .filter_map(|i| {
                let top = flat.get(i)?.1;
                let rows = 1 + list.item(i).map_or(0, |it| {
                    if place.parent.is_none() {
                        it.items.len()
                    } else {
                        0
                    }
                });
                Some((top, rows as f32 * ROW_H))
            })
            .collect();
        let index = drop_index(y, &slots);
        let moved = drag.moved || {
            let d = event.position - drag.from;
            f32::from(d.x).hypot(f32::from(d.y)) >= DRAG_SLOP
        };
        if let Some(drag) = state(self).and_then(|s| s.drag.as_mut()) {
            drag.now = event.position;
            drag.moved = moved;
            drag.index = index;
        }
        // The other rows make room smoothly; the dragged one follows the pointer.
        let after = self.checklist().map(|s| places(&s.shown()));
        let held = self
            .checklist()
            .and_then(|s| s.drag.as_ref())
            .map(|d| d.item.clone());
        if let (Some(after), Some(id)) = (after, held) {
            let mut others = before;
            others.remove(&id);
            self.glides.shift_all(&others, &after);
        }
        cx.notify();
    }

    fn drop_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = state(self).and_then(|s| s.drag.take()) else {
            return;
        };
        cx.notify();
        if !drag.moved {
            return;
        }
        let Some(s) = self.checklist() else { return };
        let list = s.edits.working();
        let Some(place) = list.place(&drag.item) else {
            return;
        };
        let parent = place.parent.map(str::to_string);
        let items = list.moved(&drag.item, parent.as_deref(), drag.index);
        // The row lands from where it was let go.
        let rem = f32::from(window.rem_size()) / 16.;
        let held = places(list).get(&drag.item).copied();
        let dy = f32::from(drag.now.y - drag.from.y) / rem;
        if list.ids(parent.as_deref()) == items {
            if let Some((x, y)) = held {
                self.glides.shift(&drag.item, (x, y + dy), (x, y));
            }
            return;
        }
        if self.checklist_edit(Change::Order { parent, items }, cx)
            && let (Some((x, y)), Some(s)) = (held, self.checklist())
            && let Some(to) = places(s.edits.working()).get(&drag.item).copied()
        {
            self.glides.forget(&drag.item);
            self.glides.shift(&drag.item, (x, y + dy), to);
        }
    }

    // ── Drawing ────────────────────────────────────────────────────────────

    pub(super) fn render_checklist(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(s) = state(self) {
            for edit in s.adding.iter_mut().chain(s.renaming.iter_mut()) {
                crate::shared::i18n::sync_placeholder(
                    &edit.input,
                    edit.placeholder,
                    &mut edit.language,
                    window,
                    cx,
                );
            }
        }
        let animate = motion::allowed(window, cx);
        let rem = f32::from(window.rem_size()) / 16.;
        let Some(s) = self.checklist() else {
            return div().into_any_element();
        };
        let shown = s.shown();
        let dragging = s.drag.as_ref().filter(|d| d.moved).map(|d| {
            let dy = f32::from(d.now.y - d.from.y) / rem;
            let start = places(s.edits.working()).get(&d.item).map_or(0., |p| p.1);
            let slot = places(&shown).get(&d.item).map_or(0., |p| p.1);
            (d.item.clone(), start + dy - slot)
        });
        let mut rows: Vec<AnyElement> = Vec::new();
        for (parent, item) in shown.flat() {
            let lifted = dragging.as_ref().and_then(|(id, dy)| {
                let carried = *id == item.id || parent.is_some_and(|p| p.id == *id);
                carried.then_some(*dy)
            });
            rows.push(self.render_item_row(parent, item, lifted, animate, cx));
        }
        let empty = rows.is_empty();
        let cell = s.bounds.clone();
        let locked = self.locked();
        let run_bar = self.render_run_bar(cx);
        let add = match &s.adding {
            Some(edit) if edit.target.is_none() => ui::field()
                .h(rems(28. / 16.))
                .mx_3()
                .my_1()
                .child(Icon::new(IconName::Plus).xsmall().text_color(ui::faint()))
                .child(Input::new(&edit.input).appearance(false).xsmall())
                .into_any_element(),
            _ => h_flex()
                .id("genui-checklist-add")
                .debug_selector(|| "genui-checklist-add".into())
                .h(rems(ADD_H / 16.))
                .px_3()
                .gap_2()
                .cursor_pointer()
                .text_size(rems(12.5 / 16.))
                .text_color(ui::faint())
                .hover(|d| d.text_color(ui::muted()))
                .child(Icon::new(IconName::Plus).xsmall())
                .child(crate::shared::i18n::t("Add item"))
                .on_click(cx.listener(|this, _, window, cx| this.start_add_item(window, cx)))
                .into_any_element(),
        };
        v_flex()
            .id("genui-checklist")
            .size_full()
            .overflow_y_scroll()
            .py_1p5()
            .on_mouse_move(cx.listener(Self::drag_row))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.drop_row(window, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.drop_row(window, cx)),
            )
            .children(run_bar)
            .child(
                v_flex()
                    .relative()
                    .child(
                        canvas(move |bounds, _, _| cell.set(bounds), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .children(rows),
            )
            .when(empty, |d| {
                d.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(ui::muted())
                        .child(crate::shared::i18n::t("Nothing here yet.")),
                )
            })
            .when(!locked, |d| d.child(add))
            .into_any_element()
    }

    /// Progress and Run / Run next / Stop over a run plan's rows.
    fn render_run_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.locked() {
            return None;
        }
        let list = self.checklist()?.edits.working();
        let run = list.run?;
        let steps = list.steps();
        let done = steps
            .iter()
            .filter(|i| matches!(i.status, Status::Done | Status::Skipped))
            .count();
        let running = list.running();
        let busy = running.is_some() || matches!(run, Run::All | Run::Step);
        let sending = matches!(self.send, super::diagram::SendState::Sending { .. });
        let next = list.next_step();
        let status = match running {
            Some(item) => crate::shared::i18n::t("Running step {number} · {item}")
                .replace(
                    "{number}",
                    &list.step_number(&item.id).unwrap_or(0).to_string(),
                )
                .replace("{item}", &item.label),
            None if next.is_none() => crate::shared::i18n::t("Every step is through").to_string(),
            None => crate::shared::i18n::t("Next: step {number} · {item}")
                .replace(
                    "{number}",
                    &next
                        .and_then(|i| list.step_number(&i.id))
                        .unwrap_or(0)
                        .to_string(),
                )
                .replace("{item}", next.map_or("", |i| i.label.as_str())),
        };
        let button = |id: &'static str, icon: IconName, label: &'static str| {
            Button::new(id)
                .xsmall()
                .icon(Icon::new(icon))
                .label(crate::shared::i18n::t(label))
                .disabled(sending)
        };
        let controls = if busy {
            h_flex().child(
                div().debug_selector(|| "genui-run-stop".into()).child(
                    button("genui-run-stop", IconName::Square, "Stop")
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| this.stop_plan(cx))),
                ),
            )
        } else {
            h_flex()
                .gap_1()
                .child(
                    div().debug_selector(|| "genui-run-next".into()).child(
                        button("genui-run-next", IconName::SkipForward, "Run next")
                            .ghost()
                            .disabled(sending || next.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.run_plan(Run::Step, cx))),
                    ),
                )
                .child(
                    div().debug_selector(|| "genui-run-all".into()).child(
                        button("genui-run-all", IconName::Play, "Run")
                            .primary()
                            .disabled(sending || next.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.run_plan(Run::All, cx))),
                    ),
                )
        };
        Some(
            h_flex()
                .h(rems(RUN_H / 16.))
                .flex_shrink_0()
                .px_3()
                .gap_2()
                .border_b_1()
                .border_color(ui::border())
                .text_size(rems(12. / 16.))
                .child(ui::chip(
                    format!("{done}/{}", steps.len()),
                    if busy { ui::accent() } else { ui::muted() },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(ui::muted())
                        .child(status),
                )
                .child(controls)
                .into_any_element(),
        )
    }

    fn render_item_row(
        &self,
        parent: Option<&Item>,
        item: &Item,
        lifted: Option<f32>,
        animate: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(s) = self.checklist() else {
            return div().into_any_element();
        };
        let selected = s.selected.as_deref() == Some(item.id.as_str());
        let renaming = s
            .renaming
            .as_ref()
            .filter(|r| r.target.as_deref() == Some(item.id.as_str()));
        let adding_here = s
            .adding
            .as_ref()
            .filter(|a| a.target.as_deref() == Some(item.id.as_str()));
        let (toggle_id, select_id, rename_id, grip_id, ask_id, remove_id, add_id) = (
            item.id.clone(),
            item.id.clone(),
            item.id.clone(),
            item.id.clone(),
            item.id.clone(),
            item.id.clone(),
            item.id.clone(),
        );
        let label = item.label.clone();
        let locked = self.locked();
        let run_mode = s.edits.working().run.is_some();
        let skip_id = item.id.clone();
        let check_selector = format!("genui-item-check-{}", item.id);
        let row_selector = format!("genui-item-{}", item.id);
        let (icon, color) = match item.status {
            Status::Done => (IconName::CircleCheck, ui::success()),
            Status::Running => (IconName::LoaderCircle, ui::accent()),
            Status::Failed => (IconName::CircleX, ui::danger()),
            Status::Skipped => (IconName::Ban, ui::faint()),
            Status::Pending => (IconName::Circle, ui::faint()),
        };
        let check = div()
            .id(SharedString::from(format!("genui-item-check-{}", item.id)))
            .debug_selector(move || check_selector)
            .size(rems(18. / 16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .when(!locked, |d| d.cursor_pointer())
            .child(Icon::new(icon).small().text_color(color))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_item(&toggle_id, cx)));
        // A fresh check pops in.
        let check = if animate && item.done {
            check
                .with_animation(
                    SharedString::from(format!("genui-item-done-{}", item.id)),
                    Animation::new(std::time::Duration::from_millis(260)),
                    |d, t| {
                        let s = ui::spring(t);
                        d.opacity(s.min(1.))
                            .size(rems((18. * (0.6 + 0.4 * s)) / 16.))
                    },
                )
                .into_any_element()
        } else {
            check.into_any_element()
        };
        let progress = (!item.items.is_empty()).then(|| {
            let done = item.items.iter().filter(|i| i.done).count();
            format!("{done}/{}", item.items.len())
        });
        let title: AnyElement = match renaming {
            Some(edit) => ui::field()
                .flex_1()
                .h(rems(26. / 16.))
                .border_color(ui::accent().opacity(0.6))
                .child(Input::new(&edit.input).appearance(false).xsmall())
                .into_any_element(),
            None => h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(
                    div()
                        .min_w_0()
                        .flex_shrink(1.)
                        .truncate()
                        .text_color(if item.done || item.status == Status::Skipped {
                            ui::faint()
                        } else {
                            ui::fg()
                        })
                        .when(item.done || item.status == Status::Skipped, |d| {
                            d.line_through()
                        })
                        .child(item.label.clone()),
                )
                .children(item.code_ref.as_ref().map(|code_ref| {
                    super::refs::chip(
                        format!("genui-item-ref-{}", item.id),
                        code_ref,
                        &self.env,
                        cx,
                    )
                }))
                .children(item.note.clone().map(|note| {
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_size(rems(11.5 / 16.))
                        .text_color(ui::muted())
                        .child(note)
                }))
                .into_any_element(),
        };
        let action = |id: SharedString, icon: IconName, tip: &'static str| {
            div()
                .id(id)
                .size(rems(22. / 16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .text_color(ui::muted())
                .hover(|d| d.text_color(ui::fg()).bg(ui::surface_hover()))
                .child(Icon::new(icon).xsmall())
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(crate::shared::i18n::t(tip))
                        .build(window, cx)
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let actions = h_flex()
            .flex_shrink_0()
            .gap_0p5()
            .opacity(0.)
            .group_hover("genui-item", |d| d.opacity(1.))
            .when(run_mode && !locked && item.items.is_empty(), |d| {
                d.child(
                    action(
                        SharedString::from(format!("genui-item-skip-{}", item.id)),
                        IconName::Ban,
                        if item.status == Status::Skipped {
                            "Don't skip"
                        } else {
                            "Skip this step"
                        },
                    )
                    .debug_selector({
                        let selector = format!("genui-item-skip-{}", item.id);
                        move || selector
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.skip_item(&skip_id, cx))),
                )
            })
            .when(parent.is_none() && !locked, |d| {
                d.child(
                    action(
                        SharedString::from(format!("genui-item-sub-{}", item.id)),
                        IconName::Plus,
                        "Add a sub-item",
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_add_under(Some(add_id.clone()), window, cx)
                    })),
                )
            })
            .child(
                action(
                    SharedString::from(format!("genui-item-ask-{}", item.id)),
                    IconName::MessageSquare,
                    "Ask about this",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ask(&format!("the item “{label}” ({ask_id})"), cx)
                })),
            )
            .when(!locked, |d| {
                d.child(
                    action(
                        SharedString::from(format!("genui-item-remove-{}", item.id)),
                        IconName::Trash,
                        "Remove",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.remove_item(&remove_id, cx))),
                )
            });
        let row = h_flex()
            .id(SharedString::from(format!("genui-item-{}", item.id)))
            .debug_selector(move || row_selector)
            .group("genui-item")
            .h(rems(ROW_H / 16.))
            .flex_shrink_0()
            .pl(rems(
                (8. + if parent.is_some() { INDENT } else { 0. }) / 16.,
            ))
            .pr_2()
            .gap_1p5()
            .text_size(rems(13. / 16.))
            .bg(ui::surface())
            .when(selected, |d| d.bg(ui::accent().opacity(0.08)))
            .hover(|d| d.bg(ui::surface_hover()))
            .when(!locked, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("genui-item-grip-{}", item.id)))
                        .debug_selector({
                            let selector = format!("genui-item-grip-{}", item.id);
                            move || selector
                        })
                        .flex_shrink_0()
                        .cursor(CursorStyle::OpenHand)
                        .text_color(ui::faint())
                        .opacity(0.)
                        .group_hover("genui-item", |d| d.opacity(1.))
                        .when(selected, |d| d.opacity(1.))
                        .child(Icon::new(IconName::GripVertical).xsmall())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.press_grip(&grip_id, event, window, cx)
                            }),
                        ),
                )
            })
            .child(check)
            .child(title)
            .children(progress.map(|p| {
                div()
                    .flex_shrink_0()
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(11.))
                    .text_color(ui::faint())
                    .child(p)
            }))
            .child(actions)
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus, cx);
                if event.click_count() >= 2 {
                    this.start_rename_item(&rename_id, window, cx);
                } else {
                    this.select_item(Some(select_id.clone()), cx);
                }
            }));
        let row = match lifted {
            // Held by the pointer, above the others.
            Some(dy) => row
                .relative()
                .top(rems(dy / 16.))
                .shadow_lg()
                .rounded_md()
                .bg(ui::raised())
                .into_any_element(),
            None => motion::placed(row, self.glides.at(&item.id)).into_any_element(),
        };
        match adding_here {
            Some(edit) => v_flex()
                .child(row)
                .child(
                    ui::field()
                        .h(rems(28. / 16.))
                        .ml(rems((8. + INDENT) / 16.))
                        .mr_3()
                        .my_1()
                        .child(Icon::new(IconName::Plus).xsmall().text_color(ui::faint()))
                        .child(Input::new(&edit.input).appearance(false).xsmall()),
                )
                .into_any_element(),
            None => row,
        }
    }
}
