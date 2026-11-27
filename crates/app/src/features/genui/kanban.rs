//! The kanban panel: columns of cards laid out here (not by flex), so a
//! card dragged between columns, the gap opening where it will land and
//! the drop itself all glide. Click a card for its details (edit, ask,
//! delete); double-click to edit; Alt-arrows move the selected card. A
//! card moved into a `starts` column goes to the agent at once, which
//! dispatches a subagent for it; a card with an `agentId` shows that
//! subagent's live status, and its details can stop it.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gentle_core::genui::edits::{Editable as _, Edits};
use gentle_core::genui::kanban::{Board, Card, CardPatch, Change, drop_index};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::diagram::DiagramEvent;
use super::live::AgentStatus;
use super::panel::{Body, CHROME, MAX_HEIGHT, PanelView};
use crate::shared::ui;

/// Board geometry, px at 100 %.
const PAD: f32 = 12.;
const COL_W: f32 = 236.;
const COL_GAP: f32 = 12.;
const HEAD_H: f32 = 34.;
const CARD_GAP: f32 = 8.;
const ADD_H: f32 = 30.;
const CARD_PAD: f32 = 10.;
/// A press that moves less than this (px) is a click, not a drag.
const DRAG_SLOP: f32 = 4.;

fn card_height(card: &Card) -> f32 {
    let mut h = CARD_PAD * 2. + 18.;
    if card.description.is_some() {
        h += 20.;
    }
    if !card.tags.is_empty() || card.assignee.is_some() || card.code_ref.is_some() {
        h += 26.;
    }
    if card.agent.is_some() || card.agent_id.is_some() {
        h += 20.;
    }
    h
}

/// A subagent's status color: working, finished, or stopped short.
fn agent_color(status: &AgentStatus) -> Hsla {
    match status.status.as_str() {
        _ if status.active => ui::accent(),
        "completed" | "done" | "succeeded" => ui::success(),
        "failed" | "error" | "cancelled" | "canceled" | "timed_out" | "interrupted" => ui::danger(),
        _ => ui::muted(),
    }
}

/// Where everything sits on the board (px at 100 %, from its top-left).
struct Layout {
    /// Each column's x and the bottom of its cards.
    columns: Vec<(f32, f32)>,
    /// Each card's top-left and height.
    cards: HashMap<String, (f32, f32, f32)>,
    width: f32,
    height: f32,
}

fn layout(board: &Board, without: Option<&str>) -> Layout {
    let mut columns = Vec::new();
    let mut cards = HashMap::new();
    let mut bottom_max: f32 = 0.;
    for (ci, column) in board.columns.iter().enumerate() {
        let x = PAD + ci as f32 * (COL_W + COL_GAP);
        let mut y = PAD + HEAD_H;
        for card in board.cards.iter().filter(|c| c.column == column.id) {
            if Some(card.id.as_str()) == without {
                continue;
            }
            let h = card_height(card);
            cards.insert(card.id.clone(), (x + 8., y, h));
            y += h + CARD_GAP;
        }
        bottom_max = bottom_max.max(y + ADD_H);
        columns.push((x, y));
    }
    let n = board.columns.len().max(1) as f32;
    Layout {
        columns,
        cards,
        width: PAD * 2. + n * COL_W + (n - 1.) * COL_GAP,
        height: bottom_max + PAD,
    }
}

fn places(layout: &Layout) -> HashMap<String, (f32, f32)> {
    layout
        .cards
        .iter()
        .map(|(id, &(x, y, _))| (id.clone(), (x, y)))
        .collect()
}

pub(super) struct CardDrag {
    pub card: String,
    pub from: Point<Pixels>,
    pub now: Point<Pixels>,
    pub moved: bool,
    /// The pointer inside the card (px at 100 %).
    pub grab: (f32, f32),
    /// Where it would land: a column and an index among its other cards.
    pub target: Option<(String, usize)>,
}

pub(super) struct CardEditor {
    pub card: String,
    pub title: Entity<InputState>,
    pub description: Entity<InputState>,
    pub tags: Entity<InputState>,
    pub assignee: Entity<InputState>,
    language: gentle_core::i18n::Language,
}

const ADD_CARD_PLACEHOLDER: &str = "Card title, Enter to add";
const CARD_TAGS_PLACEHOLDER: &str = "Tags, comma-separated";

pub(super) struct AddCard {
    language: gentle_core::i18n::Language,
    pub column: String,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

pub(super) struct KanbanState {
    pub edits: Edits<Board>,
    pub selected: Option<String>,
    pub drag: Option<CardDrag>,
    pub editor: Option<CardEditor>,
    pub adding: Option<AddCard>,
    /// The board's bounds last frame (they move with its scroll).
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl KanbanState {
    pub fn new(board: Board) -> Self {
        Self {
            edits: Edits::new(board),
            selected: None,
            drag: None,
            editor: None,
            adding: None,
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }

    pub fn rebase(&mut self, board: Board) {
        self.edits.rebase(board);
        let working = self.edits.working();
        if self
            .selected
            .as_ref()
            .is_some_and(|id| working.card(id).is_none())
        {
            self.selected = None;
            self.editor = None;
        }
        if self
            .adding
            .as_ref()
            .is_some_and(|a| working.column(&a.column).is_none())
        {
            self.adding = None;
        }
    }

    pub fn height(&self) -> f32 {
        layout(self.edits.working(), None)
            .height
            .clamp(200., MAX_HEIGHT - CHROME)
    }

    /// The board drawn: the working one, or the drop preview while a card
    /// is dragged.
    fn shown(&self) -> Board {
        let working = self.edits.working();
        match &self.drag {
            Some(CardDrag {
                card,
                moved: true,
                target: Some((column, index)),
                ..
            }) => {
                let mut preview = working.clone();
                let order = working.dropped(card, column, *index);
                let _ = preview.apply(&Change::Order {
                    column: column.clone(),
                    cards: order,
                });
                preview
            }
            _ => working.clone(),
        }
    }
}

fn state(this: &mut PanelView) -> Option<&mut KanbanState> {
    match &mut this.body {
        Some(Body::Kanban(s)) => Some(s),
        _ => None,
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

impl PanelView {
    fn kanban(&self) -> Option<&KanbanState> {
        match &self.body {
            Some(Body::Kanban(s)) => Some(s),
            _ => None,
        }
    }

    /// Apply `change`; cards that end up elsewhere glide there.
    fn kanban_edit(&mut self, change: Change, cx: &mut Context<Self>) -> bool {
        let before = self
            .kanban()
            .map(|s| places(&layout(s.edits.working(), None)));
        let applied = self.edit_with(cx, |body| match body {
            Body::Kanban(s) => Some(s.edits.edit(&change)),
            _ => None,
        });
        let after = self
            .kanban()
            .map(|s| places(&layout(s.edits.working(), None)));
        if applied && let (Some(before), Some(after)) = (before, after) {
            self.glides.shift_all(&before, &after);
        }
        applied
    }

    fn select_card(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if let Some(s) = state(self) {
            if s.selected != id {
                s.editor = None;
            }
            s.selected = id;
        }
        cx.notify();
    }

    fn remove_card(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.kanban_edit(
            Change::RemoveCard {
                card: id.to_string(),
            },
            cx,
        ) {
            self.select_card(None, cx);
        }
    }

    /// Alt-←/→: to the end of the column beside; Alt-↑/↓: one place.
    fn nudge_card(&mut self, id: &str, columns: isize, places_by: isize, cx: &mut Context<Self>) {
        let Some(s) = self.kanban() else { return };
        let board = s.edits.working();
        let Some(card) = board.card(id) else { return };
        let Some(ci) = board.columns.iter().position(|c| c.id == card.column) else {
            return;
        };
        let Some(target) = ci
            .checked_add_signed(columns)
            .and_then(|i| board.columns.get(i))
        else {
            return;
        };
        let others = board
            .ids(&target.id)
            .into_iter()
            .filter(|c| c != id)
            .count();
        let index = if columns != 0 {
            others
        } else {
            let at = board
                .ids(&card.column)
                .iter()
                .position(|c| c == id)
                .unwrap_or(0);
            match at.checked_add_signed(places_by) {
                Some(i) if i <= others => i,
                _ => return,
            }
        };
        let order = board.dropped(id, &target.id, index);
        let column = target.id.clone();
        self.kanban_edit(
            Change::Order {
                column,
                cards: order,
            },
            cx,
        );
        self.dispatch_started(cx);
    }

    /// Cards just moved into a `starts` column go to the agent at once
    /// (with the other pending edits): it dispatches a subagent for each.
    fn dispatch_started(&mut self, cx: &mut Context<Self>) {
        let started = self
            .kanban()
            .is_some_and(|s| !Board::dispatches(s.edits.base(), s.edits.working()).is_empty());
        if started {
            self.send(cx);
        }
    }

    pub(super) fn start_add_card(
        &mut self,
        column: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(column) = column.or_else(|| {
            self.kanban()
                .and_then(|s| s.edits.working().columns.first())
                .map(|c| c.id.clone())
        }) else {
            return;
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::shared::i18n::t(ADD_CARD_PLACEHOLDER))
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.finish_add_card(true, window, cx),
                InputEvent::Blur => this.finish_add_card(false, window, cx),
                _ => {}
            },
        );
        input.update(cx, |state, cx| state.focus(window, cx));
        if let Some(s) = state(self) {
            s.adding = Some(AddCard {
                language: crate::shared::i18n::language(),
                column,
                input,
                _subscription: subscription,
            });
        }
        cx.notify();
    }

    /// Enter adds the card and keeps the field open for the next one.
    fn finish_add_card(&mut self, keep: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.kanban() else { return };
        let Some(adding) = &s.adding else { return };
        let title = adding.input.read(cx).value().trim().to_string();
        let (column, input) = (adding.column.clone(), adding.input.clone());
        if !keep || title.is_empty() {
            if let Some(s) = state(self) {
                s.adding = None;
            }
            cx.notify();
            return;
        }
        let id = s.edits.working().fresh_id(&title);
        let card = Card {
            id: id.clone(),
            title,
            column,
            description: None,
            tags: Vec::new(),
            assignee: None,
            code_ref: None,
            agent: None,
            agent_id: None,
        };
        if self.kanban_edit(Change::AddCard { card }, cx) {
            self.glides.appear(&id);
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
    }

    pub(super) fn start_edit_card(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self
            .kanban()
            .and_then(|s| s.edits.working().card(id))
            .cloned()
        else {
            return;
        };
        let field = |text: String,
                     placeholder: &'static str,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(crate::shared::i18n::t(placeholder))
                    .default_value(text)
            })
        };
        let title = field(card.title.clone(), "Title", window, cx);
        let description = field(
            card.description.clone().unwrap_or_default(),
            "Description",
            window,
            cx,
        );
        let tags = field(card.tags.join(", "), CARD_TAGS_PLACEHOLDER, window, cx);
        let assignee = field(
            card.assignee.clone().unwrap_or_default(),
            "Assignee",
            window,
            cx,
        );
        title.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        if let Some(s) = state(self) {
            s.selected = Some(id.to_string());
            s.editor = Some(CardEditor {
                language: crate::shared::i18n::language(),
                card: id.to_string(),
                title,
                description,
                tags,
                assignee,
            });
        }
        cx.notify();
    }

    fn save_card(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = state(self).and_then(|s| s.editor.take()) else {
            return;
        };
        cx.notify();
        let Some(card) = self
            .kanban()
            .and_then(|s| s.edits.working().card(&editor.card))
            .cloned()
        else {
            return;
        };
        let read = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let optional = |text: String| (!text.is_empty()).then_some(text);
        let title = read(&editor.title);
        let description = optional(read(&editor.description));
        let tags: Vec<String> = read(&editor.tags)
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let assignee = optional(read(&editor.assignee));
        let patch = CardPatch {
            title: (!title.is_empty() && title != card.title).then_some(title),
            description: (description != card.description).then_some(description),
            tags: (tags != card.tags).then_some(tags),
            assignee: (assignee != card.assignee).then_some(assignee),
        };
        if patch != CardPatch::default() {
            self.kanban_edit(
                Change::UpdateCard {
                    card: card.id,
                    patch,
                },
                cx,
            );
        }
    }

    pub(super) fn kanban_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(s) = self.kanban() else {
            return false;
        };
        if keystroke.key == "escape" && (s.editor.is_some() || s.adding.is_some()) {
            if let Some(s) = state(self) {
                s.editor = None;
                s.adding = None;
            }
            window.focus(&self.focus, cx);
            cx.notify();
            return true;
        }
        if keystroke.key == "enter" && s.editor.is_some() {
            self.save_card(cx);
            window.focus(&self.focus, cx);
            return true;
        }
        if self.typing(window, cx) {
            return false;
        }
        let Some(id) = s.selected.clone() else {
            return false;
        };
        let alt = keystroke.modifiers.alt;
        match keystroke.key.as_str() {
            "backspace" | "delete" => self.remove_card(&id, cx),
            "enter" => self.start_edit_card(&id, window, cx),
            "left" if alt => self.nudge_card(&id, -1, 0, cx),
            "right" if alt => self.nudge_card(&id, 1, 0, cx),
            "up" if alt => self.nudge_card(&id, 0, -1, cx),
            "down" if alt => self.nudge_card(&id, 0, 1, cx),
            "escape" => self.select_card(None, cx),
            _ => return false,
        }
        true
    }

    // ── Dragging ───────────────────────────────────────────────────────────

    /// The pointer on the board, px at 100 %.
    fn board_point(&self, position: Point<Pixels>, window: &Window) -> (f32, f32) {
        let rem = f32::from(window.rem_size()) / 16.;
        let origin = self
            .kanban()
            .map_or(Point::default(), |s| s.bounds.get().origin);
        (
            f32::from(position.x - origin.x) / rem,
            f32::from(position.y - origin.y) / rem,
        )
    }

    fn press_card(
        &mut self,
        id: &str,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        window.focus(&self.focus, cx);
        let (px_, py) = self.board_point(event.position, window);
        let Some(s) = state(self) else { return };
        let Some(&(x, y, _)) = layout(s.edits.working(), None).cards.get(id) else {
            return;
        };
        s.drag = Some(CardDrag {
            card: id.to_string(),
            from: event.position,
            now: event.position,
            moved: false,
            grab: (px_ - x, py - y),
            target: None,
        });
        cx.notify();
    }

    fn drag_card(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (bx, by) = self.board_point(event.position, window);
        let Some(s) = self.kanban() else { return };
        let Some(drag) = &s.drag else { return };
        let moved = drag.moved || {
            let d = event.position - drag.from;
            f32::from(d.x).hypot(f32::from(d.y)) >= DRAG_SLOP
        };
        let card = drag.card.clone();
        let board = s.edits.working();
        let before = places(&layout(&s.shown(), Some(&card)));
        // The column under the pointer (the nearest one in a gap), and the
        // index among its other cards whose middle is below the pointer.
        let base = layout(board, Some(&card));
        let target = board
            .columns
            .iter()
            .zip(&base.columns)
            .min_by(|(_, (a, _)), (_, (b, _))| {
                let d = |x: f32| (bx - (x + COL_W / 2.)).abs();
                d(*a).total_cmp(&d(*b))
            })
            .map(|(column, _)| {
                let slots: Vec<(f32, f32)> = board
                    .ids(&column.id)
                    .iter()
                    .filter(|c| **c != card)
                    .filter_map(|c| base.cards.get(c).map(|&(_, y, h)| (y, h)))
                    .collect();
                (column.id.clone(), drop_index(by, &slots))
            });
        if let Some(drag) = state(self).and_then(|s| s.drag.as_mut()) {
            drag.now = event.position;
            drag.moved = moved;
            drag.target = target;
        }
        if moved
            && let Some(after) = self
                .kanban()
                .map(|s| places(&layout(&s.shown(), Some(&card))))
        {
            self.glides.shift_all(&before, &after);
        }
        cx.notify();
    }

    fn drop_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = state(self).and_then(|s| s.drag.take()) else {
            return;
        };
        cx.notify();
        if !drag.moved {
            self.select_card(Some(drag.card), cx);
            return;
        }
        let (bx, by) = self.board_point(drag.now, window);
        let held = (bx - drag.grab.0, by - drag.grab.1);
        let Some((column, index)) = drag.target else {
            return;
        };
        let Some(order) = self
            .kanban()
            .map(|s| s.edits.working().dropped(&drag.card, &column, index))
        else {
            return;
        };
        let unchanged = self
            .kanban()
            .is_some_and(|s| s.edits.working().ids(&column) == order);
        if !unchanged {
            self.kanban_edit(
                Change::Order {
                    column,
                    cards: order,
                },
                cx,
            );
        }
        // It lands from where it was let go.
        if let Some(to) = self.kanban().and_then(|s| {
            places(&layout(s.edits.working(), None))
                .get(&drag.card)
                .copied()
        }) {
            self.glides.forget(&drag.card);
            self.glides.shift(&drag.card, held, to);
        }
        if let Some(s) = state(self) {
            s.selected = Some(drag.card);
        }
        self.dispatch_started(cx);
    }

    // ── Drawing ────────────────────────────────────────────────────────────

    pub(super) fn render_kanban(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(s) = state(self) {
            if let Some(add) = &mut s.adding {
                crate::shared::i18n::sync_placeholder(
                    &add.input,
                    ADD_CARD_PLACEHOLDER,
                    &mut add.language,
                    window,
                    cx,
                );
            }
            if let Some(edit) = &mut s.editor {
                super::sync_placeholders(
                    [
                        (&edit.title, "Title"),
                        (&edit.description, "Description"),
                        (&edit.tags, CARD_TAGS_PLACEHOLDER),
                        (&edit.assignee, "Assignee"),
                    ],
                    &mut edit.language,
                    window,
                    cx,
                );
            }
        }
        let Some(s) = self.kanban() else {
            return div().into_any_element();
        };
        let shown = s.shown();
        let dragged = s.drag.as_ref().filter(|d| d.moved).map(|d| d.card.clone());
        let geometry = layout(&shown, dragged.as_deref());
        let r = |v: f32| rems(v / 16.);
        let mut board = div().relative().w(r(geometry.width)).h(r(geometry.height));
        let cell = s.bounds.clone();
        board = board.child(
            canvas(move |bounds, _, _| cell.set(bounds), |_, _, _, _| {})
                .absolute()
                .size_full(),
        );
        let column_h = geometry.height - PAD * 2.;
        for (column, &(x, _)) in shown.columns.iter().zip(&geometry.columns) {
            let count = shown.ids(&column.id).len();
            let over = column.limit.is_some_and(|l| count > l as usize);
            let count_text = match column.limit {
                Some(limit) => format!("{count}/{limit}"),
                None => count.to_string(),
            };
            let column_id = column.id.clone();
            let selector = format!("genui-column-{}", column.id);
            board = board.child(
                v_flex()
                    .debug_selector(move || selector)
                    .absolute()
                    .left(r(x))
                    .top(r(PAD))
                    .w(r(COL_W))
                    .h(r(column_h))
                    .rounded_xl()
                    .bg(ui::well())
                    .border_1()
                    .border_color(if over {
                        ui::warning().opacity(0.5)
                    } else {
                        ui::border()
                    })
                    .child(
                        h_flex()
                            .h(r(HEAD_H))
                            .px_3()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(rems(12.5 / 16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ui::fg())
                                    .child(column.label.clone()),
                            )
                            .when(column.starts, |d| {
                                d.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "genui-column-starts-{}",
                                            column.id
                                        )))
                                        .text_color(ui::accent())
                                        .child(Icon::new(IconName::Bot).xsmall())
                                        .tooltip(|window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(
                                                crate::shared::i18n::t("A card dropped here: the agent starts a subagent on it"),
                                            )
                                            .build(window, cx)
                                        }),
                                )
                            })
                            .child(ui::chip(
                                count_text,
                                if over { ui::warning() } else { ui::muted() },
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("genui-add-card-{}", column.id)))
                            .h(r(ADD_H))
                            .px_3()
                            .gap_1p5()
                            .cursor_pointer()
                            .text_size(rems(12. / 16.))
                            .text_color(ui::faint())
                            .hover(|d| d.text_color(ui::muted()))
                            .child(Icon::new(IconName::Plus).xsmall())
                            .child(crate::shared::i18n::t("Add card"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.start_add_card(Some(column_id.clone()), window, cx)
                            })),
                    ),
            );
        }
        if let Some(adding) = &s.adding
            && let Some(ci) = shown.columns.iter().position(|c| c.id == adding.column)
        {
            let (x, bottom) = geometry.columns[ci];
            board = board.child(
                div()
                    .absolute()
                    .left(r(x + 8.))
                    .top(r(bottom))
                    .w(r(COL_W - 16.))
                    .child(
                        ui::field()
                            .h(r(28.))
                            .bg(ui::surface())
                            .child(Input::new(&adding.input).appearance(false).xsmall()),
                    ),
            );
        }
        for card in &shown.cards {
            if Some(&card.id) == dragged.as_ref() {
                continue;
            }
            let Some(&(x, y, h)) = geometry.cards.get(&card.id) else {
                continue;
            };
            let (dx, dy, opacity) = self.glides.at(&card.id);
            let element = self
                .render_card(card, h, false, cx)
                .absolute()
                .left(r(x + dx))
                .top(r(y + dy))
                .opacity(opacity);
            board = board.child(element);
        }
        // The dragged card follows the pointer, lifted above the others.
        if let Some(drag) = s.drag.as_ref().filter(|d| d.moved)
            && let Some(card) = shown.card(&drag.card)
        {
            let (bx, by) = self.board_point(drag.now, window);
            board = board.child(
                self.render_card(card, card_height(card), true, cx)
                    .absolute()
                    .left(r(bx - drag.grab.0))
                    .top(r(by - drag.grab.1)),
            );
        }
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("genui-board")
                    .size_full()
                    .overflow_scroll()
                    .on_mouse_move(cx.listener(Self::drag_card))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.drop_card(window, cx)),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.drop_card(window, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.select_card(None, cx)),
                    )
                    .child(board),
            )
            .children(self.render_card_details(cx))
            .into_any_element()
    }

    fn render_card(
        &self,
        card: &Card,
        height: f32,
        lifted: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self
            .kanban()
            .is_some_and(|s| s.selected.as_deref() == Some(card.id.as_str()));
        let (press_id, edit_id) = (card.id.clone(), card.id.clone());
        let selector = format!("genui-card-{}", card.id);
        v_flex()
            .id(SharedString::from(format!("genui-card-{}", card.id)))
            .debug_selector(move || selector)
            .w(rems((COL_W - 16.) / 16.))
            .h(rems(height / 16.))
            .p(rems(CARD_PAD / 16.))
            .gap(rems(4. / 16.))
            .rounded_lg()
            .bg(ui::surface())
            .border_1()
            .border_color(if selected || lifted {
                ui::accent().opacity(0.7)
            } else {
                ui::border()
            })
            .cursor(if lifted {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::PointingHand
            })
            .when(lifted, |d| d.shadow_lg().bg(ui::raised()))
            .when(!lifted, |d| {
                d.hover(|d| d.border_color(ui::border_strong()))
            })
            .child(
                div()
                    .truncate()
                    .text_size(rems(12.5 / 16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ui::fg())
                    .child(card.title.clone()),
            )
            .children(card.description.clone().map(|d| {
                div()
                    .truncate()
                    .text_size(rems(11.5 / 16.))
                    .text_color(ui::muted())
                    .child(d)
            }))
            .when(
                !card.tags.is_empty() || card.assignee.is_some() || card.code_ref.is_some(),
                |d| {
                    d.child(
                        h_flex()
                            .mt(rems(2. / 16.))
                            .gap_1()
                            .min_w_0()
                            .children(card.tags.iter().take(3).map(|t| {
                                ui::chip(t.clone(), ui::provider_color(t)).flex_shrink_0()
                            }))
                            .when(card.tags.len() > 3, |d| {
                                d.child(
                                    div()
                                        .text_size(rems(10.5 / 16.))
                                        .text_color(ui::faint())
                                        .child(format!("+{}", card.tags.len() - 3)),
                                )
                            })
                            .child(div().flex_1())
                            .children(card.code_ref.as_ref().map(|code_ref| {
                                super::refs::chip(
                                    format!("genui-card-ref-{}", card.id),
                                    code_ref,
                                    &self.env,
                                    cx,
                                )
                            }))
                            .children(card.assignee.clone().map(|who| {
                                div()
                                    .flex_shrink_0()
                                    .size(rems(20. / 16.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(ui::champagne().opacity(0.18))
                                    .text_size(rems(9.5 / 16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(ui::champagne())
                                    .child(initials(&who))
                            })),
                    )
                },
            )
            .when(card.agent.is_some() || card.agent_id.is_some(), |d| {
                d.child(self.agent_row(card))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if event.click_count >= 2 {
                        cx.stop_propagation();
                        this.start_edit_card(&edit_id, window, cx);
                    } else {
                        this.press_card(&press_id, event, window, cx);
                    }
                }),
            )
    }

    /// "🤖 worker · running · writing · $0.12": the card's subagent, live
    /// once the agent reported its id.
    fn agent_row(&self, card: &Card) -> Stateful<Div> {
        let status = card
            .agent_id
            .as_ref()
            .and_then(|id| self.agents.iter().find(|a| &a.id == id));
        let name = status
            .map(|s| s.agent.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| card.agent.as_ref()?.name.clone())
            .unwrap_or_else(|| "subagent".into());
        let (text, color) = match (status, &card.agent_id) {
            (Some(s), _) => {
                let mut parts = vec![s.status.clone()];
                parts.extend(s.step.clone().filter(|step| *step != s.status));
                parts.extend(s.cost.filter(|c| *c > 0.).map(|c| format!("${c:.2}")));
                (parts.join(" · "), agent_color(s))
            }
            (None, Some(_)) => ("dispatched".to_string(), ui::info()),
            (None, None) => (
                crate::shared::i18n::t("not started").to_string(),
                ui::faint(),
            ),
        };
        let selector = format!("genui-card-agent-{}", card.id);
        h_flex()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector)
            .gap_1()
            .min_w_0()
            .text_size(rems(11. / 16.))
            .text_color(color)
            .child(Icon::new(IconName::Bot).xsmall())
            .child(div().flex_shrink_0().text_color(ui::muted()).child(name))
            .child(div().min_w_0().truncate().child(text))
    }

    /// The selected card's details (or its editor), over the board's
    /// top-right corner.
    fn render_card_details(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.kanban()?;
        if s.drag.as_ref().is_some_and(|d| d.moved) {
            return None;
        }
        let board = s.edits.working();
        let card = board.card(s.selected.as_ref()?)?;
        let column = board
            .column(&card.column)
            .map_or(card.column.clone(), |c| c.label.clone());
        let panel = ui::island(cx)
            .absolute()
            .top_2()
            .right_2()
            .w(rems(264. / 16.))
            .p_3()
            .gap_2()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let button = |id: &'static str, icon: IconName, label: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .text_color(ui::muted())
                .icon(Icon::new(icon))
                .label(crate::shared::i18n::t(label))
        };
        if let Some(editor) = s.editor.as_ref().filter(|e| e.card == card.id) {
            let field = |input: &Entity<InputState>| {
                ui::field()
                    .h(rems(28. / 16.))
                    .child(Input::new(input).appearance(false).xsmall())
            };
            return Some(
                panel
                    .child(ui::section_label(crate::shared::i18n::t("Edit card")))
                    .child(field(&editor.title))
                    .child(field(&editor.description))
                    .child(field(&editor.tags))
                    .child(field(&editor.assignee))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().flex_1())
                            .child(
                                button("genui-card-cancel", IconName::Close, "Cancel").on_click(
                                    cx.listener(|this, _, _, cx| {
                                        if let Some(s) = state(this) {
                                            s.editor = None;
                                        }
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                div().debug_selector(|| "genui-card-save".into()).child(
                                    Button::new("genui-card-save")
                                        .primary()
                                        .xsmall()
                                        .label(crate::shared::i18n::t("Save"))
                                        .on_click(cx.listener(|this, _, _, cx| this.save_card(cx))),
                                ),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let (edit_id, ask_id, remove_id) = (card.id.clone(), card.id.clone(), card.id.clone());
        let title = card.title.clone();
        Some(
            panel
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(ui::fg())
                        .whitespace_normal()
                        .child(card.title.clone()),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .flex_wrap()
                        .child(ui::chip(column, ui::info()))
                        .children(
                            card.tags
                                .iter()
                                .map(|t| ui::chip(t.clone(), ui::provider_color(t))),
                        )
                        .children(
                            card.assignee
                                .clone()
                                .map(|who| ui::chip(format!("@{who}"), ui::champagne())),
                        ),
                )
                .children(card.description.clone().map(|d| {
                    div()
                        .text_size(rems(12. / 16.))
                        .text_color(ui::muted())
                        .whitespace_normal()
                        .child(d)
                }))
                .children(card.code_ref.as_ref().map(|code_ref| {
                    super::refs::chip(
                        format!("genui-card-details-ref-{}", card.id),
                        code_ref,
                        &self.env,
                        cx,
                    )
                }))
                .child(
                    div()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(10.5))
                        .text_color(ui::faint())
                        .child(card.id.clone()),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .flex_wrap()
                        .child(
                            button("genui-card-edit", IconName::Pencil, "Edit").on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.start_edit_card(&edit_id, window, cx)
                                }),
                            ),
                        )
                        .child(
                            button("genui-card-ask", IconName::MessageSquare, "Ask about this")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.ask(&format!("the card “{title}” ({ask_id})"), cx)
                                })),
                        )
                        .child(
                            button("genui-card-delete", IconName::Trash, "Delete").on_click(
                                cx.listener(move |this, _, _, cx| this.remove_card(&remove_id, cx)),
                            ),
                        )
                        .when_some(
                            card.agent_id
                                .as_ref()
                                .filter(|id| self.agents.iter().any(|a| &a.id == *id && a.active))
                                .cloned(),
                            |d, id| {
                                d.child(
                                    div().debug_selector(|| "genui-card-stop".into()).child(
                                        button(
                                            "genui-card-stop",
                                            IconName::Square,
                                            "Stop subagent",
                                        )
                                        .on_click(
                                            cx.listener(move |_, _, _, cx| {
                                                cx.emit(DiagramEvent::CancelSubagent(id.clone()))
                                            }),
                                        ),
                                    ),
                                )
                            },
                        ),
                )
                .into_any_element(),
        )
    }
}
