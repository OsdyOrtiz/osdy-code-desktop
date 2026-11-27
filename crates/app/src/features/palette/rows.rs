//! Matching: the rows and group headers shown for the current query and
//! category, from each source.

use gpui_kit::App;
use gpui_kit::assets::IconName;

use crate::features::session::SessionView;
use crate::shared::ui;
use gentle_core::fuzzy::{Needle, fuzzy_score, rank_files};

use super::{Category, Line, PaletteAction, PaletteView, Row, WorkspaceRow, t, tab_count};

impl PaletteView {
    /// Re-match the rows against the current query and category.
    pub(super) fn refresh_rows(&mut self, cx: &App) {
        let query = self.query.read(cx).value();
        if query.trim() != self.needle.text {
            self.needle = Needle::new(&query);
        }
        let typed = !self.needle.text.is_empty();
        let all = self.category == Category::All;
        let mut rows = Vec::new();
        if matches!(self.category, Category::All | Category::Workspaces) {
            rows.extend(self.workspace_rows(all));
        }
        if matches!(self.category, Category::All | Category::Commands) {
            rows.extend(self.command_rows(if all { 12 } else { 80 }));
        }
        if self.category == Category::Files || (all && typed) {
            rows.extend(self.file_rows(if all { 8 } else { 60 }));
        }
        if self.category == Category::Branches || (all && typed) {
            rows.extend(self.branch_rows(if all { 5 } else { 60 }));
        }
        if !rows.is_empty() && self.selected >= rows.len() {
            self.selected = rows.len() - 1;
        }
        self.lines = Self::lines(&rows);
        self.rows = rows;
    }

    fn workspace_rows(&self, all: bool) -> Vec<Row> {
        let query = self.needle.text.as_str();
        let mut scored: Vec<(usize, &WorkspaceRow)> = self
            .workspaces
            .iter()
            .filter_map(|w| Some((fuzzy_score(&self.needle.lower, &w.name)?, w)))
            .collect();
        scored.sort_by_key(|(score, w)| (*score, w.index));
        let mut rows: Vec<Row> = scored
            .into_iter()
            .take(if all { 6 } else { usize::MAX })
            .map(|(_, w)| Row {
                icon: if w.current {
                    IconName::Check
                } else {
                    IconName::Layers
                },
                title: w.name.clone(),
                meta: format!(
                    "{}{}",
                    tab_count(w.tabs),
                    if w.current { t(" · current") } else { "" }
                ),
                detail: String::new(),
                group: "Workspaces",
                action: PaletteAction::SwitchWorkspace(w.index),
                workspace: Some(w.index),
                current: w.current,
            })
            .collect();
        if all {
            return rows;
        }
        let exists = self
            .workspaces
            .iter()
            .any(|w| w.name.eq_ignore_ascii_case(query));
        if !query.is_empty() && !exists {
            rows.insert(
                0,
                Row {
                    icon: IconName::Plus,
                    title: t("Create workspace “{name}”").replace("{name}", query),
                    meta: String::new(),
                    detail: t("in {folder}").replace("{folder}", &ui::split_path(&self.root).1),
                    group: "Workspaces",
                    action: PaletteAction::CreateWorkspace {
                        name: query.to_string(),
                        pick_folder: false,
                    },
                    workspace: None,
                    current: false,
                },
            );
        }
        rows.push(Row {
            icon: IconName::Folder,
            title: t("Browse folders…").into(),
            meta: String::new(),
            detail: t("Open Osdy Code Desktop in any folder ({shortcut})")
                .replace("{shortcut}", &ui::shortcut("O")),
            group: "Workspaces",
            action: PaletteAction::OpenExplorer,
            workspace: None,
            current: false,
        });
        rows.push(Row {
            icon: IconName::FolderOpen,
            title: t("Create workspace from folder…").into(),
            meta: String::new(),
            detail: if query.is_empty() {
                t("Named after the folder").into()
            } else {
                t("Named “{name}”").replace("{name}", query)
            },
            group: "Workspaces",
            action: PaletteAction::CreateWorkspace {
                name: if exists {
                    String::new()
                } else {
                    query.to_string()
                },
                pick_folder: true,
            },
            workspace: None,
            current: false,
        });
        rows
    }

    fn command_rows(&self, cap: usize) -> Vec<Row> {
        let query = self.needle.command.as_str();
        let mut scored: Vec<(usize, usize, &Row)> = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                let localized = self.localized_commands.get(i).unwrap_or(row);
                let score = |row: &Row| {
                    fuzzy_score(query, &row.title)
                        .or_else(|| fuzzy_score(query, &row.detail).map(|s| s + 500))
                };
                let score = score(row).or_else(|| score(localized)).or_else(|| {
                    let name = row.title.strip_prefix('/')?;
                    let english = SessionView::builtin_english_description(name, &row.detail)?;
                    fuzzy_score(query, english).map(|s| s + 500)
                })?;
                Some((score, i, localized))
            })
            .collect();
        // Keep groups together when nothing is typed.
        if !query.is_empty() {
            scored.sort_by_key(|(score, i, _)| (*score, *i));
        }
        scored
            .into_iter()
            .take(cap)
            .map(|(_, _, r)| r.clone())
            .collect()
    }

    fn file_rows(&self, cap: usize) -> Vec<Row> {
        let Some(files) = &self.files else {
            return Vec::new();
        };
        rank_files(files, &self.needle.lower, cap)
            .into_iter()
            .map(|i| {
                let file = &files[i];
                let path = &file.path;
                let (dir, name) = match file.name_at {
                    0 => (String::new(), path.clone()),
                    at => (path[..at - 1].to_string(), path[at..].to_string()),
                };
                Row {
                    icon: IconName::File,
                    title: name,
                    meta: String::new(),
                    detail: dir,
                    group: "Files",
                    action: PaletteAction::OpenFile(self.root.join(path)),
                    workspace: None,
                    current: false,
                }
            })
            .collect()
    }

    fn branch_rows(&self, cap: usize) -> Vec<Row> {
        let Some(branches) = &self.branches else {
            return Vec::new();
        };
        let mut scored: Vec<(usize, &(String, bool))> = branches
            .iter()
            .filter_map(|b| Some((fuzzy_score(&self.needle.lower, &b.0)?, b)))
            .collect();
        scored.sort_by_key(|(score, (_, current))| (!*current, *score));
        scored
            .into_iter()
            .take(cap)
            .map(|(_, (name, current))| Row {
                icon: if *current {
                    IconName::Check
                } else {
                    IconName::GitBranch
                },
                title: name.clone(),
                meta: if *current {
                    t("current").into()
                } else {
                    String::new()
                },
                detail: String::new(),
                group: "Branches",
                action: PaletteAction::SwitchBranch(name.clone()),
                workspace: None,
                current: *current,
            })
            .collect()
    }

    /// Group headers and rows in display order; the scroll container's children.
    fn lines(rows: &[Row]) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut group = "";
        for (i, row) in rows.iter().enumerate() {
            if row.group != group {
                group = row.group;
                lines.push(Line::Header(group));
            }
            lines.push(Line::Row(i));
        }
        lines
    }
}
