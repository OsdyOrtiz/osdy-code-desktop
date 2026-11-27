//! The command rows: the app's screens and pane commands, and the rows of
//! the backend's slash commands.

use gpui_kit::assets::IconName;

use gentle_core::commands::{self, SlashCommand};

use crate::shared::ui;

use super::{BrowserCard, CardCommand, PaletteAction, Row, TabLayout};

pub(super) fn screen_rows(tab_layout: TabLayout) -> Vec<Row> {
    let row = |icon, title: &str, detail: &str, action| Row {
        icon,
        title: title.into(),
        meta: String::new(),
        detail: detail.into(),
        group: "Osdy Code Desktop",
        action,
        workspace: None,
        current: false,
    };
    vec![
        row(
            IconName::Users,
            "Agents",
            &format!(
                "Helpers working for this session ({})",
                ui::shortcut_shift("A")
            ),
            PaletteAction::Agents,
        ),
        row(
            IconName::Route,
            "Profiles",
            "Model and effort per agent; profiles as tabs",
            PaletteAction::Profiles,
        ),
        row(
            IconName::RotateCcwClock,
            "Sessions",
            "Resume a previous conversation",
            PaletteAction::Sessions,
        ),
        row(
            IconName::Brain,
            "Memory",
            "Browse and search Engram, the persistent memory",
            PaletteAction::Memory,
        ),
        row(
            IconName::BookOpen,
            "Architecture docs",
            "ADRs, C4, data, states and flows, written and kept current by the agent",
            PaletteAction::Docs,
        ),
        row(
            IconName::GitGraph,
            "Git",
            "Commits as a graph, branches, tags, pull, push, cherry-pick, revert and reset",
            PaletteAction::History,
        ),
        row(
            IconName::Smartphone,
            "Devices",
            "Launch an Android emulator or iOS simulator and watch it",
            PaletteAction::Devices,
        ),
        row(
            IconName::Gauge,
            "Usage & stats",
            "Subscription windows and local token/cost stats",
            PaletteAction::Usage,
        ),
        row(
            IconName::KeyRound,
            "Providers",
            "Sign in with a subscription or an API key",
            PaletteAction::Providers,
        ),
        row(
            IconName::Settings,
            "Settings",
            "Osdy Code Desktop and Gentle Shell settings",
            PaletteAction::Settings,
        ),
        row(
            IconName::CircleCheck,
            "Setup & requirements",
            "Check Node, Gentle Shell, providers and extras; install what's missing",
            PaletteAction::Setup,
        ),
        row(
            IconName::Sparkles,
            "Welcome screen",
            "The first-run screen: the intro, then the install sequence",
            PaletteAction::Welcome,
        ),
        row(
            IconName::Folder,
            "Browse folders",
            &format!(
                "Open Osdy Code Desktop in any folder, oil-style ({})",
                ui::shortcut("O")
            ),
            PaletteAction::OpenExplorer,
        ),
        row(
            IconName::Terminal,
            "Install `ocd` command",
            "Type ocd in any terminal folder to open it here as a workspace",
            PaletteAction::InstallGsd,
        ),
        row(
            IconName::Plus,
            "New session",
            &format!(
                "Start a fresh conversation in this pane ({})",
                ui::shortcut("N")
            ),
            PaletteAction::NewSession,
        ),
        row(
            IconName::Bot,
            "New agent tab",
            &format!("Another agent in the same folder ({})", ui::shortcut("T")),
            PaletteAction::NewTab,
        ),
        row(
            IconName::SquareTerminal,
            "New terminal tab",
            "A shell in the same folder",
            PaletteAction::NewTerminalTab,
        ),
        row(
            IconName::PenTool,
            "New canvas tab",
            &format!(
                "A whiteboard for ink, shapes, notes and images ({})",
                ui::platform_shortcut("⌘⇧E", "Ctrl+Alt+Shift+E")
            ),
            PaletteAction::NewCanvasTab,
        ),
        row(
            IconName::Building2,
            "Gentle Tower",
            "Every session as a floor of its project's building: tasks, talk, artifacts, cost",
            PaletteAction::Screen(gentle_core::commands::Screen::Tower),
        ),
        row(
            IconName::LayoutDashboard,
            "New pane (tiled)",
            &format!(
                "Same kind of pane, tiled beside the focused one ({})",
                ui::platform_shortcut("⌘D", "Ctrl+Shift+D")
            ),
            PaletteAction::TilePane,
        ),
        row(
            IconName::Columns2,
            "Split right",
            "Same kind of pane to the right of this one",
            PaletteAction::SplitRight,
        ),
        row(
            IconName::Rows2,
            "Split down",
            &format!(
                "Same kind of pane below this one ({})",
                ui::platform_shortcut("⌘⇧D", "Ctrl+Shift+E")
            ),
            PaletteAction::SplitDown,
        ),
        row(
            IconName::Bot,
            "New agent pane",
            "Split with an agent beside the focused pane",
            PaletteAction::NewAgentPane,
        ),
        row(
            IconName::SquareTerminal,
            "New terminal pane",
            "Split with a terminal beside the focused pane",
            PaletteAction::NewTerminalPane,
        ),
        row(
            IconName::X,
            "Close pane",
            &format!("The last pane closes its tab ({})", ui::shortcut("W")),
            PaletteAction::ClosePane,
        ),
        row(
            IconName::RotateCcwClock,
            "Reopen closed pane",
            &format!(
                "The last pane or tab closed, where it was ({})",
                ui::platform_shortcut("⌥⌘T", "Ctrl+Alt+Shift+T")
            ),
            PaletteAction::ReopenClosed,
        ),
        match tab_layout {
            TabLayout::Horizontal => row(
                IconName::PanelLeft,
                "View: Show tabs vertically",
                "Move tabs from the top bar into the left sidebar",
                PaletteAction::SetTabLayout(TabLayout::Vertical),
            ),
            TabLayout::Vertical => row(
                IconName::List,
                "View: Show tabs horizontally",
                "Move tabs from the sidebar back into the top bar",
                PaletteAction::SetTabLayout(TabLayout::Horizontal),
            ),
        },
        row(
            IconName::PanelRight,
            "Toggle Files sidebar",
            &format!(
                "Show or hide Files / Changes / Branches on the right ({})",
                ui::shortcut("B")
            ),
            PaletteAction::ToggleSidebar,
        ),
    ]
}

/// The rows of the browser card that had the keyboard: what it can do now.
/// A maximized card restores, a floating one maximizes (a docked one stays
/// in its column); any hides or closes.
pub(super) fn card_rows(card: BrowserCard) -> Vec<Row> {
    let BrowserCard {
        stage,
        title,
        maximized,
        docked,
    } = card;
    let row = |icon, name: &str, detail: String, command| Row {
        icon,
        title: name.into(),
        meta: String::new(),
        detail,
        group: "Browser card",
        action: PaletteAction::BrowserCard { stage, command },
        workspace: None,
        current: false,
    };
    let mut rows = Vec::new();
    if maximized {
        rows.push(row(
            IconName::Minimize2,
            "Restore browser card",
            format!("{title} back to its size and place (Esc)"),
            CardCommand::Restore,
        ));
    } else if !docked {
        rows.push(row(
            IconName::Maximize2,
            "Maximize browser card",
            format!("{title} fills the window (or double-click its header)"),
            CardCommand::Maximize,
        ));
    }
    rows.push(row(
        IconName::Minus,
        "Hide browser card",
        format!("{title} waits in the instance tray"),
        CardCommand::Hide,
    ));
    rows.push(row(
        IconName::X,
        "Close browser card",
        format!("{title} closes, page and all"),
        CardCommand::Close,
    ));
    rows
}

/// The row of a slash command: `None` for commands that do nothing over RPC;
/// terminal-only Gentle Shell ones open their native screen instead of
/// running, and Pi built-ins that only work in the terminal say so.
pub(super) fn command_row(command: SlashCommand) -> Option<Row> {
    let SlashCommand {
        name,
        description: detail,
        terminal,
    } = command;
    if commands::hidden(&name) {
        return None;
    }
    let (group, icon) = if name.starts_with("skill:") {
        ("Skills", IconName::Zap)
    } else if name.starts_with("gentle:") {
        ("Gentle commands", IconName::Command)
    } else {
        ("Commands", IconName::Command)
    };
    let action = match commands::route(&name) {
        Some(screen) => PaletteAction::Screen(screen),
        None => PaletteAction::Command(name.clone()),
    };
    Some(Row {
        icon,
        title: format!("/{name}"),
        meta: if terminal {
            "terminal".to_string()
        } else {
            String::new()
        },
        detail,
        group,
        action,
        workspace: None,
        current: false,
    })
}

#[cfg(test)]
mod tests {
    use gentle_core::commands::{Screen, SlashCommand};

    use super::super::{BrowserCard, CardCommand, PaletteAction, TabLayout};
    use super::{card_rows, command_row, screen_rows};

    fn command(name: &str, detail: &str, terminal: bool) -> SlashCommand {
        SlashCommand {
            name: name.into(),
            description: detail.into(),
            terminal,
        }
    }

    fn action(name: &str) -> Option<PaletteAction> {
        command_row(command(name, "detail", false)).map(|row| row.action)
    }

    #[test]
    fn tab_placement_command_matches_current_layout() {
        for (current, expected_title, target, absent) in [
            (
                TabLayout::Horizontal,
                "View: Show tabs vertically",
                TabLayout::Vertical,
                "View: Show tabs horizontally",
            ),
            (
                TabLayout::Vertical,
                "View: Show tabs horizontally",
                TabLayout::Horizontal,
                "View: Show tabs vertically",
            ),
        ] {
            let rows = screen_rows(current);
            assert!(rows.iter().any(|row| {
                row.title == expected_title && row.action == PaletteAction::SetTabLayout(target)
            }));
            assert!(!rows.iter().any(|row| row.title == absent));
        }
    }

    #[test]
    fn backend_commands_skip_dead_ones_and_open_native_screens() {
        assert_eq!(action("gentle:guide"), None);
        assert_eq!(action("history"), None);
        assert_eq!(action("gentle:panel"), None);
        assert_eq!(
            action("gentle:usage"),
            Some(PaletteAction::Screen(Screen::Usage))
        );
        assert_eq!(
            action("gentle:profiles"),
            Some(PaletteAction::Screen(Screen::Profiles))
        );
        assert_eq!(
            action("gentle:doctor"),
            Some(PaletteAction::Command("gentle:doctor".into()))
        );
        let row = command_row(command("gentle:stats", "Local stats", false)).unwrap();
        assert_eq!(
            (row.title.as_str(), row.detail.as_str(), row.meta.as_str()),
            ("/gentle:stats", "Local stats", "")
        );
    }

    #[test]
    fn browser_card_offers_what_it_can_do_now() {
        let actions = |maximized, docked| {
            card_rows(BrowserCard {
                stage: 3,
                title: "localhost:5173".into(),
                maximized,
                docked,
            })
            .into_iter()
            .map(|row| row.action)
            .collect::<Vec<_>>()
        };
        let card = |command| PaletteAction::BrowserCard { stage: 3, command };
        let (hide, close) = (card(CardCommand::Hide), card(CardCommand::Close));
        assert_eq!(
            actions(false, false),
            [card(CardCommand::Maximize), hide.clone(), close.clone()]
        );
        assert_eq!(
            actions(true, false),
            [card(CardCommand::Restore), hide.clone(), close.clone()]
        );
        assert_eq!(actions(false, true), [hide, close]);
    }

    #[test]
    fn terminal_only_pi_built_ins_are_marked() {
        let row = command_row(command("model", "Select model", true)).unwrap();
        assert_eq!(row.meta, "terminal");
        assert_eq!(row.action, PaletteAction::Command("model".into()));
    }
}
