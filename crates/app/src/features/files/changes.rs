//! The sidebar's Changes tab, after Tern: the working tree in three groups,
//! Staged first, then Unstaged (tracked files' edits) and Untracked, each
//! with its count and a checkbox per file to move it across; a list or
//! by-folder view, stash, and a commit box whose message Gentle Shell can
//! write from the staged diff.

use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _, DropdownButton};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    Disableable as _, Icon, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Text;
use super::status_color;
use crate::features::git::{self, ChangeGroup, EntryKind, StatusEntry, WorkTree};
use crate::shared::i18n::{language, sync_placeholder, t};
use crate::shared::{themes, ui};

const POLL: Duration = Duration::from_secs(3);
const ROW_HEIGHT: f32 = 26.;
/// The most diff handed to the model for a commit message.
const DIFF_LIMIT: usize = 24_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangesEvent {
    /// A change was clicked: show its diff (absolute path).
    OpenDiff(PathBuf),
    /// "Open file" from a change's menu (absolute path).
    OpenFile(PathBuf),
    /// [`ChangesView::change_count`] changed.
    Changed,
    /// "Show stashes" from the clean view: open the stashes in Branches.
    ShowStashes,
}

#[derive(Clone, PartialEq)]
struct Repo {
    /// Status paths are relative to this, not to the sidebar's root.
    toplevel: PathBuf,
    tree: WorkTree,
    /// Shortened path of the repository when the sidebar's root sits inside a larger one.
    repository: Option<String>,
    /// Stashes kept, counted only while the working tree is clean.
    stashes: usize,
}

impl Repo {
    fn read(root: &Path, toplevel: PathBuf) -> Option<Self> {
        let tree = git::work_tree(&toplevel)?;
        let repository = git::repository_path(root, &toplevel);
        let stashes = if tree.entries.is_empty() {
            git::stash_count(&toplevel)
        } else {
            0
        };
        Some(Repo {
            toplevel,
            tree,
            repository,
            stashes,
        })
    }
}

/// One row of the virtualized list; every kind is [`ROW_HEIGHT`] tall.
enum Line {
    Header {
        group: ChangeGroup,
        count: usize,
        folded: bool,
    },
    Dir {
        group: ChangeGroup,
        dir: String,
        count: usize,
        collapsed: bool,
    },
    Entry {
        group: ChangeGroup,
        /// Index into the work tree's entries.
        entry: usize,
        badge: char,
        /// Under a folder row in the by-folder view.
        nested: bool,
    },
}

pub struct ChangesView {
    root: PathBuf,
    /// `None` until the first read, `Some(None)` outside a git work tree.
    repo: Option<Option<Repo>>,
    /// Group by folder (the default) or the flat list; the choice is saved
    /// in the app's settings as `changesGroupByFolder`.
    tree_mode: bool,
    /// Groups folded to their header (Staged, Unstaged, Untracked).
    folded: HashSet<ChangeGroup>,
    /// Folders folded in the by-folder view, per group.
    collapsed: HashSet<(ChangeGroup, String)>,
    selected: Option<(ChangeGroup, String)>,
    /// The list's rows, rebuilt when the tree, mode or folded groups or folders change.
    lines: Rc<Vec<Line>>,
    scroll: UniformListScrollHandle,
    summary: Entity<InputState>,
    description: Entity<TextareaState>,
    stash_message: Entity<InputState>,
    form_language: gentle_core::i18n::Language,
    stash_language: gentle_core::i18n::Language,
    amend: bool,
    generating: bool,
    /// What a running commit/push is doing, for the status line.
    busy: Option<&'static str>,
    error: Option<Text>,
    visible: bool,
    /// [`Self::open_diff`] came before the first read: run it once it lands.
    open_pending: bool,
    /// The clean view offers "Show stashes" (the sidebar; the review has
    /// no Branches tab to show them in).
    pub(super) offers_stashes: bool,
    _poll: Task<()>,
    _commit: Task<()>,
    _generate: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ChangesEvent> for ChangesView {}

impl ChangesView {
    pub fn new(root: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let summary = cx.new(|cx| InputState::new(window, cx).placeholder(t("Summary")));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 8)
                .placeholder(t("Why this change…"))
        });
        let stash_message =
            cx.new(|cx| InputState::new(window, cx).placeholder(t("Message (optional)")));
        // The primary button reads "Generate" until a summary exists.
        let subscriptions = vec![cx.subscribe(&summary, |_, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                cx.notify();
            }
        })];
        let mut this = Self {
            root,
            repo: None,
            tree_mode: themes::store_path()
                .as_deref()
                .is_none_or(read_group_by_folder),
            folded: HashSet::new(),
            collapsed: HashSet::new(),
            selected: None,
            lines: Rc::new(Vec::new()),
            scroll: UniformListScrollHandle::new(),
            summary,
            description,
            stash_message,
            form_language: language(),
            stash_language: language(),
            amend: false,
            generating: false,
            busy: None,
            error: None,
            visible: true,
            open_pending: false,
            offers_stashes: false,
            _poll: Task::ready(()),
            _commit: Task::ready(()),
            _generate: Task::ready(()),
            _subscriptions: subscriptions,
        };
        this._poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                // Only while shown and the window is in front.
                let Ok(due) = this.update_in(cx, |this, window, _| {
                    (this.visible && window.is_window_active()).then(|| this.status_job())
                }) else {
                    break;
                };
                if let Some(job) = due {
                    let repo = cx.background_executor().spawn(job).await;
                    if this.update(cx, |this, cx| this.apply(repo, cx)).is_err() {
                        break;
                    }
                }
                cx.background_executor().timer(POLL).await;
                crate::shared::diagnostics::tick();
            }
        });
        this
    }

    /// Pause polling while hidden; showing the tab re-reads status.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        let shown = visible && !self.visible;
        self.visible = visible;
        if shown {
            self.refresh(cx);
        }
    }

    /// Files with any change, staged or not (one per path).
    pub fn change_count(&self) -> usize {
        self.tree().map_or(0, |tree| tree.entries.len())
    }

    fn tree(&self) -> Option<&WorkTree> {
        self.repo.as_ref()?.as_ref().map(|repo| &repo.tree)
    }

    fn repository_path(&self) -> Option<String> {
        self.repo.as_ref()?.as_ref()?.repository.clone()
    }

    fn toplevel(&self) -> Option<PathBuf> {
        self.repo
            .as_ref()?
            .as_ref()
            .map(|repo| repo.toplevel.clone())
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let job = self.status_job();
        cx.spawn(async move |this, cx| {
            let repo = cx.background_executor().spawn(job).await;
            let _ = this.update(cx, |this, cx| this.apply(repo, cx));
        })
        .detach();
    }

    fn status_job(&self) -> impl Future<Output = Option<Repo>> + Send + 'static + use<> {
        let root = self.root.clone();
        let known = self.toplevel();
        async move {
            let toplevel = known.or_else(|| git::toplevel(&root))?;
            Repo::read(&root, toplevel)
        }
    }

    fn apply(&mut self, repo: Option<Repo>, cx: &mut Context<Self>) {
        if self.repo.as_ref() == Some(&repo) {
            return;
        }
        let before = self.change_count();
        self.repo = Some(repo);
        self.relayout();
        if self.change_count() != before {
            cx.emit(ChangesEvent::Changed);
        }
        if std::mem::take(&mut self.open_pending) {
            self.open_diff(cx);
        }
        cx.notify();
    }

    /// Show a diff (the composer's review button): the selected file's, else
    /// the first listed. Before the first read, once it lands.
    pub fn open_diff(&mut self, cx: &mut Context<Self>) {
        if self.repo.is_none() {
            self.open_pending = true;
            return;
        }
        let Some(tree) = self.tree() else { return };
        // The selected file while it's still changed, else the first listed.
        let target = self
            .selected
            .clone()
            .filter(|(_, path)| tree.entries.iter().any(|e| e.path == *path))
            .or_else(|| {
                self.lines.iter().find_map(|line| match line {
                    Line::Entry { group, entry, .. } => {
                        Some((*group, tree.entries.get(*entry)?.path.clone()))
                    }
                    _ => None,
                })
            });
        if let Some((group, path)) = target {
            self.click_entry(&path, group, cx);
        }
    }

    /// Rebuild the flat row model: per group (Staged, Unstaged, Untracked) a
    /// header with its count, then, unless the group is folded, its entries
    /// (under folder rows of its own in the by-folder view).
    fn relayout(&mut self) {
        let mut lines = Vec::new();
        if let Some(tree) = self.tree() {
            for (group, entries) in git::change_groups(&tree.entries) {
                let folded = self.folded.contains(&group);
                lines.push(Line::Header {
                    group,
                    count: entries.len(),
                    folded,
                });
                if folded {
                    continue;
                }
                if !self.tree_mode {
                    lines.extend(entries.into_iter().map(|(entry, badge)| Line::Entry {
                        group,
                        entry,
                        badge,
                        nested: false,
                    }));
                    continue;
                }
                let mut folders: BTreeMap<&str, Vec<(usize, char)>> = BTreeMap::new();
                for (entry, badge) in entries {
                    let path = &tree.entries[entry].path;
                    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
                    folders.entry(dir).or_default().push((entry, badge));
                }
                for (dir, files) in folders {
                    let nested = !dir.is_empty();
                    if nested {
                        let collapsed = self.collapsed.contains(&(group, dir.to_string()));
                        lines.push(Line::Dir {
                            group,
                            dir: dir.to_string(),
                            count: files.len(),
                            collapsed,
                        });
                        if collapsed {
                            continue;
                        }
                    }
                    lines.extend(files.into_iter().map(|(entry, badge)| Line::Entry {
                        group,
                        entry,
                        badge,
                        nested,
                    }));
                }
            }
        }
        self.lines = Rc::new(lines);
    }

    /// Fold or unfold a group to its header; staging is left alone.
    fn toggle_fold(&mut self, group: ChangeGroup, cx: &mut Context<Self>) {
        if !self.folded.remove(&group) {
            self.folded.insert(group);
        }
        self.relayout();
        cx.notify();
    }

    /// Run a git operation in the background, then re-read status; a
    /// failure shows git's message under the commit box.
    fn git_op(
        &mut self,
        job: impl FnOnce(&Path) -> Result<(), Text> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        let root = self.root.clone();
        cx.spawn(async move |this, cx| {
            let (result, tree) = cx
                .background_executor()
                .spawn(async move {
                    let result = job(&toplevel);
                    let tree = Repo::read(&root, toplevel);
                    (result, tree)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.error = result.err();
                this.apply(tree, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle(&mut self, entry: &StatusEntry, group: ChangeGroup, cx: &mut Context<Self>) {
        let mut paths = vec![entry.path.clone()];
        // Unstaging a rename has to bring its source back to the index too.
        paths.extend(entry.orig.clone());
        match group {
            ChangeGroup::Unstaged | ChangeGroup::Untracked => {
                self.git_op(move |top| git::stage(top, &paths), cx)
            }
            ChangeGroup::Staged => self.git_op(move |top| git::unstage(top, &paths), cx),
        }
    }

    fn click_entry(&mut self, path: &str, group: ChangeGroup, cx: &mut Context<Self>) {
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        self.selected = Some((group, path.to_string()));
        cx.emit(ChangesEvent::OpenDiff(toplevel.join(path)));
        cx.notify();
    }

    fn confirm_discard(
        &mut self,
        entry: StatusEntry,
        group: ChangeGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = entry
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&entry.path)
            .to_string();
        let title = Text::from("Discard changes to “{name}”?").with("{name}", name);
        let description_key = match (group, entry.index) {
            (ChangeGroup::Untracked, _) => "The untracked file is deleted.",
            (ChangeGroup::Unstaged, _) => "Unstaged edits go back to the staged version.",
            (ChangeGroup::Staged, 'A' | 'C') => {
                "The new file is removed from the index and deleted."
            }
            (ChangeGroup::Staged, _) => {
                "The file goes back to the last commit, staged edits included."
            }
        };
        let description = Text::from("{description} This can't be undone.")
            .with_text("{description}", description_key.into());
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (this, entry) = (this.clone(), entry.clone());
            alert
                .title(title.render())
                .description(description.render())
                .ok_text(t("Discard"))
                .cancel_text(t("Cancel"))
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let entry = entry.clone();
                    let staged = group == ChangeGroup::Staged;
                    let _ = this.update(cx, |this, cx| {
                        this.git_op(move |top| git::discard(top, &entry, staged), cx)
                    });
                    true
                })
        });
    }

    fn open_stash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_message
            .update(cx, |state, cx| state.set_value("", window, cx));
        let input = self.stash_message.clone();
        let this = cx.entity().downgrade();
        let first = Cell::new(true);
        window.open_alert_dialog(cx, move |alert, window, cx| {
            if first.replace(false) {
                input.update(cx, |state, cx| state.focus(window, cx));
            }
            let (this, message) = (this.clone(), input.clone());
            alert
                .title(t("Stash all changes"))
                .description(
                    t("Staged, unstaged and untracked changes move to a new stash and the working tree is left clean."),
                )
                .child(div().pt_2().child(Input::new(&input).small()))
                .ok_text(t("Stash"))
                .cancel_text(t("Cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let message = message.read(cx).value().to_string();
                    let _ = this.update(cx, |this, cx| {
                        this.git_op(move |top| git::stash_push(top, &message), cx)
                    });
                    true
                })
        });
    }

    // ── Commit box ──────────────────────────────────────────────────────────

    fn summary_text(&self, cx: &App) -> String {
        self.summary.read(cx).value().trim().to_string()
    }

    /// Cmd/Ctrl+Enter and the main button: write a message first if there is none.
    fn primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.summary_text(cx).is_empty() && !self.amend {
            self.generate(window, cx);
        } else {
            self.commit(false, window, cx);
        }
    }

    fn commit(&mut self, push: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        let summary = self.summary_text(cx);
        if summary.is_empty() && !self.amend {
            self.error = Some("Write a summary, or let Gentle Shell write it".into());
            cx.notify();
            return;
        }
        let description = self.description.read(cx).value().to_string();
        let amend = self.amend;
        let root = self.root.clone();
        self.busy = Some(if push {
            "Committing and pushing…"
        } else {
            "Committing…"
        });
        self.error = None;
        cx.notify();
        self._commit = cx.spawn_in(window, async move |this, cx| {
            let (committed, result, tree) = cx
                .background_executor()
                .spawn(async move {
                    let mut committed = false;
                    let result =
                        git::commit(&toplevel, &summary, &description, amend).and_then(|()| {
                            committed = true;
                            if push {
                                git::push(&toplevel).map_err(|e| {
                                    Text::from("Committed, but the push failed: {error}")
                                        .with_text("{error}", e)
                                })
                            } else {
                                Ok(())
                            }
                        });
                    let tree = Repo::read(&root, toplevel);
                    (committed, result, tree)
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = None;
                this.error = result.err();
                if committed {
                    this.amend = false;
                    this.summary
                        .update(cx, |state, cx| state.set_value("", window, cx));
                    this.description
                        .update(cx, |state, cx| state.set_value("", window, cx));
                }
                this.apply(tree, cx);
                cx.notify();
            });
        });
    }

    fn generate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.generating {
            return;
        }
        let Some(toplevel) = self.toplevel() else {
            return;
        };
        self.generating = true;
        self.error = None;
        cx.notify();
        self._generate = cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { generate_message(&toplevel) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.generating = false;
                match result {
                    Ok((summary, description)) => {
                        this.summary
                            .update(cx, |state, cx| state.set_value(summary, window, cx));
                        this.description
                            .update(cx, |state, cx| state.set_value(description, window, cx));
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        });
    }

    /// Amending with empty fields starts from the last commit's message.
    fn toggle_amend(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.amend = !self.amend;
        cx.notify();
        let empty =
            self.summary_text(cx).is_empty() && self.description.read(cx).value().trim().is_empty();
        let Some(toplevel) = self.toplevel().filter(|_| self.amend && empty) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let message = cx
                .background_executor()
                .spawn(async move { git::last_commit_message(&toplevel) })
                .await;
            let Some((summary, description)) = message else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if this.amend && this.summary_text(cx).is_empty() {
                    this.summary
                        .update(cx, |state, cx| state.set_value(summary, window, cx));
                    this.description
                        .update(cx, |state, cx| state.set_value(description, window, cx));
                }
            });
        })
        .detach();
    }

    fn on_commit_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "enter" && event.keystroke.modifiers.secondary(cx) {
            self.primary(window, cx);
            cx.stop_propagation();
        }
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    fn render_header(&self, cx: &mut Context<Self>) -> Div {
        let has_changes = self.change_count() > 0;
        let mode = |id: &'static str, icon: IconName, tree: bool, tip: &'static str| {
            let active = self.tree_mode == tree;
            div()
                .id(id)
                .size(rems(24. / 16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .when(active, |d| d.bg(ui::raised()))
                .when(!active, |d| d.hover(|s| s.bg(ui::surface_hover())))
                .child(Icon::new(icon).xsmall().text_color(if active {
                    ui::fg()
                } else {
                    ui::muted()
                }))
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.tree_mode != tree {
                        this.tree_mode = tree;
                        if let Some(path) = themes::store_path()
                            && let Err(err) = write_group_by_folder(&path, tree)
                        {
                            eprintln!("gentle-shell-gpui: could not save the Changes view: {err}");
                        }
                        this.relayout();
                        cx.notify();
                    }
                }))
        };
        h_flex()
            .flex_none()
            .h(rems(36. / 16.))
            .pl_3()
            .pr_1p5()
            .gap_2()
            .child(Icon::new(IconName::Diff).small().text_color(ui::muted()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(rems(13.5 / 16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui::fg())
                    .child(t("Working tree")),
            )
            .child(
                h_flex()
                    .flex_none()
                    .p(px(2.))
                    .gap(px(2.))
                    .rounded_lg()
                    .bg(ui::well())
                    .child(mode("changes-mode-list", IconName::List, false, t("List")))
                    .child(mode(
                        "changes-mode-tree",
                        IconName::ListTree,
                        true,
                        t("Group by folder"),
                    )),
            )
            .child(
                Button::new("changes-stash")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(IconName::Archive))
                    .tooltip(t("Stash all changes"))
                    .disabled(!has_changes)
                    .on_click(cx.listener(|this, _, window, cx| this.open_stash(window, cx))),
            )
    }

    fn render_section_header(
        &self,
        group: ChangeGroup,
        count: usize,
        folded: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let (label, action, id) = match group {
            ChangeGroup::Staged => (t("Staged"), t("Unstage all"), "changes-unstage-all"),
            ChangeGroup::Unstaged => (t("Unstaged"), t("Stage all"), "changes-stage-all"),
            ChangeGroup::Untracked => (t("Untracked"), t("Stage all"), "changes-stage-untracked"),
        };
        let all = move |this: &mut Self, cx: &mut Context<Self>| match group {
            ChangeGroup::Staged => this.git_op(git::unstage_all, cx),
            ChangeGroup::Unstaged => this.git_op(git::stage_tracked, cx),
            ChangeGroup::Untracked => {
                let paths: Vec<String> = this.tree().map_or_else(Vec::new, |tree| {
                    tree.entries
                        .iter()
                        .filter(|e| e.kind == EntryKind::Untracked)
                        .map(|e| e.path.clone())
                        .collect()
                });
                if !paths.is_empty() {
                    this.git_op(move |top| git::stage(top, &paths), cx);
                }
            }
        };
        let weak = cx.entity().downgrade();
        // An empty Staged stays with its zero, dimmed and with nothing to act on.
        let empty = count == 0;
        h_flex()
            .h(rems(ROW_HEIGHT / 16.))
            .px_3()
            .gap_2()
            .child(
                Checkbox::new(SharedString::from(format!("{id}-check")))
                    .small()
                    .checked(group == ChangeGroup::Staged && !empty)
                    .disabled(empty)
                    .tooltip(action)
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| all(this, cx));
                    }),
            )
            // The chevron, name and count fold the group; the checkbox
            // beside them is what stages or unstages.
            .child(
                h_flex()
                    .id(SharedString::from(format!("{id}-fold")))
                    .h_full()
                    .px_1()
                    .gap_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(ui::surface_hover()))
                    .child(
                        Icon::new(if folded {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .xsmall()
                        .text_color(ui::faint()),
                    )
                    .child(
                        div()
                            .text_size(rems(13.5 / 16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if empty { ui::muted() } else { ui::fg() })
                            .child(label),
                    )
                    .child(
                        div()
                            .px_1p5()
                            .rounded_md()
                            .bg(ui::well())
                            .font_family(ui::mono())
                            .text_size(ui::code_rems(10.5))
                            .text_color(ui::muted())
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_fold(group, cx))),
            )
            .child(div().flex_1())
            .child(
                Button::new(id)
                    .outline()
                    .xsmall()
                    .label(action)
                    .disabled(empty)
                    .on_click(cx.listener(move |this, _, _, cx| all(this, cx))),
            )
    }

    fn render_line(&self, line: &Line, cx: &mut Context<Self>) -> AnyElement {
        match line {
            Line::Header {
                group,
                count,
                folded,
            } => self
                .render_section_header(*group, *count, *folded, cx)
                .into_any_element(),
            Line::Dir {
                group,
                dir,
                count,
                collapsed,
            } => self
                .render_dir(*group, dir, *count, *collapsed, cx)
                .into_any_element(),
            Line::Entry {
                group,
                entry,
                badge,
                nested,
            } => match self.tree().and_then(|tree| tree.entries.get(*entry)) {
                Some(status) => {
                    let indent = if *nested { 18. } else { 0. };
                    self.render_entry(status, *group, *badge, indent, cx)
                }
                None => div().h(rems(ROW_HEIGHT / 16.)).into_any_element(),
            },
        }
    }

    fn render_dir(
        &self,
        group: ChangeGroup,
        dir: &str,
        count: usize,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let key = (group, dir.to_string());
        h_flex()
            .id(SharedString::from(format!("changes-dir-{group:?}-{dir}")))
            .h(rems(ROW_HEIGHT / 16.))
            .mx_1p5()
            .px_1p5()
            .gap_1p5()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(ui::surface_hover()))
            .child(
                Icon::new(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .text_color(ui::faint()),
            )
            .child(
                Icon::new(if collapsed {
                    IconName::Folder
                } else {
                    IconName::FolderOpen
                })
                .small()
                .text_color(ui::muted()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(rems(12.5 / 16.))
                    .text_color(ui::muted())
                    .child(dir.to_string()),
            )
            .child(
                div()
                    .font_family(ui::mono())
                    .text_size(ui::code_rems(10.5))
                    .text_color(ui::faint())
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.collapsed.remove(&key) {
                    this.collapsed.insert(key.clone());
                }
                this.relayout();
                cx.notify();
            }))
    }

    fn render_entry(
        &self,
        entry: &StatusEntry,
        group: ChangeGroup,
        badge: char,
        indent: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self
            .selected
            .as_ref()
            .is_some_and(|(g, path)| *g == group && *path == entry.path);
        // The name first, always whole; its folder after it, quieter, giving
        // way from its start so the part nearest the file stays readable.
        let (dir, name) = match entry.path.rsplit_once('/') {
            Some((dir, name)) if indent == 0. => (dir.to_string(), name.to_string()),
            Some((_, name)) => (String::new(), name.to_string()),
            None => (String::new(), entry.path.clone()),
        };
        let deleted = badge == 'D';
        let lines = match group {
            ChangeGroup::Staged => entry.staged_lines,
            ChangeGroup::Unstaged | ChangeGroup::Untracked => entry.unstaged_lines,
        }
        .filter(|&(added, removed)| added + removed > 0);
        let id = format!("change-{group:?}-{}", entry.path);
        let weak = cx.entity().downgrade();
        let check = {
            let (weak, entry) = (weak.clone(), entry.clone());
            div()
                .id(SharedString::from(format!("{id}-check-wrap")))
                .flex_none()
                // Toggling the checkbox must not also open the diff.
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    Checkbox::new(SharedString::from(format!("{id}-check")))
                        .small()
                        .checked(group == ChangeGroup::Staged)
                        .on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.toggle(&entry, group, cx));
                        }),
                )
        };
        let path = entry.path.clone();
        let menu_entry = entry.clone();
        // A list item is laid out as its own root, so the row only gets the
        // list's width (and its folder only gives way) when it asks for it:
        // the wrapper takes the full width, the row fills it inside the inset.
        let row = h_flex()
            .id(SharedString::from(id))
            .w_full()
            .h(rems(ROW_HEIGHT / 16.))
            .pl(rems((6. + indent) / 16.))
            .pr_1p5()
            .gap_2()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |d| d.bg(ui::raised()))
            .when(!selected, |d| d.hover(|s| s.bg(ui::surface_hover())))
            .child(check)
            .child(status_badge(badge))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .items_baseline()
                    .text_size(rems(13. / 16.))
                    .child(
                        div()
                            .flex_none()
                            .max_w_full()
                            .truncate()
                            .text_color(if deleted { ui::muted() } else { ui::fg() })
                            .when(deleted, |d| d.line_through())
                            .child(name),
                    )
                    .when(!dir.is_empty(), |d| {
                        d.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis_start()
                                .text_size(rems(11.5 / 16.))
                                .text_color(ui::faint())
                                .child(dir),
                        )
                    }),
            )
            .when_some(lines, |d, (added, removed)| {
                d.child(
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .font_family(ui::mono())
                        .text_size(ui::code_rems(11.))
                        .when(added > 0, |d| {
                            d.child(div().text_color(ui::success()).child(format!("+{added}")))
                        })
                        .when(removed > 0, |d| {
                            d.child(
                                div()
                                    .text_color(ui::danger())
                                    .child(format!("\u{2212}{removed}")),
                            )
                        }),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| this.click_entry(&path, group, cx)))
            .context_menu(move |menu, _, _| {
                let entry = menu_entry.clone();
                let deleted = match group {
                    ChangeGroup::Staged => entry.index == 'D',
                    ChangeGroup::Unstaged | ChangeGroup::Untracked => entry.worktree == 'D',
                };
                let open = {
                    let (weak, path) = (weak.clone(), entry.path.clone());
                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let _ = weak.update(cx, |this, cx| {
                            if let Some(top) = this.toplevel() {
                                cx.emit(ChangesEvent::OpenFile(top.join(&path)));
                            }
                        });
                    }
                };
                let discard = {
                    let (weak, entry) = (weak.clone(), entry.clone());
                    move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        let entry = entry.clone();
                        let _ = weak.update(cx, |this, cx| {
                            this.confirm_discard(entry, group, window, cx)
                        });
                    }
                };
                let toggle = {
                    let (weak, entry) = (weak.clone(), entry.clone());
                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let _ = weak.update(cx, |this, cx| this.toggle(&entry, group, cx));
                    }
                };
                menu.item(
                    PopupMenuItem::new(t("Open file"))
                        .icon(Icon::new(IconName::File))
                        .disabled(deleted)
                        .on_click(open),
                )
                .item(
                    PopupMenuItem::new(match group {
                        ChangeGroup::Staged => t("Unstage"),
                        ChangeGroup::Unstaged | ChangeGroup::Untracked => t("Stage"),
                    })
                    .icon(Icon::new(match group {
                        ChangeGroup::Staged => IconName::Minus,
                        ChangeGroup::Unstaged | ChangeGroup::Untracked => IconName::Plus,
                    }))
                    .on_click(toggle),
                )
                .separator()
                .item(
                    PopupMenuItem::new(t("Discard changes…"))
                        .icon(Icon::new(IconName::Undo2))
                        .disabled(entry.kind == EntryKind::Conflict)
                        .on_click(discard),
                )
            });
        div().w_full().px_1p5().child(row).into_any_element()
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let repo = match &self.repo {
            None => {
                return empty_state(IconName::RefreshCw, t("Reading git status…"))
                    .into_any_element();
            }
            Some(None) => {
                return empty_state(IconName::GitBranch, t("Not a git repository"))
                    .into_any_element();
            }
            Some(Some(repo)) => repo,
        };
        if repo.tree.entries.is_empty() {
            let clean = empty_state(IconName::CircleCheck, t("Working tree clean"));
            if !self.offers_stashes || repo.stashes == 0 {
                return clean.into_any_element();
            }
            return clean
                .child(
                    Button::new("changes-show-stashes")
                        .outline()
                        .small()
                        .icon(Icon::new(IconName::Archive))
                        .label(t("Show stashes"))
                        .tooltip(t("Open the stashes in Branches"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ChangesEvent::ShowStashes))),
                )
                .into_any_element();
        }
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                uniform_list(
                    "changes-list",
                    self.lines.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        let lines = this.lines.clone();
                        lines[range]
                            .iter()
                            .map(|line| this.render_line(line, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .py_1()
                .track_scroll(&self.scroll),
            )
            .vertical_scrollbar(&self.scroll)
            .into_any_element()
    }

    fn render_commit(&self, cx: &mut Context<Self>) -> Div {
        let ready = matches!(self.repo, Some(Some(_)));
        let staged = self
            .tree()
            .is_some_and(|t| t.entries.iter().any(|e| e.staged().is_some()));
        let writes = self.summary_text(cx).is_empty() && !self.amend;
        // "Generate" alone keeps the footer inside the sidebar; the tooltip
        // and the menu spell out "Generate message".
        let (primary_label, primary_tip) = if writes {
            (t("Generate"), t("Generate message"))
        } else {
            (t("Commit"), t("Commit"))
        };
        let working = self.busy.is_some() || self.generating;
        let weak = cx.entity().downgrade();
        v_flex()
            .flex_none()
            .mx_2()
            .mb_2()
            .mt_1()
            .px_3()
            .py_2()
            .gap_1()
            .rounded_xl()
            .bg(ui::surface())
            .capture_key_down(cx.listener(Self::on_commit_key))
            .child(
                div()
                    .text_size(rems(13.5 / 16.))
                    .child(Input::new(&self.summary).appearance(false)),
            )
            .child(
                div()
                    .min_h(rems(48. / 16.))
                    .text_size(rems(13. / 16.))
                    .text_color(ui::muted())
                    .child(Textarea::new(&self.description).appearance(false)),
            )
            // The sidebar is narrower than the row's full labels: "Write it"
            // is the sparkles icon (its tooltip says what it does) and the
            // primary reads "Generate". When the row still doesn't fit, the
            // primary wraps whole to a line of its own, full width, so
            // nothing shrinks to an ellipsis or spills past the panel's edge.
            .child(
                h_flex()
                    .flex_wrap()
                    .min_w_0()
                    .pt_1()
                    .gap_1()
                    // Both grow, the primary by a negligible share: on one row
                    // the free space goes to this group, so the primary keeps
                    // its own width; alone on a wrapped line it fills it.
                    .child(
                        h_flex()
                            .flex_grow(10_000.)
                            .gap_1()
                            .child(
                                Button::new("changes-write")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(IconName::Sparkles))
                                    .loading(self.generating)
                                    .disabled(!ready || working || !staged)
                                    .tooltip(t(
                                        "Write the message from the staged diff with Gentle Shell",
                                    ))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| {
                                            this.generate(window, cx)
                                        }),
                                    ),
                            )
                            .child(
                                Button::new("changes-amend")
                                    .ghost()
                                    .xsmall()
                                    .label(t("Amend"))
                                    .selected(self.amend)
                                    .disabled(!ready || working)
                                    .tooltip(t(
                                        "Rewrite the last commit instead of making a new one",
                                    ))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_amend(window, cx)
                                    })),
                            )
                            // The status fills only the space the buttons
                            // leave, so it is what truncates first.
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .justify_end()
                                    .gap_1p5()
                                    .text_size(rems(11.5 / 16.))
                                    .text_color(ui::muted())
                                    .when_some(self.busy, |d, busy| {
                                        d.child(Spinner::new().xsmall().color(ui::accent()))
                                            .child(div().min_w_0().truncate().child(t(busy)))
                                    })
                                    .when(self.generating, |d| {
                                        d.child(div().min_w_0().truncate().child(t("Writing…")))
                                    }),
                            ),
                    )
                    // Secondary actions on the left, the primary last on the
                    // right (the design system's footer), its key in the
                    // tooltip rather than a cap on the button. It never
                    // shrinks.
                    .child(
                        DropdownButton::new("changes-commit")
                            .primary()
                            .small()
                            .flex_grow(1.)
                            .flex_shrink_0()
                            .disabled(!ready || working)
                            .button(
                                Button::new("changes-commit-main")
                                    .flex_grow(1.)
                                    .label(primary_label)
                                    .tooltip(format!("{primary_tip} ({})", ui::shortcut("↵")))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.primary(window, cx)),
                                    ),
                            )
                            .dropdown_menu(move |menu, _, _| {
                                let item =
                                    |label: &'static str, icon: IconName, push: Option<bool>| {
                                        let weak = weak.clone();
                                        PopupMenuItem::new(label).icon(Icon::new(icon)).on_click(
                                            move |_, window, cx| {
                                                let _ = weak.update(cx, |this, cx| match push {
                                                    Some(push) => this.commit(push, window, cx),
                                                    None => this.generate(window, cx),
                                                });
                                            },
                                        )
                                    };
                                menu.item(item(
                                    t("Commit"),
                                    IconName::GitCommitHorizontal,
                                    Some(false),
                                ))
                                .item(item(t("Commit & Push"), IconName::Upload, Some(true)))
                                .separator()
                                .item(item(
                                    t("Generate message"),
                                    IconName::Sparkles,
                                    None,
                                ))
                            }),
                    ),
            )
            .when_some(self.error.clone(), |d, error| {
                d.child(
                    div()
                        .id("changes-error")
                        .max_h(rems(96. / 16.))
                        .overflow_y_scroll()
                        .text_size(rems(11.5 / 16.))
                        .text_color(ui::danger())
                        .child(error.render()),
                )
            })
    }
}

impl Render for ChangesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.form_language != language() {
            sync_placeholder(
                &self.summary,
                "Summary",
                &mut self.form_language,
                window,
                cx,
            );
            self.description.update(cx, |state, cx| {
                state.set_placeholder(t("Why this change…"), window, cx)
            });
        }
        sync_placeholder(
            &self.stash_message,
            "Message (optional)",
            &mut self.stash_language,
            window,
            cx,
        );
        // The sidebar slot paints the glass; no fill here.
        v_flex()
            .size_full()
            .min_h_0()
            .child(self.render_header(cx))
            .when_some(self.repository_path(), |d, path| {
                let line = t("Repository: {path}").replace("{path}", &path);
                // The tail (the repository's own name) is what a narrow
                // sidebar cuts, so the tooltip carries the whole line.
                let tip = line.clone();
                d.child(
                    div()
                        .id("changes-repository")
                        .flex_none()
                        .px_3()
                        .pb_1p5()
                        .truncate()
                        .text_size(rems(11. / 16.))
                        .text_color(ui::muted())
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tip.clone())
                                .build(window, cx)
                        })
                        .child(line),
                )
            })
            .child(self.render_list(cx))
            .when(matches!(self.repo, Some(Some(_))), |d| {
                d.child(self.render_commit(cx))
            })
    }
}

/// The boxed status letter: A, M, D, R, `?` untracked, `U` conflict.
fn status_badge(badge: char) -> Div {
    let color = if badge == 'U' {
        ui::danger()
    } else {
        status_color(badge)
    };
    div()
        .flex_none()
        .size(rems(1.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(rems(4. / 16.))
        .border_1()
        .border_color(color.opacity(0.5))
        .bg(color.opacity(0.1))
        .font_family(ui::mono())
        .text_size(ui::code_rems(10.))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(badge.to_string())
}

fn empty_state(icon: IconName, text: &'static str) -> Div {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_2()
        .p_6()
        .child(Icon::new(icon).text_color(ui::faint()))
        .child(
            div()
                .text_size(rems(12. / 16.))
                .text_color(ui::muted())
                .child(text),
        )
}

/// Changes groups by folder unless the list was chosen: a missing or
/// invalid `changesGroupByFolder` reads as by folder.
fn read_group_by_folder(path: &Path) -> bool {
    themes::read_key(path, "changesGroupByFolder")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

fn write_group_by_folder(path: &Path, by_folder: bool) -> std::io::Result<()> {
    themes::write_key(path, "changesGroupByFolder", by_folder.into())
}

// ── Commit messages from Gentle Shell ──────────────────────────────────────

fn commit_prompt(diff: &str) -> String {
    format!(
        "Write a git commit message for the staged changes below, following Conventional \
         Commits (`type(scope): summary`, types: feat, fix, refactor, perf, docs, test, build, \
         ci, chore, style). Reply with the message only, no code fences, quotes or commentary: \
         a summary line of at most 72 characters in the imperative mood, then a blank line, \
         then a short body wrapped at 72 columns explaining what changed and why. Omit the body \
         for trivial changes.\n\n<diff>\n{diff}</diff>\n"
    )
}

/// Run the user's Gentle Shell once, in print mode without a session or
/// tools, on the staged diff. Blocking; call off the UI thread.
fn generate_message(toplevel: &Path) -> Result<(String, String), Text> {
    let diff = git::staged_diff(toplevel, DIFF_LIMIT)?;
    if diff.trim().is_empty() {
        return Err("Stage some changes first; the message is written from the staged diff".into());
    }
    let spec = crate::features::backend::shell_spec(toplevel).map_err(|e| format!("{e:#}"))?;
    // Launcher options (home selectors) come before Pi's own arguments;
    // interactive-host dialogs make no sense for a one-shot print run.
    let mut command = spec.print_command(["--print", "--no-session", "--no-tools"]);
    command.arg(commit_prompt(&diff)).stdin(Stdio::null());
    let out = command.output().map_err(|e| {
        Text::from("Could not start gentle-shell: {error}").with("{error}", e.to_string())
    })?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let detail = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        let (tail, _) = ui::tail(detail.trim(), 6);
        return Err(Text::from("gentle-shell failed: {error}").with("{error}", tail));
    }
    parse_commit_message(&stdout).ok_or_else(|| "gentle-shell returned no message".into())
}

/// `(summary, description)` from a model reply: code fences, a leading
/// "Commit message:" label and wrapping quotes are dropped.
fn parse_commit_message(reply: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = reply
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect();
    let start = lines.iter().position(|line| {
        let line = line.trim();
        !line.is_empty()
            && !line
                .trim_end_matches(':')
                .eq_ignore_ascii_case("commit message")
    })?;
    let summary = lines[start].trim();
    let summary = summary
        .strip_prefix("Summary:")
        .unwrap_or(summary)
        .trim()
        .trim_matches(|c| c == '"' || c == '`' || c == '\'')
        .trim()
        .to_string();
    let description = lines[start + 1..].join("\n").trim().to_string();
    (!summary.is_empty()).then_some((summary, description))
}

#[cfg(test)]
mod tests {
    use super::{
        ChangeGroup, ChangesView, Line, Repo, parse_commit_message, read_group_by_folder,
        write_group_by_folder,
    };
    use gentle_core::git::status::parse_status_v2;

    /// The list's rows: each header with whether it is folded, each entry
    /// with its path.
    fn rows(view: &ChangesView) -> Vec<String> {
        let tree = view.tree().unwrap();
        view.lines
            .iter()
            .map(|line| match line {
                Line::Header { group, folded, .. } => format!("{group:?} folded={folded}"),
                Line::Entry { group, entry, .. } => {
                    format!("{group:?} {}", tree.entries[*entry].path)
                }
                Line::Dir { group, dir, .. } => format!("{group:?} dir {dir}"),
            })
            .collect()
    }

    #[gpui::test]
    fn folded_group_keeps_its_header_and_survives_a_status_refresh(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let root = std::env::temp_dir();
        let (view, cx) =
            cx.add_window_view(|window, cx| ChangesView::new(root.clone(), window, cx));
        let repo = |raw: &str| Repo {
            toplevel: root.clone(),
            tree: parse_status_v2(raw),
            repository: None,
            stashes: 0,
        };
        let first = repo("1 .M N... 100644 100644 100644 aaaa bbbb a.rs\0? b.txt\0");
        let refreshed = repo("1 .M N... 100644 100644 100644 aaaa bbbb a.rs\0? b.txt\0? c.txt\0");
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                // The flat list, whatever the saved choice is.
                view.tree_mode = false;
                view.apply(Some(first), cx);
                view.toggle_fold(ChangeGroup::Untracked, cx);
                assert_eq!(
                    rows(view),
                    [
                        "Staged folded=false",
                        "Unstaged folded=false",
                        "Unstaged a.rs",
                        "Untracked folded=true"
                    ]
                );
                view.apply(Some(refreshed), cx);
                assert_eq!(
                    rows(view),
                    [
                        "Staged folded=false",
                        "Unstaged folded=false",
                        "Unstaged a.rs",
                        "Untracked folded=true"
                    ]
                );
                view.toggle_fold(ChangeGroup::Untracked, cx);
                assert_eq!(
                    rows(view),
                    [
                        "Staged folded=false",
                        "Unstaged folded=false",
                        "Unstaged a.rs",
                        "Untracked folded=false",
                        "Untracked b.txt",
                        "Untracked c.txt"
                    ]
                );
            });
        });
    }

    #[test]
    fn group_by_folder_is_the_default_and_a_saved_list_is_kept() {
        let dir = std::env::temp_dir().join(format!(
            "gentle-changes-view-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert!(read_group_by_folder(&path), "no settings file");
        std::fs::write(&path, r#"{"changesGroupByFolder":"no"}"#).unwrap();
        assert!(read_group_by_folder(&path), "invalid value");
        std::fs::write(&path, r#"{"theme":"Studio"}"#).unwrap();
        write_group_by_folder(&path, false).unwrap();
        assert!(!read_group_by_folder(&path));
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["theme"], "Studio");
        write_group_by_folder(&path, true).unwrap();
        assert!(read_group_by_folder(&path));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn commit_message_summary_and_body() {
        let reply = "feat(files): add changes tab\n\nSplit the working tree into staged\nand unstaged sections.\n";
        assert_eq!(
            parse_commit_message(reply),
            Some((
                "feat(files): add changes tab".into(),
                "Split the working tree into staged\nand unstaged sections.".into()
            ))
        );
    }

    #[test]
    fn commit_message_strips_fences_labels_and_quotes() {
        let reply = "Commit message:\n```\n\"fix: handle detached HEAD\"\n```\n";
        assert_eq!(
            parse_commit_message(reply),
            Some(("fix: handle detached HEAD".into(), String::new()))
        );
        assert_eq!(parse_commit_message("\n```\n```\n"), None);
    }
}
