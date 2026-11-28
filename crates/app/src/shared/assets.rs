//! Brand images and additional icons, embedded on top of GPUI Kit's `Assets`.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

gpui_kit::assets::icon_assets!(
    ExtraIcons,
    [
        Activity,
        CodeXml,
        FilePen,
        Save,
        Wrench,
        Shield,
        Sparkles,
        Flame,
        MessageSquare,
        Braces,
        FileCode,
        FileDiff,
        Files,
        Image,
        Lock,
        Package,
        Funnel,
        OctagonX,
        Columns2,
        Rows2,
        GripHorizontal,
        Pencil,
        Bot,
        Brain,
        Command,
        Gauge,
        GitBranch,
        KeyRound,
        Layers,
        ListTodo,
        LogIn,
        LogOut,
        RefreshCw,
        RotateCcwClock,
        Settings,
        ShieldCheck,
        SquareTerminal,
        Trash,
        Unlink,
        Upload,
        Users,
        X,
        Zap,
        Paperclip,
        // Configuration → Water reminder.
        Droplets,
        // Architecture docs and the diagram renderer.
        BookOpen,
        FileText,
        NotebookPen,
        Workflow,
        Download,
        // File tree header and context menu.
        FilePlus,
        FolderPlus,
        FolderInput,
        ArrowUpRight,
        Terminal,
        CopyPlus,
        Scissors,
        ClipboardPaste,
        ChevronsDownUp,
        Diff,
        // Changes and Branches tabs.
        List,
        ListTree,
        Archive,
        Tag,
        Cloud,
        FolderGit,
        GitCommitHorizontal,
        // The Git pane: graph, pull, push, new branch, jump to HEAD, cherry-pick.
        GitGraph,
        ArrowDownToLine,
        ArrowUpFromLine,
        GitBranchPlus,
        LocateFixed,
        Cherry,
        // Profiles: routing models to agents.
        Route,
        // Effort levels (off, minimal, low, high; Brain, Zap and Flame above).
        Moon,
        Feather,
        Leaf,
        BrainCircuit,
        // In-app browser panel.
        Maximize2,
        Minimize2,
        // Device cards and the instance tray.
        Smartphone,
        MonitorSmartphone,
        Power,
        // The music player.
        Music,
        SkipForward,
        Volume2,
        Square,
        // The device panel.
        Apple,
        PanelRight,
        PictureInPicture2,
        // Configuration → Theme: share a theme, import one from a link.
        Share2,
        Link,
        // Generative UI diagrams: node kinds and the toolbar.
        Server,
        Database,
        Monitor,
        SquareFunction,
        StickyNote,
        ZoomIn,
        ZoomOut,
        Scan,
        Send,
        // Generative UI diagrams: groups, box selection and the lock.
        Group,
        Ungroup,
        SquareDashed,
        LockOpen,
        // Generative UI panels: tables, forms, checklists, kanban, charts.
        Table,
        ClipboardList,
        ListChecks,
        SquareKanban,
        ChartColumn,
        GripVertical,
        Circle,
        ListIndentIncrease,
        ListIndentDecrease,
        ArrowUpDown,
        // Generative UI canvas kinds: sequence, state machine, timeline.
        ArrowLeftRight,
        CircleDot,
        ChartGantt,
        Flag,
        // The session's spend beside the send.
        DollarSign,
        // A diff pane's wrap toggle.
        TextWrap,
        // Pinned tabs and pinned generative UI surfaces.
        Pin,
        PinOff,
        // Canvas tabs: tools and the style panel.
        MousePointer2,
        Lasso,
        Hand,
        Pen,
        Highlighter,
        Eraser,
        WandSparkles,
        MoveUpRight,
        Slash,
        Type,
        PenTool,
        Signature,
        Spline,
        Hash,
        // Workspace badges (`workspaces::BadgeIcon`).
        Rocket,
        Gem,
        Crown,
        Coffee,
        Gamepad2,
        FlaskConical,
        Atom,
        Bug,
        Puzzle,
        Cat,
        Camera,
        Film,
        Headphones,
        Compass,
        Mountain,
        Snowflake,
        TreePine,
        Anchor,
        Plane,
        Car,
        House,
        Briefcase,
        Trophy,
        Target,
        Key,
        Ghost,
        Dog,
        Fish,
        Bird
    ]
);

// The icons a diagram element may name (`gentle_core::genui::style::ICONS`,
// kept in step: `genui::look::icon` finds them by path).
gpui_kit::assets::icon_assets!(
    DiagramIcons,
    [
        Activity,
        Archive,
        BookOpen,
        Bot,
        Box,
        Boxes,
        Brain,
        Bug,
        Building,
        Cable,
        Calendar,
        ChartColumn,
        CircleCheck,
        CircleX,
        Clock,
        Cloud,
        CloudCog,
        CloudDownload,
        CloudUpload,
        Code,
        Container,
        Cookie,
        Cpu,
        CreditCard,
        Database,
        DatabaseZap,
        Download,
        Eye,
        File,
        FileText,
        FingerprintPattern,
        Flag,
        Folder,
        Gauge,
        GitBranch,
        GitMerge,
        GitPullRequest,
        Globe,
        GraduationCap,
        Hammer,
        HardDrive,
        Heart,
        Hourglass,
        Image,
        Inbox,
        KeyRound,
        Laptop,
        Layers,
        Lightbulb,
        ListTodo,
        Lock,
        LogIn,
        LogOut,
        Mail,
        MapPin,
        MemoryStick,
        Merge,
        MessageSquare,
        Monitor,
        Network,
        Package,
        Pause,
        Play,
        Plug,
        Puzzle,
        Radio,
        Receipt,
        RefreshCw,
        Repeat,
        Rocket,
        Router,
        Satellite,
        Scale,
        ScanFace,
        Search,
        Send,
        Server,
        ServerCog,
        Settings,
        Shield,
        ShieldCheck,
        ShoppingCart,
        Smartphone,
        Sparkles,
        Split,
        SquareFunction,
        Star,
        Store,
        Table,
        Terminal,
        Timer,
        Trash,
        TriangleAlert,
        Truck,
        Unplug,
        Upload,
        User,
        Users,
        Wallet,
        Warehouse,
        Webhook,
        Wifi,
        Workflow,
        Wrench,
        Zap
    ]
);

pub struct AppAssets;

const BRAND: &[(&str, &[u8])] = &[
    (
        "brand/badge.png",
        include_bytes!("../../assets/brand/badge.png"),
    ),
    (
        "brand/eyes-closed.svg",
        include_bytes!("../../assets/brand/eyes-closed.svg"),
    ),
    (
        "brand/eyes-wink.svg",
        include_bytes!("../../assets/brand/eyes-wink.svg"),
    ),
    (
        "brand/wordmark-text.svg",
        include_bytes!("../../assets/brand/wordmark-text.svg"),
    ),
    (
        "brand/cursor.svg",
        include_bytes!("../../assets/brand/cursor.svg"),
    ),
];

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = BRAND.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        if let Some(bytes) = DiagramIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.extend(DiagramIcons.list(path)?);
        paths.extend(
            BRAND
                .iter()
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::from(*name)),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use image::ImageFormat;
    use resvg::usvg::{Options, Tree};

    use super::*;

    #[test]
    fn embedded_banner_assets_decode_and_align() {
        let load = |path| {
            AppAssets
                .load(path)
                .unwrap()
                .unwrap_or_else(|| panic!("missing {path}"))
        };
        let badge = image::load_from_memory_with_format(&load("brand/badge.png"), ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        assert_eq!(badge.dimensions(), (768, 768));
        assert_eq!(badge.get_pixel(0, 0)[3], 0);
        assert_eq!(badge.get_pixel(384, 384)[3], 255);

        let tree = |path| Tree::from_data(&load(path), &Options::default()).unwrap();
        for path in ["brand/eyes-closed.svg", "brand/eyes-wink.svg"] {
            let eyes = tree(path);
            assert_eq!((eyes.size().width(), eyes.size().height()), (384., 384.));
        }
        let text = tree("brand/wordmark-text.svg");
        let cursor = tree("brand/cursor.svg");
        assert_eq!(text.size(), cursor.size());
        let text_ink = text.root().abs_bounding_box();
        let cursor_ink = cursor.root().abs_bounding_box();
        assert!(text_ink.width() > cursor_ink.width());
        assert!(text_ink.right() < cursor_ink.left());
        assert!(cursor_ink.right() <= cursor.size().width());
    }

    /// Every icon a workspace badge offers is embedded: the picker draws
    /// nothing for one that isn't.
    #[test]
    fn every_badge_icon_is_embedded() {
        let missing: Vec<_> = crate::features::workspaces::BadgeIcon::ALL
            .into_iter()
            .filter(|icon| {
                AppAssets
                    .load(icon.icon().path().as_ref())
                    .ok()
                    .flatten()
                    .is_none()
            })
            .map(|icon| icon.label())
            .collect();
        assert!(missing.is_empty(), "not embedded: {missing:?}");
    }
}
