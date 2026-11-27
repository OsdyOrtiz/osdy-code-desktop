//! Filas, mediciones y cachés de presentación de la lista virtualizada.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::*;

use super::rail::preview;
use super::{BubbleText, CardSpec, follows_step, line_count};
use crate::features::session::attachments;
use crate::shared::ui;
use gentle_core::scroll::{self, Estimate, RailMark};
use gentle_core::transcript::{Block, Dirty, Item};

/// Rows after the transcript items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tail {
    /// The prompt just sent, in the slot (`base`) Pi's echo of it takes.
    PendingBubble(usize),
    /// "Thinking…" until Pi picks the prompt up.
    Pending(usize),
    /// "Working…" between tool calls, at this item count.
    Between(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    /// The hero and the backend state (empty once both are gone).
    Header,
    Item(usize),
    Tail(Tail),
    /// The column's bottom padding.
    Footer,
}

/// The list's rows: the header, one per transcript item, the tail rows and
/// the footer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Shape {
    items: usize,
    tail: Vec<Tail>,
}

impl Shape {
    fn len(&self) -> usize {
        self.items + self.tail.len() + 2
    }

    fn row(&self, ix: usize) -> Option<Row> {
        let tail_start = 1 + self.items;
        match ix {
            0 => Some(Row::Header),
            ix if ix < tail_start => Some(Row::Item(ix - 1)),
            ix if ix < tail_start + self.tail.len() => Some(Row::Tail(self.tail[ix - tail_start])),
            ix if ix + 1 == self.len() => Some(Row::Footer),
            _ => None,
        }
    }
}

/// How the list's rows change from `old` to `new`, given the first changed
/// item: rows to re-measure in place, then rows to replace.
#[derive(Debug, PartialEq, Eq)]
struct ListUpdate {
    remeasure: Range<usize>,
    /// Old rows replaced, and how many new rows take their place.
    splice: Option<(Range<usize>, usize)>,
}

fn list_update(old: &Shape, new: &Shape, dirty_from: Option<usize>) -> ListUpdate {
    let (old_len, new_len) = (old.len(), new.len());
    let mut first = old_len.max(new_len);
    if let Some(from) = dirty_from {
        first = first.min(1 + from);
    }
    if old.items != new.items {
        first = first.min(1 + old.items.min(new.items));
    } else if let Some(i) =
        (0..old.tail.len().max(new.tail.len())).find(|&i| old.tail.get(i) != new.tail.get(i))
    {
        first = first.min(1 + old.items + i);
    }
    let common = old_len.min(new_len);
    ListUpdate {
        remeasure: first.min(common)..common,
        splice: (old_len != new_len).then(|| (common..old_len, new_len - common)),
    }
}

/// What the header row shows; a change re-measures it (and the first row,
/// whose top padding depends on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HeaderKey {
    pub(super) hero: bool,
    pub(super) backend: u8,
}

/// Display data derived once per change of an item, not per frame.
pub(super) enum Prepared {
    /// A tool or shell card's data, built when its row first shows.
    Card(OnceCell<Option<CardSpec>>),
    /// A custom message's line count.
    Lines(usize),
    /// A message you sent as its bubble shows it, cut when first shown.
    Bubble(OnceCell<BubbleText>),
    /// An answer's thinking blocks by block index, trimmed when first shown
    /// open (empty for other blocks).
    Thoughts(Vec<OnceCell<SharedString>>),
}

/// How long after it first shows a row still fades in.
const APPEAR_WINDOW: Duration = Duration::from_millis(300);

/// The conversation as a virtualized list: only rows on screen render, and
/// rows are re-measured only from the first changed item on.
pub(in crate::features::session) struct TranscriptList {
    state: ListState,
    /// The rows `state` was last told about.
    shape: Shape,
    header: Option<HeaderKey>,
    /// By transcript index.
    prepared: HashMap<usize, Prepared>,
    /// When each animated row first rendered: it animates in only then, not
    /// each time it scrolls back into view.
    born: HashMap<(&'static str, usize), Instant>,
    /// Row heights seen on screen, by row. The list only knows the heights of
    /// rows it has laid out; the rail estimates the rest from these.
    heights: HashMap<usize, f32>,
    /// Each item's size from its content (`gentle_core::scroll::item_weight`),
    /// by transcript index: how the rail sizes rows it hasn't seen.
    weights: Vec<f32>,
    /// Items folded to their quiet one-line step row, by transcript index:
    /// they estimate as [`STEP_LINE_WEIGHT`] instead of their full weight.
    step_collapsed: HashSet<usize>,
    /// Items that render a step row right after another step row (see
    /// `follows_step`): their row drops the top padding.
    step_tight: HashSet<usize>,
    /// Items inside a run of finished steps
    /// (`gentle_core::transcript::steps::step_run`), by transcript index:
    /// how each renders and estimates.
    groups: HashMap<usize, GroupRow>,
    /// Every row's weight, in row order: the estimate's input.
    row_weights: Vec<f32>,
    /// Where the pane is along the whole conversation; its row tops are
    /// re-laid only while `stale` (weights, rows or heights changed).
    estimate: Estimate,
    stale: bool,
    /// Your messages, for the rail: transcript index and tooltip text.
    marks: Vec<(usize, SharedString)>,
    /// The marks laid out on a rail `rail_for` px tall (close ones merged);
    /// shared with the frame that paints them.
    rail: Rc<Vec<RailMark>>,
    rail_for: Option<f32>,
    /// The rem size the rows were measured at.
    rem_size: Option<Pixels>,
}

/// Header, tail and footer rows, in the same "lines" as item weights.
const HEADER_WEIGHT: f32 = 4.;
const TAIL_WEIGHT: f32 = 2.5;
const FOOTER_WEIGHT: f32 = 1.4;
/// A step folded to its quiet one-line row (`Item::Tool`, `Item::Custom`):
/// the line's padding and text. The full weight stays in `weights`, so
/// unfolding restores the estimate without re-reading the item.
const STEP_LINE_WEIGHT: f32 = 2.;
/// A row's top padding (24 px) in weight units: what a step row that
/// follows another step row drops (padding before and after a run of
/// steps only).
const ROW_PAD_WEIGHT: f32 = 24. / 17.;

/// How an item inside a run of finished steps renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroupRow {
    /// The run's first step, folded: its row is the run's summary line.
    Summary,
    /// The run's first step, unfolded: the summary line above its own row.
    Open,
    /// Any other item of a folded run: no row at all.
    Hidden,
}

/// An item's estimate weight: its content weight, folded to the step-line
/// weight when its quiet line shows, minus the top padding a step row
/// that follows another step row drops. Inside a run of finished steps
/// (`group`), a hidden item weighs nothing and the first step's row is,
/// or adds, the summary line.
fn item_row_weight(
    weight: Option<f32>,
    step_collapsed: bool,
    tight: bool,
    group: Option<GroupRow>,
) -> f32 {
    let own = || {
        if step_collapsed {
            STEP_LINE_WEIGHT
        } else {
            weight.unwrap_or(TAIL_WEIGHT)
        }
    };
    let base = match group {
        Some(GroupRow::Hidden) => return 0.,
        Some(GroupRow::Summary) => STEP_LINE_WEIGHT,
        Some(GroupRow::Open) => STEP_LINE_WEIGHT + own(),
        None => own(),
    };
    if tight { base - ROW_PAD_WEIGHT } else { base }
}

impl TranscriptList {
    pub(in crate::features::session) fn new() -> Self {
        let shape = Shape::default();
        // The list keeps the latest output in view on every layout, however
        // the rows grew since (streamed text, code highlighted later, a
        // row measured past its estimate); a scroll up stops it and coming
        // back to the bottom picks it up again.
        let state = ListState::new(shape.len(), ListAlignment::Top, px(600.));
        state.set_follow_mode(FollowMode::Tail);
        Self {
            state,
            shape,
            header: None,
            prepared: HashMap::new(),
            born: HashMap::new(),
            heights: HashMap::new(),
            weights: Vec::new(),
            step_collapsed: HashSet::new(),
            step_tight: HashSet::new(),
            groups: HashMap::new(),
            row_weights: Vec::new(),
            estimate: Estimate::default(),
            stale: true,
            marks: Vec::new(),
            rail: Rc::default(),
            rail_for: None,
            rem_size: None,
        }
    }

    /// The transcript was replaced: indices name other items now.
    pub(super) fn forget_items(&mut self) {
        self.prepared.clear();
        self.born.clear();
        self.heights.clear();
        self.weights.clear();
        self.step_collapsed.clear();
        self.step_tight.clear();
        self.groups.clear();
        self.marks.clear();
        self.stale = true;
    }

    /// Rows are measured at a rem size; a new interface size measures them
    /// again (the list itself only does that when its width changes).
    pub(super) fn follow_rem_size(&mut self, rem_size: Pixels) {
        if self
            .rem_size
            .replace(rem_size)
            .is_some_and(|was| was != rem_size)
        {
            self.heights.clear();
            self.stale = true;
            self.state.remeasure();
        }
    }

    /// Note the heights of the rows on screen. Only those: a row laid out in
    /// the overdraw below can still be growing (its Markdown parses in the
    /// background) and its size isn't updated until it scrolls in.
    pub(super) fn record_heights(&mut self) {
        let viewport = self.state.viewport_bounds();
        let mut row = self.state.logical_scroll_top().item_ix;
        while row < self.shape.len() {
            let Some(bounds) = self.state.bounds_for_item(row) else {
                break;
            };
            if bounds.top() >= viewport.bottom() {
                break;
            }
            let height = f32::from(bounds.size.height);
            if self.heights.insert(row, height) != Some(height) {
                self.stale = true;
            }
            row += 1;
        }
    }

    /// Row tops, total height and scroll offset: rows never laid out count
    /// by their content, scaled by what the laid-out rows measured. Also
    /// the pane's height. The tops are re-laid only when something they
    /// depend on changed; the offset is placed on every call.
    pub(super) fn estimate(&mut self) -> (&Estimate, f32) {
        if self.stale {
            self.stale = false;
            self.rail_for = None;
            self.row_weights.clear();
            self.row_weights.push(HEADER_WEIGHT);
            self.row_weights.extend((0..self.shape.items).map(|ix| {
                item_row_weight(
                    self.weights.get(ix).copied(),
                    self.step_collapsed.contains(&ix),
                    self.step_tight.contains(&ix),
                    self.groups.get(&ix).copied(),
                )
            }));
            self.row_weights
                .extend(self.shape.tail.iter().map(|_| TAIL_WEIGHT));
            self.row_weights.push(FOOTER_WEIGHT);
            self.estimate.measure(&self.row_weights, &self.heights);
        }
        let top = self.state.logical_scroll_top();
        let viewport = f32::from(self.state.viewport_bounds().size.height);
        self.estimate
            .scroll_to(top.item_ix, f32::from(top.offset_in_item), viewport);
        (&self.estimate, viewport)
    }

    /// Lay out the marks of your messages on a rail `rail` px tall, at the
    /// same scale as the thumb, unless they already are. Call after
    /// [`Self::estimate`].
    pub(super) fn lay_rail(&mut self, rail: f32) {
        if self.rail_for == Some(rail) {
            return;
        }
        self.rail_for = Some(rail);
        let Estimate { tops, content, .. } = &self.estimate;
        let scale = rail / content.max(1.);
        let ys = self
            .marks
            .iter()
            .map(|(ix, _)| (tops.get(ix + 1).copied().unwrap_or(*content) * scale).clamp(0., rail));
        scroll::rail_marks(ys, Rc::make_mut(&mut self.rail));
    }

    /// The last estimate ([`Self::estimate`]).
    pub(super) fn last_estimate(&self) -> &Estimate {
        &self.estimate
    }

    /// Your messages: transcript index and preview, in order.
    pub(super) fn marks(&self) -> &[(usize, SharedString)] {
        &self.marks
    }

    /// The rail's marks as last laid out ([`Self::lay_rail`]).
    pub(super) fn rail(&self) -> &Rc<Vec<RailMark>> {
        &self.rail
    }

    /// Derive display data for items `from..`.
    pub(super) fn prepare(&mut self, items: &[Item], from: usize) {
        self.stale = true;
        self.weights.resize(from.min(items.len()), TAIL_WEIGHT);
        // Items from `from` on are re-derived; the view re-states their fold.
        self.step_collapsed.retain(|&ix| ix < from);
        self.step_tight.retain(|&ix| ix < from);
        self.groups.retain(|&ix, _| ix < from);
        let kept = self.marks.partition_point(|(ix, _)| *ix < from);
        self.marks.truncate(kept);
        let changed = items.get(from..).unwrap_or_default();
        for (ix, item) in (from..).zip(changed) {
            self.weights.push(scroll::item_weight(item));
            if follows_step(items, ix) {
                self.step_tight.insert(ix);
            } else {
                self.step_tight.remove(&ix);
            }
            let prepared = match item {
                Item::Tool { .. } | Item::Shell { .. } => Some(Prepared::Card(OnceCell::new())),
                Item::Custom { text, .. } => Some(Prepared::Lines(line_count(text))),
                Item::CommandOutput { notices, .. } => Some(Prepared::Lines(
                    notices.iter().map(|(_, text)| line_count(text)).sum(),
                )),
                Item::User { text, .. } => {
                    // The rail and its tooltip show what was typed, never the
                    // `Attached files:` block: a prompt of files only reads
                    // as their names.
                    let marks = attachments::summary(text);
                    if !marks.trim().is_empty() {
                        self.marks.push((ix, preview(&marks)));
                    }
                    Some(Prepared::Bubble(OnceCell::new()))
                }
                Item::Assistant { blocks, .. }
                    if blocks.iter().any(|b| matches!(b, Block::Thinking(_))) =>
                {
                    Some(Prepared::Thoughts(
                        blocks.iter().map(|_| OnceCell::new()).collect(),
                    ))
                }
                _ => None,
            };
            match prepared {
                Some(prepared) => {
                    self.prepared.insert(ix, prepared);
                }
                None => {
                    self.prepared.remove(&ix);
                }
            }
        }
    }

    /// Tell the list which rows changed since the last sync.
    pub(super) fn sync(
        &mut self,
        items: usize,
        tail: Vec<Tail>,
        header: HeaderKey,
        dirty: Option<Dirty>,
    ) {
        let shape = Shape { items, tail };
        if dirty.is_some() || self.header != Some(header) || self.shape != shape {
            self.stale = true;
        }
        if dirty.is_some_and(|d| d.reset) {
            self.state.reset(shape.len());
            self.heights.clear();
        } else {
            if self.header.is_some_and(|old| old != header) {
                self.state.remeasure_items(0..self.shape.len().min(2));
                self.heights.retain(|row, _| *row >= 2);
            }
            let update = list_update(&self.shape, &shape, dirty.map(|d| d.from));
            if !update.remeasure.is_empty() {
                self.heights
                    .retain(|row, _| !update.remeasure.contains(row));
                self.state.remeasure_items(update.remeasure);
            }
            if let Some((range, count)) = update.splice {
                // Rows from the splice on now name other rows.
                self.heights.retain(|row, _| *row < range.start);
                self.state.splice(range, count);
            }
        }
        self.header = Some(header);
        self.shape = shape;
    }

    /// Drop localized card copy, but keep folds, the rail and the scroll anchor.
    pub(super) fn language_changed(&mut self, items: usize) {
        for prepared in self.prepared.values_mut() {
            if let Prepared::Card(card) = prepared {
                card.take();
            }
        }
        self.remeasure_items(0..items);
    }

    /// Items `items` changed height (a fold toggled, a run of steps grew).
    pub(super) fn remeasure_items(&mut self, items: Range<usize>) {
        let rows = (items.start + 1)..(items.end + 1).min(self.shape.len());
        if rows.is_empty() {
            return;
        }
        let before = self.heights.len();
        self.heights.retain(|row, _| !rows.contains(row));
        self.stale |= self.heights.len() != before;
        self.state.remeasure_items(rows);
    }

    #[cfg(test)]
    pub(in crate::features::session) fn weight(&self, ix: usize) -> Option<f32> {
        self.weights.get(ix).copied()
    }

    /// Item `ix` is a confirmed live skill invocation: weigh it collapsed.
    pub(super) fn confirm_skill(&mut self, ix: usize, item: &Item, command: &str) {
        if let Some(weight) = self.weights.get_mut(ix) {
            *weight = scroll::item_weight_confirmed(item, Some(command));
            self.stale = true;
        }
    }

    /// Whether item `ix` sits folded to its quiet one-line step row; a
    /// change re-lays the estimate (the row's weight moved).
    pub(super) fn set_step_collapsed(&mut self, ix: usize, collapsed: bool) {
        let changed = if collapsed {
            self.step_collapsed.insert(ix)
        } else {
            self.step_collapsed.remove(&ix)
        };
        self.stale |= changed;
    }

    /// How item `ix` renders inside a run of finished steps, if it is in
    /// one; a change re-lays the estimate. Whether it changed.
    pub(super) fn set_group(&mut self, ix: usize, group: Option<GroupRow>) -> bool {
        let was = match group {
            Some(group) => self.groups.insert(ix, group),
            None => self.groups.remove(&ix),
        };
        let changed = was != group;
        self.stale |= changed;
        changed
    }

    pub(super) fn group(&self, ix: usize) -> Option<GroupRow> {
        self.groups.get(&ix).copied()
    }

    pub(super) fn prepared(&self, ix: usize) -> Option<&Prepared> {
        self.prepared.get(&ix)
    }

    pub(super) fn row(&self, ix: usize) -> Option<Row> {
        self.shape.row(ix)
    }

    pub(super) fn state(&self) -> ListState {
        self.state.clone()
    }

    /// Scroll to the bottom at the next layout.
    pub(in crate::features::session) fn scroll_to_end(&self) {
        self.state.scroll_to_end();
    }

    /// Scrolled to (within a few lines of) the bottom: new output follows.
    pub(in crate::features::session) fn at_bottom(&self) -> bool {
        let offset = -self.state.scroll_px_offset_for_scrollbar().y;
        let max = self.state.max_offset_for_scrollbar().y;
        offset >= max - px(48.)
    }

    /// `element`, fading in if the row first showed just now.
    pub(super) fn appear(
        &mut self,
        id: (&'static str, usize),
        now: Instant,
        element: Div,
    ) -> AnyElement {
        let born = *self.born.entry(id).or_insert(now);
        if now.duration_since(born) < APPEAR_WINDOW {
            ui::appear(id, element).into_any_element()
        } else {
            element.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GroupRow, ListUpdate, ROW_PAD_WEIGHT, Row, STEP_LINE_WEIGHT, Shape, TAIL_WEIGHT, Tail,
        item_row_weight, list_update,
    };

    #[test]
    fn a_step_row_that_follows_another_drops_its_padding_from_the_estimate() {
        // The run's first row keeps its top padding.
        assert_eq!(
            item_row_weight(Some(13.), true, false, None),
            STEP_LINE_WEIGHT
        );
        assert_eq!(item_row_weight(Some(13.), false, false, None), 13.);
        // A step row that follows another step row weighs its content
        // without the top padding it drops.
        assert_eq!(
            item_row_weight(Some(13.), true, true, None),
            STEP_LINE_WEIGHT - ROW_PAD_WEIGHT
        );
        assert_eq!(
            item_row_weight(Some(13.), false, true, None),
            13. - ROW_PAD_WEIGHT
        );
        // Unmeasured rows estimate from the default tail weight.
        assert_eq!(
            item_row_weight(None, false, true, None),
            TAIL_WEIGHT - ROW_PAD_WEIGHT
        );
    }

    #[test]
    fn a_folded_run_of_steps_estimates_as_its_summary_line() {
        // The other items of a folded run take no room at all.
        assert_eq!(
            item_row_weight(Some(13.), true, true, Some(GroupRow::Hidden)),
            0.
        );
        // The first step's row is the summary line, whatever its own fold.
        assert_eq!(
            item_row_weight(Some(13.), false, false, Some(GroupRow::Summary)),
            STEP_LINE_WEIGHT
        );
        // Unfolded, the summary line sits above the step's own row.
        assert_eq!(
            item_row_weight(Some(13.), true, true, Some(GroupRow::Open)),
            2. * STEP_LINE_WEIGHT - ROW_PAD_WEIGHT
        );
    }

    #[test]
    fn rows_map_to_header_items_tail_and_footer() {
        let shape = Shape {
            items: 2,
            tail: vec![Tail::Between(2)],
        };
        assert_eq!(shape.len(), 5);
        assert_eq!(shape.row(0), Some(Row::Header));
        assert_eq!(shape.row(2), Some(Row::Item(1)));
        assert_eq!(shape.row(3), Some(Row::Tail(Tail::Between(2))));
        assert_eq!(shape.row(4), Some(Row::Footer));
        assert_eq!(shape.row(5), None);

        let empty = Shape::default();
        assert_eq!(empty.len(), 2);
        assert_eq!(empty.row(0), Some(Row::Header));
        assert_eq!(empty.row(1), Some(Row::Footer));
        assert_eq!(empty.row(2), None);
    }

    #[test]
    fn list_updates_start_at_the_first_changed_row() {
        let shape = |items, tail: &[Tail]| Shape {
            items,
            tail: tail.to_vec(),
        };
        // Streaming into the last of 3 items: only its row is re-measured.
        assert_eq!(
            list_update(&shape(3, &[]), &shape(3, &[]), Some(2)),
            ListUpdate {
                remeasure: 3..5,
                splice: None
            }
        );
        // Nothing changed: nothing to do.
        assert_eq!(
            list_update(&shape(3, &[]), &shape(3, &[]), None),
            ListUpdate {
                remeasure: 5..5,
                splice: None
            }
        );
        // An item appended: the old footer slot is re-measured, one row added.
        assert_eq!(
            list_update(&shape(3, &[]), &shape(4, &[]), Some(3)),
            ListUpdate {
                remeasure: 4..5,
                splice: Some((5..5, 1))
            }
        );
        // Removing items remeasures from the first removed item and truncates rows.
        assert_eq!(
            list_update(&shape(4, &[]), &shape(2, &[]), Some(2)),
            ListUpdate {
                remeasure: 3..4,
                splice: Some((4..6, 0))
            }
        );
        // The pending bubble becomes the real message; the indicator goes.
        let pending = [Tail::PendingBubble(1), Tail::Pending(1)];
        assert_eq!(
            list_update(&shape(1, &pending), &shape(2, &[]), Some(1)),
            ListUpdate {
                remeasure: 2..4,
                splice: Some((4..5, 0))
            }
        );
        // Only the tail changed.
        assert_eq!(
            list_update(&shape(2, &[]), &shape(2, &[Tail::Between(2)]), None),
            ListUpdate {
                remeasure: 3..4,
                splice: Some((4..4, 1))
            }
        );
        // A same-length tail replacement remeasures from that tail without splicing.
        assert_eq!(
            list_update(
                &shape(2, &[Tail::Pending(2)]),
                &shape(2, &[Tail::Between(2)]),
                None,
            ),
            ListUpdate {
                remeasure: 3..5,
                splice: None
            }
        );
    }
}
