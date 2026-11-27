//! The review panel: a change set the agent proposes before writing it,
//! drawn with the app's diff viewer (`gpui_kit` `Diff`, the one the file
//! panes use, syntax highlighting included), a file list beside it, and
//! under each hunk its decision bar: Accept, Reject, Comment, or Applied
//! once the agent wrote it. Keys: j/k move between hunks, a/r accept or
//! reject (again: undecided), c comments, Shift-A/R decide the whole file.

use std::collections::HashMap;
use std::rc::Rc;

use gentle_core::genui::edits::Edits;
use gentle_core::genui::review::{Change, Decision, Review, Side, Status};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{
    Diff, DiffAnnotation, DiffDecoration, DiffFile, DiffHoverHighlight, DiffLinePosition,
    DiffLineRange, DiffSide, DiffState,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::panel::{Body, CHROME, MAX_HEIGHT, PanelView};
use crate::shared::ui;

/// The file list's width (px at 100 %), shown with two files or more.
const FILES_W: f32 = 196.;

const COMMENT_PLACEHOLDER: &str = "Comment for the agent, Enter to keep";

pub(super) struct CommentEdit {
    language: gentle_core::i18n::Language,
    pub hunk: String,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

pub(super) struct ReviewState {
    pub edits: Edits<Review>,
    /// The hunk the keys act on.
    pub current: Option<String>,
    pub commenting: Option<CommentEdit>,
    /// The diff viewer, built at the first render and given new files when
    /// the hunks change (decisions don't touch it).
    viewer: Option<Viewer>,
}

struct Viewer {
    state: Entity<DiffState>,
    /// What it shows.
    content: Review,
    _observe: Subscription,
}

impl ReviewState {
    pub fn new(review: Review) -> Self {
        Self {
            edits: Edits::new(review),
            current: None,
            commenting: None,
            viewer: None,
        }
    }

    pub fn rebase(&mut self, review: Review) {
        self.edits.rebase(review);
        let working = self.edits.working();
        if self
            .current
            .as_ref()
            .is_some_and(|id| working.hunk(id).is_none())
        {
            self.current = None;
        }
        if self
            .commenting
            .as_ref()
            .is_some_and(|c| working.hunk(&c.hunk).is_none_or(|h| h.applied))
        {
            self.commenting = None;
        }
    }

    pub fn height(&self) -> f32 {
        let review = self.edits.working();
        let lines: usize = review.hunks().map(|(_, h)| h.lines.len()).sum();
        let body = review.files.len() as f32 * 36.
            + review.hunk_count() as f32 * (26. + 40.)
            + lines as f32 * 20.
            + 8.;
        body.clamp(200., MAX_HEIGHT - CHROME)
    }
}

fn diff_side(side: Side) -> DiffSide {
    match side {
        Side::Old => DiffSide::Original,
        Side::New => DiffSide::Modified,
    }
}

/// The viewer's files: each file's hunks as a Git patch.
fn diff_files(review: &Review) -> Vec<DiffFile> {
    review
        .files
        .iter()
        .filter_map(|file| {
            let parsed = DiffFile::parse(&file.patch()).ok()?.into_iter().next()?;
            Some(match &file.language {
                Some(language) => parsed.with_language(language.clone()),
                None => parsed,
            })
        })
        .collect()
}

fn state(this: &mut PanelView) -> Option<&mut ReviewState> {
    match &mut this.body {
        Some(Body::Review(s)) => Some(s),
        _ => None,
    }
}

/// What one hunk's bar shows.
struct HunkBar {
    id: String,
    decision: Option<Decision>,
    comment: Option<String>,
    note: Option<String>,
    applied: bool,
    current: bool,
    commenting: Option<Entity<InputState>>,
}

impl PanelView {
    pub(super) fn review(&self) -> Option<&ReviewState> {
        match &self.body {
            Some(Body::Review(s)) => Some(s),
            _ => None,
        }
    }

    fn review_edit(&mut self, change: Change, cx: &mut Context<Self>) -> bool {
        self.edit_with(cx, |body| match body {
            Body::Review(s) => Some(s.edits.edit(&change)),
            _ => None,
        })
    }

    /// Accept or reject `id`; the same decision again leaves it undecided.
    pub(super) fn decide_hunk(&mut self, id: &str, decision: Decision, cx: &mut Context<Self>) {
        let Some(hunk) = self.review().and_then(|s| s.edits.working().hunk(id)) else {
            return;
        };
        let change = Change::Decide {
            hunk: id.to_string(),
            decision: (hunk.decision != Some(decision)).then_some(decision),
            comment: hunk.comment.clone(),
        };
        if let Some(s) = state(self) {
            s.current = Some(id.to_string());
        }
        self.review_edit(change, cx);
    }

    /// Accept all / Reject all, in one file (`Some(path)`) or everywhere.
    pub(super) fn decide_all(
        &mut self,
        path: Option<&str>,
        decision: Decision,
        cx: &mut Context<Self>,
    ) {
        let Some(s) = self.review() else { return };
        let hunks: Vec<String> = s
            .edits
            .working()
            .hunks()
            .filter(|(f, h)| path.is_none_or(|p| f.path == p) && !h.applied)
            .map(|(_, h)| h.id.clone())
            .collect();
        if hunks.is_empty() {
            return;
        }
        self.review_edit(
            Change::DecideAll {
                hunks,
                decision: Some(decision),
            },
            cx,
        );
    }

    pub(super) fn start_comment(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hunk) = self.review().and_then(|s| s.edits.working().hunk(id)) else {
            return;
        };
        if hunk.applied {
            return;
        }
        let text = hunk.comment.clone().unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::shared::i18n::t(COMMENT_PLACEHOLDER))
                .default_value(text)
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.finish_comment(true, cx);
                    window.focus(&this.focus, cx);
                }
                InputEvent::Blur => this.finish_comment(true, cx),
                _ => {}
            },
        );
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        if let Some(s) = state(self) {
            s.current = Some(id.to_string());
            s.commenting = Some(CommentEdit {
                language: crate::shared::i18n::language(),
                hunk: id.to_string(),
                input,
                _subscription: subscription,
            });
        }
        cx.notify();
    }

    fn finish_comment(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(edit) = state(self).and_then(|s| s.commenting.take()) else {
            return;
        };
        cx.notify();
        let text = edit.input.read(cx).value().trim().to_string();
        let Some(hunk) = self
            .review()
            .and_then(|s| s.edits.working().hunk(&edit.hunk))
        else {
            return;
        };
        let comment = (!text.is_empty()).then_some(text);
        if keep && hunk.comment != comment {
            let change = Change::Decide {
                hunk: edit.hunk.clone(),
                decision: hunk.decision,
                comment,
            };
            self.review_edit(change, cx);
        }
    }

    /// Make `id` the current hunk and scroll the viewer to it.
    fn focus_hunk(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(s) = state(self) else { return };
        let target = s.edits.working().hunks().find_map(|(f, h)| {
            (h.id == id).then(|| {
                let (side, line) = h.first_shown();
                DiffLinePosition::new(f.path.clone(), diff_side(side), line)
            })
        });
        s.current = Some(id);
        if let (Some(viewer), Some(position)) = (&s.viewer, target) {
            viewer
                .state
                .update(cx, |state, cx| state.scroll_to_line(position, cx));
        }
        cx.notify();
    }

    /// j/k: the next or previous hunk (the first one when none is current).
    fn step_hunk(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(s) = self.review() else { return };
        let ids = s.edits.working().hunk_ids();
        let next = match s
            .current
            .as_ref()
            .and_then(|c| ids.iter().position(|i| i == c))
        {
            Some(at) => at
                .checked_add_signed(by)
                .filter(|i| *i < ids.len())
                .unwrap_or(at),
            None => 0,
        };
        if let Some(id) = ids.get(next).cloned() {
            self.focus_hunk(id, cx);
        }
    }

    pub(super) fn select_file(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some(s) = state(self) else { return };
        let first = s
            .edits
            .working()
            .files
            .iter()
            .find(|f| f.path == path)
            .and_then(|f| f.hunks.first())
            .map(|h| h.id.clone());
        s.current = first;
        if let Some(viewer) = &s.viewer {
            viewer
                .state
                .update(cx, |state, cx| state.scroll_to_file(path, cx));
        }
        cx.notify();
    }

    /// Whether the keyboard is in the comment or the note field.
    fn review_typing(&self, window: &Window, cx: &App) -> bool {
        let comment = self
            .review()
            .and_then(|s| s.commenting.as_ref())
            .is_some_and(|c| c.input.focus_handle(cx).is_focused(window));
        let note = self
            .note
            .as_ref()
            .is_some_and(|n| n.focus_handle(cx).is_focused(window));
        comment || note
    }

    /// The review's keys. The viewer keeps the keyboard after a click in
    /// it, so this doesn't wait for the panel itself to have it.
    pub(super) fn review_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(s) = self.review() else {
            return false;
        };
        if keystroke.key == "escape" && s.commenting.is_some() {
            self.finish_comment(false, cx);
            window.focus(&self.focus, cx);
            return true;
        }
        if self.review_typing(window, cx) {
            return false;
        }
        let m = &keystroke.modifiers;
        if m.secondary(cx) && !m.shift && keystroke.key == "z" {
            self.undo(cx);
            return true;
        }
        if m.control || m.alt || m.platform || m.function {
            return false;
        }
        let current = s.current.clone();
        let file = current.as_ref().and_then(|id| {
            s.edits
                .working()
                .hunks()
                .find(|(_, h)| h.id == *id)
                .map(|(f, _)| f.path.clone())
        });
        match (keystroke.key.as_str(), m.shift) {
            ("j", false) => self.step_hunk(1, cx),
            ("k", false) => self.step_hunk(-1, cx),
            ("a" | "r", false) => {
                let decision = if keystroke.key == "a" {
                    Decision::Accepted
                } else {
                    Decision::Rejected
                };
                match current {
                    Some(id) => self.decide_hunk(&id, decision, cx),
                    None => self.step_hunk(1, cx),
                }
            }
            ("a" | "r", true) => {
                let decision = if keystroke.key == "a" {
                    Decision::Accepted
                } else {
                    Decision::Rejected
                };
                match file {
                    Some(path) => self.decide_all(Some(&path), decision, cx),
                    None => return false,
                }
            }
            ("c", false) => match current {
                Some(id) => self.start_comment(&id, window, cx),
                None => self.step_hunk(1, cx),
            },
            ("escape", _) if self.expanded => cx.emit(super::diagram::DiagramEvent::Collapse),
            _ => return false,
        }
        true
    }

    /// The viewer, (re)built when the hunks it shows changed.
    fn review_viewer(&mut self, cx: &mut Context<Self>) -> Option<Entity<DiffState>> {
        let s = self.review()?;
        let working = s.edits.working();
        if let Some(viewer) = &s.viewer
            && viewer.content.same_content(working)
        {
            return Some(viewer.state.clone());
        }
        let content = working.clone();
        let files = diff_files(&content);
        let existing = s.viewer.as_ref().map(|v| v.state.clone());
        let state_entity = match existing {
            Some(entity) => {
                entity.update(cx, |state, cx| state.set_files(files, cx));
                entity
            }
            None => cx.new(|cx| DiffState::new(files, cx)),
        };
        // Syntax highlighting arrives later: draw again then.
        let observe = cx.observe(&state_entity, |_, _, cx| cx.notify());
        if let Some(s) = state(self) {
            s.viewer = Some(Viewer {
                state: state_entity.clone(),
                content,
                _observe: observe,
            });
        }
        Some(state_entity)
    }

    pub(super) fn render_review(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(edit) = state(self).and_then(|s| s.commenting.as_mut()) {
            crate::shared::i18n::sync_placeholder(
                &edit.input,
                COMMENT_PLACEHOLDER,
                &mut edit.language,
                window,
                cx,
            );
        }
        let Some(viewer) = self.review_viewer(cx) else {
            return div().into_any_element();
        };
        let Some(s) = self.review() else {
            return div().into_any_element();
        };
        let review = s.edits.working();
        let weak = cx.entity().downgrade();
        let commenting = s.commenting.as_ref();

        let mut annotations = Vec::new();
        let mut bars: HashMap<DiffLinePosition, HunkBar> = HashMap::new();
        let mut decorations = Vec::new();
        for (file, hunk) in review.hunks() {
            let (side, line) = hunk.last_shown();
            let position = DiffLinePosition::new(file.path.clone(), diff_side(side), line);
            let current = s.current.as_deref() == Some(hunk.id.as_str());
            if current {
                let (first_side, first) = hunk.first_shown();
                decorations.push(
                    DiffDecoration::new(
                        DiffLineRange::new(file.path.clone(), diff_side(first_side), first, line)
                            .with_end_side(diff_side(side)),
                    )
                    .with_background(ui::accent().opacity(0.07)),
                );
            }
            annotations.push(DiffAnnotation::line(
                SharedString::from(format!("genui-hunk-{}", hunk.id)),
                position.clone(),
            ));
            bars.insert(
                position,
                HunkBar {
                    id: hunk.id.clone(),
                    decision: hunk.decision,
                    comment: hunk.comment.clone(),
                    note: hunk.note.clone(),
                    applied: hunk.applied,
                    current,
                    commenting: commenting
                        .filter(|c| c.hunk == hunk.id)
                        .map(|c| c.input.clone()),
                },
            );
        }
        let bars = Rc::new(bars);
        let counts: Rc<HashMap<String, (usize, usize)>> = Rc::new(
            review
                .files
                .iter()
                .map(|f| (f.path.clone(), (f.decided(), f.hunks.len())))
                .collect(),
        );
        let bar_view = weak.clone();
        let file_view = weak.clone();
        let diff = Diff::new(&viewer)
            .hover_highlight(DiffHoverHighlight::Line)
            .annotations(annotations)
            .render_annotation(move |annotation, _, _| {
                match annotation.position().and_then(|p| bars.get(p)) {
                    Some(bar) => hunk_bar(bar, &bar_view),
                    None => div().into_any_element(),
                }
            })
            .decorations(decorations)
            .render_header_suffix(move |file, _, _| {
                let path = file.path().to_string();
                let (decided, total) = counts.get(&path).copied().unwrap_or_default();
                file_tools(&path, decided, total, &file_view)
            })
            .size_full();

        let list = (review.files.len() > 1).then(|| self.render_file_list(cx));
        h_flex()
            .size_full()
            .min_h_0()
            .children(list)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .children(review.summary.clone().map(|summary| {
                        div()
                            .px_3()
                            .py_1p5()
                            .text_size(rems(12. / 16.))
                            .text_color(ui::muted())
                            .whitespace_normal()
                            .child(summary)
                    }))
                    .child(div().flex_1().min_h_0().child(diff)),
            )
            .into_any_element()
    }

    fn render_file_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.review() else {
            return div().into_any_element();
        };
        let review = s.edits.working();
        let current_file = s.current.as_ref().and_then(|id| {
            review
                .hunks()
                .find(|(_, h)| h.id == *id)
                .map(|(f, _)| f.path.as_str())
        });
        let rows = review.files.iter().map(|file| {
            let (letter, tone) = match file.status {
                Status::Added => ("A", ui::success()),
                Status::Modified => ("M", ui::warning()),
                Status::Deleted => ("D", ui::danger()),
                Status::Renamed => ("R", ui::info()),
            };
            let (dir, name) = match file.path.rsplit_once('/') {
                Some((dir, name)) => (Some(format!("{dir}/")), name.to_string()),
                None => (None, file.path.clone()),
            };
            let done = !file.hunks.is_empty() && file.decided() == file.hunks.len();
            let path = file.path.clone();
            let selector = format!("genui-review-file-{}", file.path);
            h_flex()
                .id(SharedString::from(format!(
                    "genui-review-file-{}",
                    file.path
                )))
                .debug_selector(move || selector)
                .px_2()
                .py_1()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .when(current_file == Some(file.path.as_str()), |d| {
                    d.bg(ui::surface_hover())
                })
                .hover(|d| d.bg(ui::surface_hover()))
                .child(ui::chip(letter, tone).flex_shrink_0())
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_size(rems(12. / 16.))
                                .text_color(ui::fg())
                                .child(name),
                        )
                        .children(dir.map(|dir| {
                            div()
                                .truncate()
                                .text_size(rems(10.5 / 16.))
                                .text_color(ui::faint())
                                .child(dir)
                        })),
                )
                .child(
                    v_flex()
                        .flex_shrink_0()
                        .items_end()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(10.5))
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_color(ui::success())
                                        .child(format!("+{}", file.additions())),
                                )
                                .child(
                                    div()
                                        .text_color(ui::danger())
                                        .child(format!("−{}", file.deletions())),
                                ),
                        )
                        .child(if done {
                            Icon::new(IconName::CircleCheck)
                                .xsmall()
                                .text_color(ui::success())
                                .into_any_element()
                        } else {
                            div()
                                .text_color(ui::faint())
                                .child(format!("{}/{}", file.decided(), file.hunks.len()))
                                .into_any_element()
                        }),
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.select_file(&path, window, cx)),
                )
        });
        v_flex()
            .id("genui-review-files")
            .w(rems(FILES_W / 16.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .p_1p5()
            .gap_0p5()
            .bg(ui::well())
            .children(rows)
            .into_any_element()
    }
}

/// What a viewer slot's button does to the panel.
type PanelAction = Box<dyn Fn(&mut PanelView, &mut Window, &mut Context<PanelView>)>;

/// Run `f` on the panel from a viewer slot (which has no `Context`), with
/// the keyboard back on the panel for j/k/a/r/c.
fn on_panel(
    view: &WeakEntity<PanelView>,
    f: PanelAction,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let view = view.clone();
    move |_, window, cx| {
        let _ = view.update(cx, |this, cx| {
            window.focus(&this.focus, cx);
            f(this, window, cx);
        });
    }
}

/// A file header's tools: its progress, Accept all and Reject all.
fn file_tools(
    path: &str,
    decided: usize,
    total: usize,
    view: &WeakEntity<PanelView>,
) -> AnyElement {
    let tool = |name: &str, label: &'static str, decision: Decision| {
        let id = format!("genui-review-{name}-file-{path}");
        let selector = id.clone();
        let path = path.to_string();
        div().debug_selector(move || selector).child(
            Button::new(SharedString::from(id))
                .ghost()
                .xsmall()
                .label(crate::shared::i18n::t(label))
                .on_click(on_panel(
                    view,
                    Box::new(move |this, _, cx| this.decide_all(Some(&path), decision, cx)),
                )),
        )
    };
    h_flex()
        .gap_1()
        .child(
            div()
                .font_family(ui::mono())
                .text_size(ui::code_rems(11.))
                .text_color(ui::faint())
                .child(format!("{decided}/{total}")),
        )
        .child(tool("accept", "Accept all", Decision::Accepted))
        .child(tool("reject", "Reject all", Decision::Rejected))
        .into_any_element()
}

/// The bar under a hunk: its state, the agent's note and the user's
/// comment, and Accept / Reject / Comment.
fn hunk_bar(bar: &HunkBar, view: &WeakEntity<PanelView>) -> AnyElement {
    let id = bar.id.clone();
    let state = match (bar.applied, bar.decision) {
        (true, _) => ui::chip(crate::shared::i18n::t("Applied"), ui::info()).into_any_element(),
        (false, Some(Decision::Accepted)) => {
            ui::chip(crate::shared::i18n::t("Accepted"), ui::success()).into_any_element()
        }
        (false, Some(Decision::Rejected)) => {
            ui::chip(crate::shared::i18n::t("Rejected"), ui::danger()).into_any_element()
        }
        (false, None) => div()
            .text_color(ui::faint())
            .child(crate::shared::i18n::t("Undecided"))
            .into_any_element(),
    };
    let action = |name: &str,
                  icon: IconName,
                  label: &'static str,
                  on: bool,
                  tone: Hsla,
                  click: PanelAction| {
        let selector = format!("genui-hunk-{name}-{id}");
        div().debug_selector(move || selector).child(
            Button::new(SharedString::from(format!("genui-hunk-{name}-{id}")))
                .ghost()
                .xsmall()
                .selected(on)
                .when(on, |b| b.text_color(tone))
                .icon(Icon::new(icon))
                .label(crate::shared::i18n::t(label))
                .on_click(on_panel(view, click)),
        )
    };
    let (accept_id, reject_id, comment_id) = (id.clone(), id.clone(), id.clone());
    let actions = h_flex()
        .flex_shrink_0()
        .gap_0p5()
        .child(action(
            "accept",
            IconName::Check,
            "Accept",
            bar.decision == Some(Decision::Accepted),
            ui::success(),
            Box::new(move |this, _, cx| this.decide_hunk(&accept_id, Decision::Accepted, cx)),
        ))
        .child(action(
            "reject",
            IconName::X,
            "Reject",
            bar.decision == Some(Decision::Rejected),
            ui::danger(),
            Box::new(move |this, _, cx| this.decide_hunk(&reject_id, Decision::Rejected, cx)),
        ))
        .child(action(
            "comment",
            IconName::MessageSquare,
            "Comment",
            bar.comment.is_some(),
            ui::accent(),
            Box::new(move |this, window, cx| this.start_comment(&comment_id, window, cx)),
        ));
    let comment: Option<AnyElement> = match (&bar.commenting, &bar.comment) {
        (Some(input), _) => Some(
            ui::field()
                .h(rems(28. / 16.))
                .border_color(ui::accent().opacity(0.6))
                .child(
                    Icon::new(IconName::MessageSquare)
                        .xsmall()
                        .text_color(ui::faint()),
                )
                .child(Input::new(input).appearance(false).xsmall())
                .into_any_element(),
        ),
        (None, Some(comment)) => Some(
            h_flex()
                .gap_1p5()
                .items_start()
                .text_color(ui::fg())
                .child(
                    Icon::new(IconName::MessageSquareText)
                        .xsmall()
                        .text_color(ui::accent()),
                )
                .child(div().min_w_0().whitespace_normal().child(comment.clone()))
                .into_any_element(),
        ),
        (None, None) => None,
    };
    let selector = format!("genui-hunk-bar-{id}");
    v_flex()
        .debug_selector(move || selector)
        .w_full()
        .px_3()
        .py_1()
        .gap_1()
        .text_size(rems(12. / 16.))
        .bg(if bar.current {
            ui::accent().opacity(0.07)
        } else {
            ui::surface()
        })
        .border_t_1()
        .border_b_1()
        .border_color(ui::border())
        .child(
            h_flex()
                .gap_2()
                .min_w_0()
                .child(state)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(ui::muted())
                        .children(bar.note.clone()),
                )
                .when(!bar.applied, |d| d.child(actions)),
        )
        .children(comment)
        .into_any_element()
}
