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
        Cat
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
        "brand/badge.webp",
        include_bytes!("../../../../assets/mascot/osdy-code-logo.webp"),
    ),
    (
        "brand/badge-still.png",
        include_bytes!("../../assets/brand/badge-still.png"),
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
    use std::io::Cursor;

    use image::{AnimationDecoder, ImageFormat, codecs::webp::WebPDecoder};
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
        let badge = load("brand/badge.webp");
        let mut decoder = WebPDecoder::new(Cursor::new(badge.as_ref())).unwrap();
        assert!(decoder.has_animation());
        decoder.set_background_color(image::Rgba([0; 4])).unwrap();
        let mut frames = decoder.into_frames();
        let still = load("brand/badge-still.png");
        let still = image::load_from_memory_with_format(&still, ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        let first = frames.next().unwrap().unwrap();
        assert_eq!(first.buffer().dimensions(), still.dimensions());
        // libwebp and image-webp can round the YUV conversion differently.
        assert!(first.buffer().pixels().zip(still.pixels()).all(|(a, b)| {
            a[3] == b[3] && (a[3] == 0 || (0..3).all(|c| a[c].abs_diff(b[c]) <= 5))
        }));
        assert_ne!(frames.next().unwrap().unwrap().buffer(), first.buffer());

        let tree = |path| Tree::from_data(&load(path), &Options::default()).unwrap();
        let eyes = tree("brand/eyes-closed.svg");
        assert_eq!((eyes.size().width(), eyes.size().height()), (384., 384.));
        let text = tree("brand/wordmark-text.svg");
        let cursor = tree("brand/cursor.svg");
        assert_eq!(text.size(), cursor.size());
        let text_ink = text.root().abs_bounding_box();
        let cursor_ink = cursor.root().abs_bounding_box();
        assert!(text_ink.width() > cursor_ink.width());
        assert!(text_ink.right() < cursor_ink.left());
        assert!(cursor_ink.right() <= cursor.size().width());
    }
}
