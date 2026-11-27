//! The project file browser: [`FileTreeView`] (sidebar with the lazily
//! loaded tree, the working-tree changes and the branches) and [`FileView`]
//! (editable, syntax-highlighted file — ⌘S saves — or read-only diff pane).

mod changes;
mod listing;
mod mermaid;
mod ops;
mod refs;
mod tree;
mod viewer;

pub use changes::{ChangesEvent, ChangesView};
pub(crate) use listing::list_dir;
pub use mermaid::{
    Diagrams as MermaidDiagrams, Job as MermaidJob, Renderer as DiagramRenderer,
    RendererState as DiagramRendererState, banner as mermaid_banner, dark as mermaid_dark,
    draw as draw_mermaid, install as install_diagram_renderer, ready_cli as diagram_renderer_cli,
    renderer as diagram_renderer,
};
pub use tree::{FileTreeEvent, FileTreeView, REVEAL};
pub use viewer::{EmbeddedImages, FileView, offline_image};

use std::path::{Path, PathBuf};

use gpui_kit::App;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, h_flex};
use gpui_kit::{
    Context, Hsla, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window,
};

use crate::shared::{i18n::t, ui};

/// Retained interface copy: translate the template at render, never the data it contains.
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    Template(&'static str, Vec<(&'static str, Text)>),
    Raw(String),
}

impl Text {
    pub fn with(self, marker: &'static str, value: impl Into<String>) -> Self {
        self.with_text(marker, Self::Raw(value.into()))
    }

    pub fn with_text(mut self, marker: &'static str, value: Self) -> Self {
        if let Self::Template(_, args) = &mut self {
            args.push((marker, value));
        }
        self
    }

    pub fn render(&self) -> String {
        match self {
            Self::Raw(text) => text.clone(),
            Self::Template(key, args) => {
                let mut rest = t(key);
                let mut text = String::new();
                while let Some((at, marker, value)) = args
                    .iter()
                    .filter_map(|(marker, value)| rest.find(marker).map(|at| (at, marker, value)))
                    .min_by_key(|(at, _, _)| *at)
                {
                    text.push_str(&rest[..at]);
                    text.push_str(&value.render());
                    rest = &rest[at + marker.len()..];
                }
                text.push_str(rest);
                text
            }
        }
    }
}

impl From<&'static str> for Text {
    fn from(key: &'static str) -> Self {
        Self::Template(key, Vec::new())
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Self::Raw(text)
    }
}

/// A row dragged out of the file tree, the payload the agent tab takes
/// (S12): dropped on one it attaches like a path dropped from Finder, so a
/// folder is refused there the same way. The tree keeps its own gestures (a
/// click opens or toggles, the context menu renames).
#[derive(Clone)]
pub struct FileDrag {
    pub path: PathBuf,
    /// The row's name, for the ghost under the pointer.
    pub name: SharedString,
    pub is_dir: bool,
}

impl FileDrag {
    pub fn new(path: PathBuf, name: impl Into<SharedString>, is_dir: bool) -> Self {
        Self {
            path,
            name: name.into(),
            is_dir,
        }
    }
}

/// What follows the pointer: the row's glyph and name, the chip a tab or a
/// workspace wears while dragged.
impl Render for FileDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1p5()
            .px_2()
            .py_1()
            .rounded_lg()
            .bg(ui::raised())
            .border_1()
            .border_color(ui::petal().opacity(0.5))
            .shadow_lg()
            .text_xs()
            .text_color(ui::fg())
            .child(
                Icon::new(if self.is_dir {
                    IconName::Folder
                } else {
                    IconName::File
                })
                .small()
                .text_color(ui::petal()),
            )
            .child(self.name.clone())
    }
}

/// Key bindings and the diagram renderer's state.
pub fn init(cx: &mut App) {
    tree::init(cx);
    mermaid::init(cx);
}

/// The icon and tint for a file name in the tree, and in the composer's `@`
/// menu.
pub(crate) fn file_icon(name: &str) -> (IconName, Hsla) {
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match (lower.as_str(), ext) {
        ("cargo.toml" | "package.json" | "pyproject.toml" | "go.mod", _) => {
            (IconName::Package, ui::champagne())
        }
        (n, _) if n.starts_with("readme") => (IconName::Info, ui::info()),
        (n, _) if n.starts_with(".git") => (IconName::GitBranch, ui::muted()),
        (n, _) if n.ends_with(".lock") || n.ends_with("-lock.json") || n == "pnpm-lock.yaml" => {
            (IconName::Lock, ui::faint())
        }
        (_, "rs") => (IconName::FileCode, ui::warning()),
        (
            _,
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "go" | "c" | "h" | "cc" | "cpp"
            | "hpp" | "java" | "kt" | "swift" | "rb" | "lua" | "zig" | "cs" | "php" | "scala"
            | "ex" | "exs" | "sh" | "bash" | "zsh" | "fish" | "sql" | "html" | "css" | "scss"
            | "vue" | "svelte" | "astro",
        ) => (IconName::FileCode, ui::info()),
        (_, "json" | "jsonc" | "json5") => (IconName::Braces, ui::champagne()),
        (_, "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "env") => {
            (IconName::Settings2, ui::muted())
        }
        (_, "md" | "mdx" | "txt" | "rst") => (IconName::FileText, ui::muted()),
        (_, "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp") => {
            (IconName::Image, ui::petal())
        }
        _ => (IconName::File, ui::muted()),
    }
}

/// `(highlighter token, label)` for a path. The token goes to the editor's
/// syntect backend, which resolves names and extensions; `None` is plain text.
fn language(path: &Path) -> (Option<&'static str>, &'static str) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let pick = match (name.as_str(), ext) {
        ("makefile" | "gnumakefile", _) => ("make", "Makefile"),
        ("dockerfile", _) => ("dockerfile", "Dockerfile"),
        ("cargo.lock", _) => ("toml", "TOML"),
        (_, "rs") => ("rust", "Rust"),
        (_, "toml") => ("toml", "TOML"),
        (_, "md" | "mdx") => ("markdown", "Markdown"),
        (_, "json" | "jsonc") => ("json", "JSON"),
        (_, "ts" | "mts" | "cts") => ("typescript", "TypeScript"),
        (_, "tsx") => ("tsx", "TSX"),
        (_, "js" | "mjs" | "cjs" | "jsx") => ("javascript", "JavaScript"),
        (_, "py" | "pyi") => ("python", "Python"),
        (_, "go") => ("go", "Go"),
        (_, "sh" | "bash" | "zsh") => ("bash", "Shell"),
        (_, "yaml" | "yml") => ("yaml", "YAML"),
        (_, "html" | "htm") => ("html", "HTML"),
        (_, "css" | "scss") => ("css", "CSS"),
        (_, "c" | "h") => ("c", "C"),
        (_, "cc" | "cpp" | "cxx" | "hpp" | "hxx") => ("cpp", "C++"),
        (_, "java") => ("java", "Java"),
        (_, "kt" | "kts") => ("kotlin", "Kotlin"),
        (_, "swift") => ("swift", "Swift"),
        (_, "rb") => ("ruby", "Ruby"),
        (_, "lua") => ("lua", "Lua"),
        (_, "zig") => ("zig", "Zig"),
        (_, "sql") => ("sql", "SQL"),
        (_, "php") => ("php", "PHP"),
        (_, "cs") => ("csharp", "C#"),
        (_, "diff" | "patch") => ("diff", "Diff"),
        (_, "xml" | "svg") => ("xml", "XML"),
        _ => return (None, "Plain text"),
    };
    (Some(pick.0), pick.1)
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / KB / KB)
    }
}

/// Color for a collapsed porcelain status letter (see [`gentle_core::git::status::FileChange`]).
fn status_color(status: char) -> Hsla {
    match status {
        'A' | '?' => ui::success(),
        'D' => ui::danger(),
        'R' => ui::info(),
        _ => ui::warning(),
    }
}

#[cfg(test)]
mod tests {
    use super::{human_size, language};
    use std::path::Path;

    #[test]
    fn languages_and_sizes() {
        assert_eq!(language(Path::new("src/main.rs")), (Some("rust"), "Rust"));
        assert_eq!(language(Path::new("Cargo.lock")), (Some("toml"), "TOML"));
        assert_eq!(language(Path::new("LICENSE")), (None, "Plain text"));
        assert_eq!(human_size(12), "12 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    }
}
