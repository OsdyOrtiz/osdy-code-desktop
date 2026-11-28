//! Named workspaces (Tern calls them sessions): each has a root folder and its
//! own tabs. The list, with every tab's pane layout, is kept as JSON in
//! `<data dir>/osdy-code-desktop/workspaces.json` and restored on launch.
//!
//! This module holds the serialized model; `store` reads and writes the file.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::assets::IconName;
use serde::{Deserialize, Serialize};

use crate::shared::pane::{PaneKind, PaneTree};

mod store;
#[cfg(test)]
mod tests;

pub use gentle_core::workspaces::unique_name;
pub use store::{load, save};

/// One pane as saved: enough to build it again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedPane {
    pub kind: PaneKind,
    pub cwd: PathBuf,
    /// The file a file or canvas pane shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The conversation an agent pane had, resumed when it still exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub layout: PaneTree<SavedPane>,
    /// Pinned tabs stay ahead of ordinary tabs.
    #[serde(default)]
    pub pinned: bool,
    /// Index of the focused pane among the layout's leaves.
    #[serde(default)]
    pub focused: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkspace {
    pub name: String,
    pub root: PathBuf,
    #[serde(default)]
    pub tabs: Vec<SavedTab>,
    #[serde(default)]
    pub active_tab: usize,
    /// Unix seconds.
    #[serde(default)]
    pub last_active: u64,
    /// The workspace's badge as the person chose it.
    #[serde(default, skip_serializing_if = "Badge::is_default")]
    pub badge: Badge,
    /// Older layouts show all workspaces expanded by default.
    #[serde(default = "shown")]
    pub sidebar_expanded: bool,
}

/// How a workspace's badge looks: an icon in place of its initials, and a
/// theme color in place of the one its name picks. Either can be left unset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Badge {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "known_or_none"
    )]
    pub icon: Option<BadgeIcon>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "known_or_none"
    )]
    pub color: Option<BadgeColor>,
}

impl Badge {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The icons a workspace badge can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeIcon {
    Folder,
    Code,
    Terminal,
    Globe,
    Bot,
    Brain,
    Rocket,
    Zap,
    Flame,
    Leaf,
    Database,
    Server,
    Cloud,
    Smartphone,
    Palette,
    Pen,
    Book,
    Music,
    Package,
    Shield,
    Sparkles,
    Star,
    Heart,
    Gem,
    Crown,
    Coffee,
    Gamepad,
    Flask,
    Atom,
    Bug,
    Puzzle,
    Cat,
}

impl BadgeIcon {
    pub const ALL: [Self; 32] = [
        Self::Folder,
        Self::Code,
        Self::Terminal,
        Self::Globe,
        Self::Bot,
        Self::Brain,
        Self::Rocket,
        Self::Zap,
        Self::Flame,
        Self::Leaf,
        Self::Database,
        Self::Server,
        Self::Cloud,
        Self::Smartphone,
        Self::Palette,
        Self::Pen,
        Self::Book,
        Self::Music,
        Self::Package,
        Self::Shield,
        Self::Sparkles,
        Self::Star,
        Self::Heart,
        Self::Gem,
        Self::Crown,
        Self::Coffee,
        Self::Gamepad,
        Self::Flask,
        Self::Atom,
        Self::Bug,
        Self::Puzzle,
        Self::Cat,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Code => "Code",
            Self::Terminal => "Terminal",
            Self::Globe => "Globe",
            Self::Bot => "Bot",
            Self::Brain => "Brain",
            Self::Rocket => "Rocket",
            Self::Zap => "Zap",
            Self::Flame => "Flame",
            Self::Leaf => "Leaf",
            Self::Database => "Database",
            Self::Server => "Server",
            Self::Cloud => "Cloud",
            Self::Smartphone => "Smartphone",
            Self::Palette => "Palette",
            Self::Pen => "Pen",
            Self::Book => "Book",
            Self::Music => "Music",
            Self::Package => "Package",
            Self::Shield => "Shield",
            Self::Sparkles => "Sparkles",
            Self::Star => "Star",
            Self::Heart => "Heart",
            Self::Gem => "Gem",
            Self::Crown => "Crown",
            Self::Coffee => "Coffee",
            Self::Gamepad => "Gamepad",
            Self::Flask => "Flask",
            Self::Atom => "Atom",
            Self::Bug => "Bug",
            Self::Puzzle => "Puzzle",
            Self::Cat => "Cat",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Folder => IconName::Folder,
            Self::Code => IconName::CodeXml,
            Self::Terminal => IconName::Terminal,
            Self::Globe => IconName::Globe,
            Self::Bot => IconName::Bot,
            Self::Brain => IconName::Brain,
            Self::Rocket => IconName::Rocket,
            Self::Zap => IconName::Zap,
            Self::Flame => IconName::Flame,
            Self::Leaf => IconName::Leaf,
            Self::Database => IconName::Database,
            Self::Server => IconName::Server,
            Self::Cloud => IconName::Cloud,
            Self::Smartphone => IconName::Smartphone,
            Self::Palette => IconName::Palette,
            Self::Pen => IconName::PenTool,
            Self::Book => IconName::BookOpen,
            Self::Music => IconName::Music,
            Self::Package => IconName::Package,
            Self::Shield => IconName::Shield,
            Self::Sparkles => IconName::Sparkles,
            Self::Star => IconName::Star,
            Self::Heart => IconName::Heart,
            Self::Gem => IconName::Gem,
            Self::Crown => IconName::Crown,
            Self::Coffee => IconName::Coffee,
            Self::Gamepad => IconName::Gamepad2,
            Self::Flask => IconName::FlaskConical,
            Self::Atom => IconName::Atom,
            Self::Bug => IconName::Bug,
            Self::Puzzle => IconName::Puzzle,
            Self::Cat => IconName::Cat,
        }
    }
}

/// The theme colors a badge can take; each follows the active theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeColor {
    Accent,
    Info,
    Teal,
    Success,
    Champagne,
    Warning,
    Danger,
    Purple,
}

impl BadgeColor {
    pub const ALL: [Self; 8] = [
        Self::Accent,
        Self::Info,
        Self::Teal,
        Self::Success,
        Self::Champagne,
        Self::Warning,
        Self::Danger,
        Self::Purple,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Accent => "Accent",
            Self::Info => "Blue",
            Self::Teal => "Teal",
            Self::Success => "Green",
            Self::Champagne => "Gold",
            Self::Warning => "Amber",
            Self::Danger => "Red",
            Self::Purple => "Purple",
        }
    }

    /// The color a workspace's name picks while none is chosen: stable for
    /// a name, and still one of the theme's.
    pub fn for_name(name: &str) -> Self {
        let hash = name
            .bytes()
            .fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
        Self::ALL[hash as usize % Self::ALL.len()]
    }
}

/// A value saved by a newer build that this one doesn't know reads as unset.
fn known_or_none<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    Ok(Option::<serde_json::Value>::deserialize(d)?.and_then(|v| T::deserialize(v).ok()))
}

/// Which presentation the workspace uses for its tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabLayout {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SavedStateOnDisk")]
pub struct SavedState {
    #[serde(default)]
    pub version: u32,
    /// Index of the workspace the window showed last.
    #[serde(default)]
    pub current: usize,
    #[serde(default)]
    pub workspaces: Vec<SavedWorkspace>,
    /// The files sidebar (⌘B) was showing.
    #[serde(default = "shown")]
    pub sidebar: bool,
    /// Older workspaces.json files keep the familiar horizontal layout.
    #[serde(default)]
    pub tab_layout: TabLayout,
    /// Expanded navigator width (px at 100 % interface size).
    #[serde(rename = "sidebar_width")]
    pub tabs_sidebar_width: f32,
    /// Width of the right Files sidebar (px at 100 % interface size).
    #[serde(default = "default_sidebar_width")]
    pub files_sidebar_width: f32,
    /// Whether the vertical navigator uses its narrow icon rail.
    #[serde(default)]
    pub sidebar_compact: bool,
    /// Whether the vertical navigator is hidden. Independent of the rail:
    /// compacting stops at the rail, hiding removes the panel.
    #[serde(default)]
    pub tabs_sidebar_hidden: bool,
}

// Read both spellings independently so a mixed-version file is not rejected
// as a duplicate field. Write the original key for older binaries.
#[derive(Deserialize)]
struct SavedStateOnDisk {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    current: usize,
    #[serde(default)]
    workspaces: Vec<SavedWorkspace>,
    #[serde(default = "shown")]
    sidebar: bool,
    #[serde(default)]
    tab_layout: TabLayout,
    sidebar_width: Option<f32>,
    tabs_sidebar_width: Option<f32>,
    #[serde(default = "default_sidebar_width")]
    files_sidebar_width: f32,
    #[serde(default)]
    sidebar_compact: bool,
    #[serde(default)]
    tabs_sidebar_hidden: bool,
}

impl From<SavedStateOnDisk> for SavedState {
    fn from(state: SavedStateOnDisk) -> Self {
        Self {
            version: state.version,
            current: state.current,
            workspaces: state.workspaces,
            sidebar: state.sidebar,
            tab_layout: state.tab_layout,
            tabs_sidebar_width: state
                .tabs_sidebar_width
                .or(state.sidebar_width)
                .unwrap_or_else(default_sidebar_width),
            files_sidebar_width: state.files_sidebar_width,
            sidebar_compact: state.sidebar_compact,
            tabs_sidebar_hidden: state.tabs_sidebar_hidden,
        }
    }
}

fn shown() -> bool {
    true
}

fn default_sidebar_width() -> f32 {
    gentle_core::workspaces::SIDEBAR_DEFAULT_WIDTH
}

/// A first launch shows the sidebar, as a file from before it was saved does,
/// and the tabs in the vertical navigator (older files keep horizontal).
impl Default for SavedState {
    fn default() -> Self {
        Self {
            version: 0,
            current: 0,
            workspaces: Vec::new(),
            sidebar: true,
            tab_layout: TabLayout::Vertical,
            tabs_sidebar_width: default_sidebar_width(),
            files_sidebar_width: default_sidebar_width(),
            sidebar_compact: false,
            tabs_sidebar_hidden: false,
        }
    }
}

pub const VERSION: u32 = 1;

/// How a workspace root is compared: canonical, so a folder reached through
/// a symlink, `..` or another drive-letter case finds the same workspace as
/// `ocd` (which passes canonical paths). Unresolvable paths stay as given.
pub fn root_key(path: PathBuf) -> PathBuf {
    dunce::canonicalize(&path).unwrap_or(path)
}

/// The position of the root that is `folder` (already a [`root_key`]),
/// comparing saved roots canonically too: older saves and other entry points
/// may hold a root reached another way.
pub fn find_root<'a>(
    roots: impl Iterator<Item = &'a PathBuf>,
    folder: &std::path::Path,
) -> Option<usize> {
    roots
        .map(|root| root_key(root.clone()))
        .position(|root| root == folder)
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
