//! The `@` file menu over the composer: typing `@` searches the tab's folder
//! the way a terminal would (ignore-aware, with the switcher's file ranking),
//! Tab completes a folder prefix or fills the file the search found, Enter
//! puts the file's `@path` into the sentence where the reference was typed —
//! as plain text in the prompt, not as a tile, and without sending — and Esc
//! closes the menu.
//!
//! The files are read off the UI thread, once per folder, and only the folder
//! the tab is in now is ever offered — drawn, picked or completed: an index or
//! a read for a folder the tab has left is dropped, and a folder whose real
//! path leaves the tab (a symlink out) is never listed. A completion lands only
//! when the composer still holds the text it was asked for and no newer Tab or
//! Esc superseded it; the typed `/` is looked up as the platform spells paths.
//! Enter pressed while the folder is still being read waits for its files
//! instead of sending the draft with the raw `@query`.

use crate::shared::i18n::t;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::files::{file_icon, list_dir};
use crate::features::palette::{MAX_FILES, walk_files};
use crate::features::session::SessionView;
use crate::shared::ui;
use gentle_core::fuzzy::{FileEntry, Needle, rank_files};
use gentle_core::markdown::paths;

/// Rows the menu lists; typing narrows them.
const ROWS: usize = 8;
/// The menu shows about eight rows; the rest scroll.
const MENU_H: f32 = 256.;

/// The `@` reference the caret sits in: where its `@` starts (a byte offset)
/// and what was typed after it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Mention {
    at: usize,
    query: String,
}

/// The reference the caret is in, or `None` when the caret isn't in one: the
/// `@` opens a word (the text starts there, or a space comes before it) and
/// nothing separates it from the caret. An address (`me@example.com`) is not
/// a reference: its `@` follows a letter.
fn mention_at(text: &str, caret: usize) -> Option<Mention> {
    if caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    let before = &text[..caret];
    let start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let query = before[start..].strip_prefix('@')?;
    Some(Mention {
        at: start,
        query: query.to_string(),
    })
}

/// What choosing the file at `path` puts in the composer: its `@path`, and a
/// space when the reference ends the text, so the next word typed doesn't
/// stick to it. Text after the reference is left exactly as it was.
fn insertion(path: &str, at_end: bool) -> String {
    if at_end {
        format!("@{path} ")
    } else {
        format!("@{path}")
    }
}

/// The query with its last segment completed from the names of its folder:
/// the longest prefix the matching names share, plus `/` when one folder
/// answers. `None` when nothing would be added (no match, or the prefix
/// already typed).
fn completed_query(query: &str, names: &[(String, bool)]) -> Option<String> {
    let head_at = query.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let (folder, head) = query.split_at(head_at);
    let lower = head.to_lowercase();
    let matches: Vec<(&str, bool)> = names
        .iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&lower))
        .map(|(name, dir)| (name.as_str(), *dir))
        .collect();
    let (first, folder_answers) = *matches.first()?;
    let mut completed = matches.iter().fold(first.to_string(), |shared, (name, _)| {
        common_prefix(&shared, name)
    });
    if matches.len() == 1 && folder_answers {
        completed.push('/');
    }
    if completed.len() <= head.len() {
        return None;
    }
    Some(format!("{folder}{completed}"))
}

/// The folder a reference's typed path names, under the tab's folder, or
/// `None` when it would leave it (an absolute path, `~`, or a `..` segment):
/// the search never looks outside the tab.
fn under_cwd(cwd: &Path, query: &str) -> Option<PathBuf> {
    if query.starts_with(['/', '\\', '~'])
        || query.split(['/', '\\']).any(|segment| segment == "..")
    {
        return None;
    }
    let folder = query.rfind(['/', '\\']).map_or("", |i| &query[..i]);
    Some(cwd.join(folder))
}

/// The names in `folder` when it really is inside `cwd`, read off the UI
/// thread (blocking I/O): a folder reached through a symlink that leaves the
/// tab lists nothing, so the search never shows what is outside it.
fn read_folder(cwd: PathBuf, folder: PathBuf) -> Option<Vec<(String, bool)>> {
    let root = cwd.canonicalize().ok()?;
    let dir = folder.canonicalize().ok()?;
    if !dir.starts_with(&root) {
        return None;
    }
    Some(
        list_dir(&dir, false)
            .into_iter()
            .map(|entry| (entry.name, entry.is_dir))
            .collect(),
    )
}

/// The longest prefix two names share.
fn common_prefix(a: &str, b: &str) -> String {
    a.chars()
        .zip(b.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a)
        .collect()
}

/// A row of the menu: a file under the tab's folder.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    /// The path the reference names, relative to the tab's folder.
    path: String,
    name: String,
    /// The folders above the file, shown muted before its name.
    folder: String,
}

/// `query` with its typed `/` spelled as `separator` — the platform's, which
/// is how a walker spells its paths (a Windows one yields `src\main.rs`). A
/// `\` is only a separator where the platform says so: on Unix it is a
/// character a file's name can hold, so it is left exactly as typed.
fn native(query: &str, separator: char) -> String {
    if separator == '/' {
        return query.to_string();
    }
    query.replace('/', &separator.to_string())
}

/// The rows `query` matches among `files`: the best names first, then the
/// shorter paths, as the switcher ranks them. `separator` is the platform's
/// path separator, which the query's typed `/` stands for.
fn rows(files: &[FileEntry], query: &str, separator: char) -> Vec<Row> {
    let needle = Needle::new(&native(query, separator));
    rank_files(files, &needle.lower, ROWS)
        .into_iter()
        .map(|i| {
            let path = files[i].path.clone();
            // The reference the mention writes reads as typed (`/`), on every
            // platform: a Windows walker spells `src\main.rs`, so its `\`
            // becomes `/` here. A `\` on Unix is a character a name can
            // hold, so it is left as it is.
            let spelled = if separator == '\\' {
                path.replace('\\', "/")
            } else {
                path.clone()
            };
            let (folder, name) = paths::split(&spelled);
            let (folder, name) = (folder.to_string(), name.to_string());
            Row {
                path: spelled,
                name,
                folder,
            }
        })
        .collect()
}

/// A completion asked for, and the request it is: the text and caret it was
/// asked about, the reference it completes, and which request it is (a newer
/// one, or an Esc, supersedes it and its answer is dropped).
#[derive(Clone)]
struct Asked {
    text: String,
    caret: usize,
    reference: Mention,
    /// The folder it was asked in: one the tab has left is not applied.
    folder: PathBuf,
    epoch: u64,
}

/// The menu's files and highlight: the folder they were read from, the
/// highlight for the query it was moved on, and the text Esc closed it for.
#[derive(Default)]
pub(in crate::features::session) struct PathMenu {
    /// Files under the tab's folder, with the folder they were read from.
    index: Option<(PathBuf, Arc<Vec<FileEntry>>)>,
    /// The folder whose walk is running, so it isn't asked twice.
    walking: Option<PathBuf>,
    /// The composer text Esc closed the menu for.
    dismissed: Option<String>,
    /// Completion requests so far: the latest one's answer may land, and an
    /// Esc takes that away from it.
    completions: u64,
    /// The highlighted row, for `query` (another query starts at the top).
    cursor: usize,
    query: String,
    scroll: ScrollHandle,
}

impl SessionView {
    /// Read the tab folder's files for `@` search — once, off the UI thread,
    /// the first time the composer holds a reference. An answer for a folder
    /// the tab has left while the walk ran is dropped, and the menu asks for
    /// the new folder on its next frame.
    pub(in crate::features::session) fn ensure_path_files(&mut self, cx: &mut Context<Self>) {
        if !self.composer.read(cx).value().contains('@') {
            return;
        }
        let cwd = self.cwd.clone();
        let asked = self.path_menu.index.as_ref().map(|(at, _)| at) == Some(&cwd)
            || self.path_menu.walking.as_ref() == Some(&cwd);
        if asked {
            return;
        }
        self.path_menu.walking = Some(cwd.clone());
        let root = cwd.clone();
        let walking = cx.background_executor().spawn(async move {
            walk_files(&root, MAX_FILES)
                .into_iter()
                .map(FileEntry::new)
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = walking.await;
            let _ = this.update(cx, |this, cx| {
                if this.cwd != cwd {
                    // The tab left that folder while the walk ran: its files
                    // would name paths the prompt's folder doesn't have.
                    this.path_menu.walking = None;
                    return;
                }
                this.path_menu.index = Some((cwd, Arc::new(files)));
                this.path_menu.walking = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// The menu's rows for `query`, best match first. Empty while the folder
    /// is still being read, for a bare `@` (Tab completes that one), and for
    /// an index read in a folder the tab has since left.
    fn path_rows(&self, query: &str) -> Vec<Row> {
        if query.is_empty() {
            return Vec::new();
        }
        match &self.path_menu.index {
            Some((at, files)) if *at == self.cwd => rows(files, query, std::path::MAIN_SEPARATOR),
            _ => Vec::new(),
        }
    }

    /// Whether the tab folder's files are read: the menu then shows what
    /// `query` matches, a reference that matches nothing included.
    fn path_files_ready(&self) -> bool {
        self.path_menu
            .index
            .as_ref()
            .is_some_and(|(at, _)| *at == self.cwd)
    }

    /// The highlighted row of the menu showing `count` rows for `query`.
    fn path_cursor(&self, query: &str, count: usize) -> usize {
        if self.path_menu.query == query {
            self.path_menu.cursor.min(count.saturating_sub(1))
        } else {
            0
        }
    }

    /// Put `insert` in place of the reference `at..end`, caret after it, and
    /// leave the rest of the text as it was. Edited in place rather than
    /// written from a string, so a long paste's chip keeps its span.
    fn replace_mention(
        &mut self,
        at: usize,
        end: usize,
        insert: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composer.update(cx, |state, cx| {
            state.set_selected_range(at..end, cx);
            state.replace(insert, window, cx);
            state.focus(window, cx);
        });
    }

    /// Choose the file at `path` — a row drawn for `folder`, as byte
    /// `at..end` of the text — as the reference: its `@path` goes into the
    /// sentence (plain text: no tile, nothing sent) and the caret lands after
    /// it, in the text that followed. A row drawn in a folder the tab has
    /// left names a path that isn't here, so it picks nothing.
    fn choose_path(
        &mut self,
        at: usize,
        end: usize,
        path: &str,
        folder: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if folder != self.cwd {
            return;
        }
        let text = self.composer.read(cx).value().to_string();
        let insert = insertion(path, end == text.len());
        self.replace_mention(at, end, &insert, window, cx);
        cx.notify();
    }

    /// Fill the highlighted file of the menu for `query` — as Enter fills it:
    /// its `@path` goes into the sentence, no tile is made and nothing is
    /// sent. Whether there was a row to fill.
    fn fill_highlighted(
        &mut self,
        query: &str,
        at: usize,
        end: usize,
        folder: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let rows = self.path_rows(query);
        if rows.is_empty() {
            return false;
        }
        let cursor = self.path_cursor(query, rows.len());
        let path = rows[cursor].path.clone();
        self.choose_path(at, end, &path, folder, window, cx);
        cx.notify();
        true
    }

    /// Tab on a reference: read the folder of the segment being typed off the
    /// UI thread, then complete it. The completion lands only while the
    /// composer still holds the text and caret it was asked for.
    fn complete_path(
        &mut self,
        mention: &Mention,
        text: String,
        caret: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(folder) = under_cwd(&self.cwd, &mention.query) else {
            return false;
        };
        self.path_menu.completions += 1;
        let asked = Asked {
            text,
            caret,
            reference: mention.clone(),
            folder: self.cwd.clone(),
            epoch: self.path_menu.completions,
        };
        let cwd = self.cwd.clone();
        let reading = cx
            .background_executor()
            .spawn(async move { read_folder(cwd, folder) });
        cx.spawn_in(window, async move |this, cx| {
            let names = reading.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.complete_landed(&asked, names, window, cx)
            });
        })
        .detach();
        true
    }

    /// Apply the completion read for `asked` when it is still the latest one
    /// and the composer still holds the text and caret it was asked about: an
    /// answer a newer Tab or an Esc superseded is dropped, as is one for text
    /// the person has since typed over. A folder that answers and adds to the
    /// prefix completes it; anything else (no such folder inside the tab, or
    /// names that add nothing — a reference like `@ui/btn` that only ranks by
    /// the index) leaves Tab its other job: filling the highlighted file from
    /// the tab's own index, as Enter fills it.
    fn complete_landed(
        &mut self,
        asked: &Asked,
        names: Option<Vec<(String, bool)>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.path_menu.completions != asked.epoch || asked.folder != self.cwd {
            return;
        }
        let current = {
            let state = self.composer.read(cx);
            state.value() == asked.text && state.cursor() == asked.caret
        };
        if !current {
            return;
        }
        let completed = names
            .as_deref()
            .and_then(|names| completed_query(&asked.reference.query, names));
        if let Some(completed) = completed {
            let insert = format!("@{completed}");
            self.replace_mention(asked.reference.at, asked.caret, &insert, window, cx);
            cx.notify();
            return;
        }
        self.fill_highlighted(
            &asked.reference.query,
            asked.reference.at,
            asked.caret,
            &asked.folder,
            window,
            cx,
        );
    }

    /// ↑/↓ move through the `@` menu, Tab completes the folder prefix being
    /// typed or fills the file the search found (the row the arrows chose
    /// when one did), Enter puts the highlighted file's `@path` in the
    /// sentence without sending, Esc closes the menu. Runs as a keystroke interceptor
    /// before ↑/↓ history and the `/` menu, so the arrows are the menu's while
    /// it is open. Enter also belongs to the menu while the folder's files are
    /// still being read: sending then would post the raw `@query` instead of
    /// the file it names.
    pub(in crate::features::session) fn path_menu_key(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt || m.shift {
            return false;
        }
        if !self.composer.focus_handle(cx).is_focused(window) {
            return false;
        }
        let (text, caret) = {
            let state = self.composer.read(cx);
            (state.value().to_string(), state.cursor())
        };
        let Some(mention) = mention_at(&text, caret) else {
            return false;
        };
        // Tab completes the reference: a row the arrows moved the highlight
        // to is filled as Enter fills it; otherwise the folder prefix is
        // completed (whether or not the menu lists anything yet), and a query
        // no prefix completes fills the highlighted file when the folder
        // answers (`complete_landed`).
        if keystroke.key == "tab" {
            let count = self.path_rows(&mention.query).len();
            if self.path_cursor(&mention.query, count) != 0 {
                let folder = self.cwd.clone();
                if self.fill_highlighted(&mention.query, mention.at, caret, &folder, window, cx) {
                    return true;
                }
            }
            return self.complete_path(&mention, text, caret, window, cx);
        }
        if self.path_menu.dismissed.as_deref() == Some(text.as_str()) {
            return false;
        }
        // Esc closes the reference and cancels the completion still being
        // read for it, so the next Esc is the composer's again.
        if keystroke.key == "escape" {
            if mention.query.is_empty() {
                return false;
            }
            self.path_menu.dismissed = Some(text);
            self.path_menu.completions += 1;
            self.live.escape_closed = Some(std::time::Instant::now());
            cx.notify();
            return true;
        }
        let rows = self.path_rows(&mention.query);
        if rows.is_empty() {
            // Nothing to choose from yet: while the folder is still being
            // read, Enter waits for its files (and sends nothing); a
            // reference no file answers is the composer's, as it always was.
            let waiting = keystroke.key == "enter"
                && !mention.query.is_empty()
                && under_cwd(&self.cwd, &mention.query).is_some()
                && !self.path_files_ready();
            if waiting {
                self.ensure_path_files(cx);
            }
            return waiting;
        }
        let count = rows.len();
        let cursor = self.path_cursor(&mention.query, count);
        match keystroke.key.as_str() {
            "up" | "down" => {
                let next = if keystroke.key == "up" {
                    (cursor + count - 1) % count
                } else {
                    (cursor + 1) % count
                };
                self.path_menu.query = mention.query.clone();
                self.path_menu.cursor = next;
                self.path_menu.scroll.scroll_to_item(next);
            }
            "enter" => {
                let path = rows[cursor].path.clone();
                let folder = self.cwd.clone();
                self.choose_path(mention.at, caret, &path, &folder, window, cx);
            }
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The `@` menu: the files the reference under the caret matches, drawn
    /// over the composer.
    pub(super) fn render_path_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (text, caret) = {
            let state = self.composer.read(cx);
            (state.value().to_string(), state.cursor())
        };
        let mention = mention_at(&text, caret)?;
        if self.path_menu.dismissed.as_deref() == Some(text.as_str()) {
            return None;
        }
        let rows = self.path_rows(&mention.query);
        if rows.is_empty() {
            return None;
        }
        let cursor = self.path_cursor(&mention.query, rows.len());
        let end = caret;
        let at = mention.at;
        // The folder the rows were read in: a click after the tab moved on
        // picks nothing (see `choose_path`).
        let drawn_in = self.cwd.clone();
        Some(
            ui::pop_in(
                "path-menu",
                v_flex()
                    .debug_selector(|| "path-menu".into())
                    .w_full()
                    .p_1()
                    .rounded_xl()
                    .border_1()
                    .border_color(ui::border_strong())
                    .bg(ui::surface())
                    .shadow_lg()
                    .font_family(cx.theme().font_family.clone())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .px_2()
                            .pt_1()
                            .pb_1()
                            .child(ui::section_label(t("Files")))
                            .child(
                                div()
                                    .text_size(rems(10. / 16.))
                                    .text_color(ui::faint())
                                    .child(t("↵ insert · tab folders")),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("path-list")
                            .max_h(rems(MENU_H / 16.))
                            .overflow_y_scroll()
                            .track_scroll(&self.path_menu.scroll)
                            .children(rows.into_iter().enumerate().map(|(i, row)| {
                                let (icon, tint) = file_icon(&row.name);
                                let path = row.path.clone();
                                let drawn_in = drawn_in.clone();
                                h_flex()
                                    .id(("path-row", i))
                                    .debug_selector(move || format!("path-row-{i}"))
                                    .w_full()
                                    .px_2()
                                    .py_1p5()
                                    .gap_2()
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(ui::surface_hover()))
                                    .when(i == cursor, |s| s.bg(ui::surface_hover()))
                                    .child(Icon::new(icon).xsmall().text_color(tint))
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .font_family(ui::mono())
                                            .text_size(ui::code_rems(12.))
                                            .when(!row.folder.is_empty(), |line| {
                                                line.child(
                                                    div()
                                                        .min_w_0()
                                                        .truncate()
                                                        .text_color(ui::faint())
                                                        .child(SharedString::from(row.folder)),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .text_color(ui::fg())
                                                    .child(SharedString::from(row.name)),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.choose_path(at, end, &path, &drawn_in, window, cx)
                                    }))
                            })),
                    ),
            )
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use gpui_kit::{Entity, Keystroke, TestAppContext, VisualTestContext};

    use super::{Mention, completed_query, mention_at, under_cwd};
    use crate::features::session::SessionView;
    use crate::features::session::attachments;
    use gentle_core::fuzzy::FileEntry;

    /// A folder of `files` for a test, named after the test so two tests
    /// never share one.
    fn folder_with(tag: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gs-path-menu-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for file in files {
            let path = dir.join(file);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, "x").unwrap();
        }
        dir
    }

    /// The folder most tests read.
    fn folder(tag: &str) -> PathBuf {
        folder_with(tag, &["alpha.md", "beta.md", "src/main.rs"])
    }

    /// The rows the menu would list for `query`, as paths.
    fn row_paths(
        view: &Entity<SessionView>,
        cx: &mut VisualTestContext,
        query: &str,
    ) -> Vec<String> {
        view.read_with(cx, |view, _| {
            view.path_rows(query)
                .into_iter()
                .map(|row| row.path)
                .collect()
        })
    }

    /// A folder symlink, as the platform makes one (Windows needs developer
    /// mode or the right to create one, so the caller copes with a refusal).
    fn link_folder(target: &std::path::Path, at: &std::path::Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, at)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(target, at)
        }
    }

    /// A tab rooted at `cwd`, in a headless window that never starts a
    /// backend.
    fn tab(cx: &mut TestAppContext, cwd: PathBuf) -> (Entity<SessionView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        cx.add_window_view(|window, cx| {
            let mut view = SessionView::new(cwd, None, window, cx, false);
            view.backend.started = true;
            view
        })
    }

    /// Put `text` in the composer with the caret at byte `caret`, focused as
    /// typing it would leave it, leaving the folder's files still unread: the
    /// window a quick key press falls into.
    fn type_now(view: &Entity<SessionView>, cx: &mut VisualTestContext, text: &str, caret: usize) {
        view.update_in(cx, |view, window, cx| {
            view.composer.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_value(text, window, cx);
                state.set_selected_range(caret..caret, cx);
            });
        });
    }

    /// The same, once the folder's files have landed.
    fn type_at(view: &Entity<SessionView>, cx: &mut VisualTestContext, text: &str, caret: usize) {
        type_now(view, cx, text, caret);
        cx.run_until_parked();
    }

    /// A key dispatched as it is pressed, without letting the background
    /// executor run: two of these arrive while what the first one started is
    /// still in flight (the window `simulate_keystrokes` would park after
    /// each one).
    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = cx
            .update(|_, cx| Keystroke::parse_with_os(key, cx.platform_info().os))
            .expect("a keystroke");
        cx.update(|window, cx| window.dispatch_keystroke(keystroke, cx));
    }

    /// Enter as GPUI parses it, for the interceptor the window calls.
    fn enter(cx: &mut VisualTestContext) -> Keystroke {
        cx.update(|_, cx| Keystroke::parse_with_os("enter", cx.platform_info().os))
            .expect("enter is a keystroke")
    }

    /// What the composer holds.
    fn composer_text(view: &Entity<SessionView>, cx: &mut VisualTestContext) -> String {
        view.read_with(cx, |view, cx| view.composer.read(cx).value().to_string())
    }

    /// Whether `selector` was drawn in the last frame.
    fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
        cx.run_until_parked();
        cx.debug_bounds(selector).is_some()
    }

    #[test]
    fn a_reference_opens_a_word_and_never_an_address() {
        let mention = |text: &str, caret: usize| mention_at(text, caret).map(|m| (m.at, m.query));

        assert_eq!(mention("@sr", 3), Some((0, "sr".into())));
        assert_eq!(mention("see @ma here", 7), Some((4, "ma".into())));
        // An address' `@` follows a letter: not a reference.
        assert_eq!(mention("mail me@example.com", 18), None);
        assert_eq!(mention("mail me@exa", 11), None);
        // A caret outside the reference names nothing.
        assert_eq!(mention("see @ma", 3), None);
        assert_eq!(mention("see @ma", 9), None, "past the text's end");
        assert_eq!(mention("at@ 12 more", 3), None);
        // Typed before the `@` of a word that starts with one.
        assert_eq!(mention("@src/main.rs and", 7), Some((0, "src/ma".into())));
    }

    #[test]
    fn tab_completes_the_longest_shared_prefix() {
        let names = |list: &[(&str, bool)]| {
            list.iter()
                .map(|(name, dir)| (name.to_string(), *dir))
                .collect::<Vec<_>>()
        };
        let icons = names(&[("src", true), ("styles", true), ("README.md", false)]);
        // One folder answers: it is completed and opened.
        assert_eq!(
            completed_query("sr", &icons).as_deref(),
            Some("src/"),
            "a single folder gets its slash"
        );
        assert_eq!(
            completed_query("src/main", &names(&[("main.rs", false)])).as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(
            completed_query("src/co", &names(&[("components", true)])).as_deref(),
            Some("src/components/")
        );
        // Two folders share only what was typed: nothing to add.
        assert_eq!(completed_query("s", &icons), None);
        assert_eq!(completed_query("zz", &icons), None);
        assert_eq!(
            completed_query("src/main.rs", &names(&[("main.rs", false)])),
            None,
            "a name typed in full adds nothing"
        );
    }

    /// Enter pressed before the folder's files have been read waits for them
    /// instead of sending the draft: the press is the menu's (consumed, so the
    /// composer never sees it and cannot submit), the draft is untouched, and
    /// the same key picks the file once the menu lists it.
    #[gpui::test]
    fn enter_waits_for_the_files_instead_of_sending_the_draft(cx: &mut TestAppContext) {
        let dir = folder("enter-race");
        let (view, cx) = tab(cx, dir.clone());

        type_now(&view, cx, "look at @alp", 12);
        let key = enter(cx);
        let consumed = view.update_in(cx, |view, window, cx| view.path_menu_key(&key, window, cx));
        assert!(consumed, "the loading picker takes Enter");
        assert_eq!(
            composer_text(&view, cx),
            "look at @alp",
            "the draft is untouched: nothing sent, nothing inserted"
        );
        assert!(
            view.read_with(cx, |view, _| view.live.pending.is_none()),
            "no prompt is in flight"
        );

        // The files land: the same key now picks the file.
        assert!(drawn(cx, "path-menu"));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "look at @alpha.md ");

        // A reference that matches no file is not the menu's: Enter is the
        // composer's again, as it always was for `@word`.
        type_at(&view, cx, "hello @zzzz", 12);
        let key = enter(cx);
        let consumed = view.update_in(cx, |view, window, cx| view.path_menu_key(&key, window, cx));
        assert!(!consumed, "nothing to choose: Enter sends as typed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A completion read asked for before Esc never lands: Esc closes the
    /// reference and drops the answer still being read.
    #[gpui::test]
    fn a_completion_asked_before_escape_never_lands(cx: &mut TestAppContext) {
        let dir = folder("escape");
        let (view, cx) = tab(cx, dir.clone());

        type_now(&view, cx, "read @sr", 8);
        press(cx, "tab");
        press(cx, "escape");
        cx.run_until_parked();

        assert_eq!(
            composer_text(&view, cx),
            "read @sr",
            "the completion asked before Esc was dropped"
        );
        assert!(!drawn(cx, "path-menu"), "and the menu stayed closed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A folder moved away from while its files are still being read: the
    /// files of the folder the tab left are never offered (drawn or picked),
    /// and the new folder's files are once they land.
    #[gpui::test]
    fn the_files_of_a_folder_the_tab_left_are_never_offered(cx: &mut TestAppContext) {
        let a = folder("cwd-a");
        let b = folder_with("cwd-b", &["gamma.md"]);
        let (view, cx) = tab(cx, a.clone());

        type_at(&view, cx, "@alp", 4);
        assert_eq!(row_paths(&view, cx, "alp"), vec!["alpha.md".to_string()]);

        // The tab moves to B while B's files are still unread: A's index is
        // still in hand, and it must not answer for B.
        view.update(cx, |view, cx| {
            view.cwd = b.clone();
            cx.notify();
        });
        assert!(
            row_paths(&view, cx, "alp").is_empty(),
            "the folder the tab left offers nothing"
        );
        assert!(row_paths(&view, cx, "gam").is_empty(), "B is unread as yet");
        assert!(!drawn(cx, "path-menu"), "and the menu is closed");

        // B's files land: they are the ones offered, and only they.
        assert_eq!(row_paths(&view, cx, "gam"), vec!["gamma.md".to_string()]);
        assert!(row_paths(&view, cx, "alp").is_empty());
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    /// A click on a drawn row inserts the file it shows; the choice itself
    /// belongs to the folder it was drawn in, so one made after the tab moved
    /// on picks nothing.
    #[gpui::test]
    fn a_choice_from_another_folder_picks_nothing(cx: &mut TestAppContext) {
        let a = folder("click-a");
        let b = folder_with("click-b", &["gamma.md"]);
        let (view, cx) = tab(cx, a.clone());

        // The click's own path: a row drawn for A inserts A's file.
        type_at(&view, cx, "see @alp", 8);
        let row = cx
            .debug_bounds("path-row-0")
            .expect("the menu lists A's file")
            .center();
        cx.simulate_click(row, gpui_kit::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "see @alpha.md ");

        // The tab moves on: the same choice, made for the folder it was drawn
        // in, picks nothing.
        type_at(&view, cx, "see @alp", 8);
        view.update(cx, |view, _| view.cwd = b.clone());
        view.update_in(cx, |view, window, cx| {
            view.choose_path(4, 8, "alpha.md", &a, window, cx)
        });
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "see @alp");
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    /// A completion read for the folder the tab has left never lands.
    #[gpui::test]
    fn a_completion_from_another_folder_never_lands(cx: &mut TestAppContext) {
        let a = folder("complete-a");
        let b = folder_with("complete-b", &["gamma.md"]);
        let (view, cx) = tab(cx, a.clone());

        type_now(&view, cx, "read @sr", 8);
        press(cx, "tab");
        view.update(cx, |view, _| view.cwd = b.clone());
        cx.run_until_parked();
        assert_eq!(
            composer_text(&view, cx),
            "read @sr",
            "the read was for the folder the tab left"
        );
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    /// A reference typed (and completed) with `/` finds the paths a Windows
    /// walker spells with `\\`, while a `\` on Unix stays the character a file
    /// name can hold.
    #[test]
    fn a_reference_spelled_with_slashes_finds_native_paths() {
        let windows = [FileEntry::new("src\\main.rs".to_string())];
        assert_eq!(
            super::rows(&windows, "src/ma", '\\')
                .into_iter()
                .map(|row| row.path)
                .collect::<Vec<_>>(),
            vec!["src/main.rs".to_string()],
            "the typed slash matches the walker's separator, and the row reads as typed"
        );
        assert!(
            super::rows(&windows, "src/ma", '/').is_empty(),
            "on Unix that spelling is not a path a walker yields"
        );
        assert_eq!(super::native("a\\b/ma", '/'), "a\\b/ma", "kept as typed");
        assert_eq!(super::native("a/b\\c", '\\'), "a\\b\\c");
    }

    /// A folder reached through a symlink that leaves the tab lists nothing:
    /// the completion never shows what is outside the tab's folder.
    #[test]
    fn a_folder_outside_the_tabs_folder_lists_nothing() {
        let tab = folder("symlink-tab");
        let outside = folder_with("symlink-outside", &["secret.md"]);

        // A folder that isn't the tab's is not listed, wherever it is...
        assert!(
            super::read_folder(tab.clone(), outside.clone()).is_none(),
            "a folder outside the tab lists nothing"
        );
        // ...nor one a link inside the tab walks out to.
        if link_folder(&outside, &tab.join("link")).is_ok() {
            assert!(
                super::read_folder(tab.clone(), tab.join("link")).is_none(),
                "a link out of the tab is not listed"
            );
        }
        assert!(
            super::read_folder(tab.clone(), tab.join("src")).is_some(),
            "a folder inside it is"
        );
        std::fs::remove_dir_all(&tab).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }

    /// The folder a reference names is read under the tab's folder, never
    /// outside it.
    #[test]
    fn a_reference_never_leaves_the_tabs_folder() {
        let cwd = Path::new("/work");
        assert_eq!(under_cwd(cwd, "sr"), Some(PathBuf::from("/work")));
        assert_eq!(
            under_cwd(cwd, "src/co"),
            Some(PathBuf::from("/work/src")),
            "the segment's folder, under the tab"
        );
        assert_eq!(under_cwd(cwd, "../etc"), None);
        assert_eq!(under_cwd(cwd, "src/../../etc"), None);
        assert_eq!(under_cwd(cwd, "/etc"), None);
        assert_eq!(under_cwd(cwd, "~/docs"), None);
    }

    /// Enter puts the chosen file's `@path` into the sentence as plain text:
    /// what the prompt would carry is that text, nothing became a tile and
    /// nothing was sent.
    #[gpui::test]
    fn enter_writes_the_files_path_into_the_sentence(cx: &mut TestAppContext) {
        let dir = folder("select");
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "look at @alp", 12);
        assert!(drawn(cx, "path-menu"), "the reference lists its files");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        assert_eq!(composer_text(&view, cx), "look at @alpha.md ");
        assert!(!drawn(cx, "path-menu"), "the menu closes on the choice");
        let view_state = view.read_with(cx, |view, _| {
            (
                view.files.len(),
                view.attachments.len(),
                view.live.pending.is_none(),
            )
        });
        assert_eq!(
            view_state,
            (0, 0, true),
            "no tile, and the prompt was not sent"
        );
        let sent = view.read_with(cx, |view, cx| {
            attachments::expand(&view.composer.read(cx).value(), &view.pasted)
        });
        assert_eq!(sent, "look at @alpha.md ", "the reference goes out as text");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The reference is replaced where it was typed: the words around it stay
    /// exactly as they were.
    #[gpui::test]
    fn the_caret_replaces_only_the_reference(cx: &mut TestAppContext) {
        let dir = folder("suffix");
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "compare @alp with bb.md", 12);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "compare @alpha.md with bb.md");
        assert_eq!(
            view.read_with(cx, |view, _| view.files.len()),
            0,
            "an inline reference is not an attachment"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Tab completes the folder prefix under the caret and types it, without
    /// sending or listing a tile.
    #[gpui::test]
    fn tab_completes_a_folder_prefix(cx: &mut TestAppContext) {
        let dir = folder("tab");
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "read @sr", 8);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "read @src/");
        assert_eq!(view.read_with(cx, |view, _| view.files.len()), 0);
        // The search stays inside the tab's folder.
        type_at(&view, cx, "@../", 4);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "@../");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Tab fills the file the search found when no folder prefix completes
    /// it (`@min` reaches `src/main.rs` through the ranking): plain text in
    /// the sentence, no tile, nothing sent.
    #[gpui::test]
    fn tab_fills_the_file_the_search_found(cx: &mut TestAppContext) {
        let dir = folder("tab-fill");
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "open @min", 9);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "open @src/main.rs ");
        assert!(!drawn(cx, "path-menu"), "the menu closes on the choice");
        assert_eq!(
            view.read_with(cx, |view, _| (
                view.files.len(),
                view.attachments.len(),
                view.live.pending.is_none()
            )),
            (0, 0, true),
            "no tile, and nothing was sent"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A fuzzy match under a folder the typed prefix doesn't spell (`@ui/btn`
    /// finds `src/ui/button.rs`) is filled by Tab as Enter fills it: the read
    /// of `ui` finds no such folder, so the tab's own index row is used.
    #[gpui::test]
    fn tab_fills_a_file_whose_prefix_is_not_a_folder(cx: &mut TestAppContext) {
        let dir = folder_with("tab-deep", &["alpha.md", "src/ui/button.rs"]);
        let (view, cx) = tab(cx, dir.clone());

        // The menu finds it by ranking, and Enter fills it.
        type_at(&view, cx, "open @ui/btn", 12);
        assert_eq!(
            row_paths(&view, cx, "ui/btn"),
            vec!["src/ui/button.rs".to_string()]
        );

        // Tab fills the same file, though `ui` is no folder here.
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "open @src/ui/button.rs ");
        assert!(!drawn(cx, "path-menu"), "the menu closes on the choice");
        assert_eq!(
            view.read_with(cx, |view, _| (
                view.files.len(),
                view.attachments.len(),
                view.live.pending.is_none()
            )),
            (0, 0, true),
            "no tile, and nothing was sent"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A reference that leaves the tab through a link fills nothing: the read
    /// is refused and no indexed row names anything outside the tab.
    #[gpui::test]
    fn tab_fills_nothing_for_a_link_that_leaves_the_tab(cx: &mut TestAppContext) {
        let dir = folder_with("tab-link", &["alpha.md"]);
        let outside = folder_with("tab-link-outside", &["secret.md"]);
        if link_folder(&outside, &dir.join("link")).is_err() {
            // Windows without the right to make one: the guard it exercises
            // is the same code path, tested below without a link.
            assert!(
                super::read_folder(dir.clone(), outside.clone()).is_none(),
                "a folder outside the tab lists nothing"
            );
            std::fs::remove_dir_all(&dir).unwrap();
            std::fs::remove_dir_all(&outside).unwrap();
            return;
        }
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "@link/secret", 12);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(
            composer_text(&view, cx),
            "@link/secret",
            "nothing outside the tab is filled"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }

    /// Tab fills the row the arrows moved the highlight to: the second match,
    /// not the folder prefix and not the top row.
    #[gpui::test]
    fn tab_fills_the_row_the_arrows_chose(cx: &mut TestAppContext) {
        let dir = folder("tab-chosen");
        let (view, cx) = tab(cx, dir.clone());

        // `alpha.md` ranks first, `src/main.rs` second: ↓ chooses the second.
        type_at(&view, cx, "@a", 2);
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "@src/main.rs ");
        assert!(!drawn(cx, "path-menu"), "the menu closes on the choice");
        assert_eq!(
            view.read_with(cx, |view, _| (
                view.files.len(),
                view.attachments.len(),
                view.live.pending.is_none()
            )),
            (0, 0, true),
            "no tile, and nothing was sent"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The arrows move the menu's highlight (not ↑/↓ history) and Enter picks
    /// the highlighted file.
    #[gpui::test]
    fn arrows_move_the_menu_and_enter_picks_the_highlight(cx: &mut TestAppContext) {
        let dir = folder("arrows");
        let (view, cx) = tab(cx, dir.clone());
        view.update(cx, |view, _| view.prompt_history.record("first"));

        // Three files match `a`, `alpha.md` first (`src/main.rs` second): ↓
        // moves the highlight onto the second, and Enter takes that one.
        type_at(&view, cx, "@a", 2);
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "@a", "↓ did not recall history");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "@src/main.rs ");

        // Closed, ↑ is ↑ history again.
        cx.simulate_keystrokes("up");
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "first");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A long paste's chip keeps its token while a reference beside it is
    /// chosen.
    #[gpui::test]
    fn a_pastes_token_survives_the_insertion(cx: &mut TestAppContext) {
        let dir = folder("paste");
        let (view, cx) = tab(cx, dir.clone());
        let token = attachments::token(1);
        let text = format!("see @alp then {token}");
        view.update(cx, |view, _| {
            view.pasted
                .push(attachments::PastedText::new("a\nb".into()));
        });

        type_at(&view, cx, &text, 8);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            composer_text(&view, cx),
            format!("see @alpha.md then {token}")
        );
        assert_eq!(
            view.read_with(cx, |view, _| view.pasted.len()),
            1,
            "the paste is still there, and still its own card"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A completion read for text that has been typed over since is dropped:
    /// the composer is never rewritten from a stale answer.
    #[gpui::test]
    fn a_completion_for_text_that_changed_is_dropped(cx: &mut TestAppContext) {
        let dir = folder("stale");
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "@src", 4);
        let names = vec![("main.rs".to_string(), false)];
        let asked = super::Asked {
            text: "@nope".into(),
            caret: 3,
            reference: Mention {
                at: 0,
                query: "sr".into(),
            },
            folder: view.read_with(cx, |view, _| view.cwd.clone()),
            epoch: view.read_with(cx, |view, _| view.path_menu.completions),
        };
        view.update_in(cx, |view, window, cx| {
            view.complete_landed(&asked, Some(names), window, cx)
        });
        cx.run_until_parked();
        assert_eq!(composer_text(&view, cx), "@src", "the answer was stale");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A path with spaces goes in whole, and the caret lands past it: the
    /// reference is whitespace-free (prose never opens one), so a spaced
    /// path is written by choosing it, not by typing it.
    #[test]
    fn a_path_with_spaces_goes_in_whole() {
        assert_eq!(
            super::insertion("my dir/file.txt", false),
            "@my dir/file.txt",
            "the path goes in as it is on disk"
        );
        assert_eq!(super::insertion("src/main.rs", true), "@src/main.rs ");
        // The space ends a reference, so ordinary prose never opens one.
        assert_eq!(mention_at("see @my dir/file", 16), None);
    }

    /// The menu's rows come from the tab's own folder, ranked as the
    /// switcher ranks files.
    #[gpui::test]
    fn the_menu_lists_files_of_the_tab_folder(cx: &mut TestAppContext) {
        let dir = folder("rows");
        let (view, cx) = tab(cx, dir.clone());

        type_at(&view, cx, "@main", 5);
        assert!(drawn(cx, "path-menu"));
        let shown = view.read_with(cx, |view, _| {
            view.path_rows("main")
                .into_iter()
                .map(|row| row.path)
                .collect::<Vec<_>>()
        });
        assert_eq!(shown, vec!["src/main.rs".to_string()]);
        // Outside the tab's folder nothing is offered.
        assert!(view.read_with(cx, |view, _| view.path_rows("zzz").is_empty()));
        assert!(view.read_with(cx, |view, _| view.path_rows("").is_empty()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
