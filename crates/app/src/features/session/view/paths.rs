//! Code spans in the agent's answers that name a file or folder on disk,
//! after Tern's: the folder muted and the name in the foreground, on the
//! code spans' backing; right-click to open it in the file viewer or its
//! default app, reveal it, or copy its path. A span that only looks like a
//! path (`origin/dev`, a domain) stays a plain code span: the TextView
//! plugin claims a span when it names something that exists
//! ([`gentle_core::markdown::paths`] says where to look).

use crate::shared::i18n::t;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gentle_core::markdown::paths;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::text::{
    InlineElement, InlineRenderContext, MarkdownNode, MarkdownParseContext, MarkdownPlugin,
    markdown_ast::Node,
};
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::{SessionEvent, SessionView};
use crate::features::files::REVEAL;
use crate::shared::ui;

const NAME: &str = "answer-path";

/// How long a lookup stands before the disk is asked again: streaming
/// reparses an answer's last block per chunk, and a file the agent names
/// before writing it shows once it's there.
const RECHECK: Duration = Duration::from_secs(5);

/// The places the answers of a tab name, under its folder: whether each
/// exists, asked once per [`RECHECK`] (parsing runs off the main thread;
/// rendering never touches the disk).
pub(in crate::features::session) struct PathLookup {
    cwd: PathBuf,
    home: Option<PathBuf>,
    seen: Mutex<HashMap<String, (Instant, Option<Found>)>>,
}

/// A span that names something on disk.
#[derive(Clone)]
struct Found {
    path: PathBuf,
    line: Option<u32>,
    dir: bool,
}

impl PathLookup {
    pub(in crate::features::session) fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            home: dirs::home_dir(),
            seen: Mutex::default(),
        }
    }

    fn find(&self, text: &str) -> Option<Found> {
        let span = paths::resolve(text, &self.cwd, self.home.as_deref())?;
        let now = Instant::now();
        let mut seen = self.seen.lock().ok()?;
        if let Some((at, found)) = seen.get(text)
            && now.duration_since(*at) < RECHECK
        {
            return found.clone();
        }
        let found = std::fs::metadata(&span.path).ok().map(|meta| Found {
            path: span.path,
            line: span.line,
            dir: meta.is_dir(),
        });
        seen.insert(text.to_string(), (now, found.clone()));
        found
    }
}

/// The plugin a tab's Markdown views take (see `SessionView::markdown_view`).
pub(super) struct AnswerPaths {
    pub lookup: Arc<PathLookup>,
    pub view: WeakEntity<SessionView>,
}

impl MarkdownPlugin for AnswerPaths {
    fn name(&self) -> &str {
        NAME
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::InlineCode(code) = node else {
            return None;
        };
        let found = self.lookup.find(&code.value)?;
        // Plain copy takes the path; the Markdown source (with its
        // backticks) is filled in by the view.
        Some(MarkdownNode::new(NAME, found).text(code.value.clone()))
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let found = node.data::<Found>()?.clone();
        let (folder, name) = paths::split(node.as_text());
        let id = node.source_range().map_or(0, |range| range.start);
        let view = self.view.clone();
        // The code span's box (`TextView` draws a code run at 7/8 of the
        // text size, its box hugging the glyphs): 4 px aside, the theme's
        // small radius, its text on the line's baseline.
        let size = context.font_size() * 0.875;
        let line_height = size * 1.25;
        let family = ui::mono();
        let font_id = window.text_system().resolve_font(&font(family.clone()));
        // Where a line of text puts its baseline (half the leading above
        // the ascent); the font tables give the descent negative.
        let ascent = window.text_system().ascent(font_id, size);
        let descent = window.text_system().descent(font_id, size).abs();
        let baseline = (line_height - ascent - descent) / 2. + ascent;
        // Its own id: a plain div's hover doesn't take inside the flow.
        let chip = div()
            .id("chip")
            .flex()
            .min_w_0()
            .max_w(rems(24.))
            .px(px(4.))
            .rounded(gpui_kit::base::Theme::global(cx).tokens.radius.sm)
            .bg(ui::code_backing())
            .font_family(family)
            .text_size(size)
            .line_height(line_height)
            .hover(|chip| chip.text_decoration_1())
            .when(!folder.is_empty(), |chip| {
                chip.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(ui::muted())
                        .child(SharedString::from(folder.to_string())),
                )
            })
            .child(
                div()
                    .flex_none()
                    .text_color(ui::fg())
                    .child(SharedString::from(name.to_string())),
            );
        // A link: the pointer, a thin underline on hover (each part in its
        // color), a click opens it; dragging across still selects. The
        // menu sits outside the chip (its mono and underline stay there),
        // in the interface font, as the app's other menus.
        let (open, menu_view) = (found.clone(), view.clone());
        let span = div()
            .id((NAME, id))
            .debug_selector(move || format!("{NAME}-{id}"))
            .flex()
            .cursor_pointer()
            .font_family(cx.theme().font_family.clone())
            .child(chip)
            .on_click(move |_, _, cx| open_path(&open, &view, cx))
            .context_menu(move |menu, _, _| menu_for(menu, &found, &menu_view));
        Some(InlineElement::new(span).with_baseline(baseline))
    }
}

/// A click on a path: a file opens in the file viewer (at its line), a
/// folder in the system's file manager.
fn open_path(found: &Found, view: &WeakEntity<SessionView>, cx: &mut App) {
    if found.dir {
        cx.open_with_system(&found.path);
        return;
    }
    let open = SessionEvent::OpenFile {
        path: found.path.clone(),
        line: found.line,
    };
    let _ = view.update(cx, |_, cx| cx.emit(open));
}

/// Tern's menu for a path: open it (here for a file; a folder opens in the
/// file manager, which is also its default app), reveal it, copy it.
fn menu_for(
    menu: gpui_kit::component::menu::PopupMenu,
    found: &Found,
    view: &WeakEntity<SessionView>,
) -> gpui_kit::component::menu::PopupMenu {
    let open = {
        let (found, view) = (found.clone(), view.clone());
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| open_path(&found, &view, cx)
    };
    let (label, icon) = if found.dir {
        (t("Open folder"), IconName::FolderOpen)
    } else {
        (t("Open file"), IconName::File)
    };
    let menu = menu.item(
        PopupMenuItem::new(label)
            .icon(Icon::new(icon))
            .on_click(open),
    );
    let menu = if found.dir {
        menu
    } else {
        let path = found.path.clone();
        menu.item(
            PopupMenuItem::new(t("Open in default app"))
                .icon(Icon::new(IconName::ArrowUpRight))
                .on_click(move |_, _, cx| cx.open_with_system(&path)),
        )
    };
    let path = found.path.clone();
    let reveal = move |_: &ClickEvent, _: &mut Window, cx: &mut App| cx.reveal_path(&path);
    let path = found.path.display().to_string();
    let copy = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
    };
    menu.item(
        PopupMenuItem::new(t(REVEAL))
            .icon(Icon::new(IconName::FolderInput))
            .on_click(reveal),
    )
    .item(
        PopupMenuItem::new(t("Copy path"))
            .icon(Icon::new(IconName::Copy))
            .on_click(copy),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui_kit::component::text::{TextView, TextViewState};
    use gpui_kit::{
        AppContext as _, Entity, FocusHandle, InteractiveElement as _, Modifiers, MouseButton,
        ParentElement as _, Render, Styled as _, WeakEntity, point, px,
    };

    use super::{AnswerPaths, PathLookup};

    struct Answer {
        state: Entity<TextViewState>,
        lookup: Arc<PathLookup>,
        focus: FocusHandle,
    }

    impl Render for Answer {
        fn render(
            &mut self,
            _: &mut gpui_kit::Window,
            _: &mut gpui_kit::Context<Self>,
        ) -> impl gpui_kit::IntoElement {
            gpui_kit::div()
                .id("focus")
                .track_focus(&self.focus)
                .w(px(600.))
                .child(
                    gpui_kit::div().debug_selector(|| "answer".into()).child(
                        TextView::new(&self.state)
                            .selectable(true)
                            .plugin(AnswerPaths {
                                lookup: self.lookup.clone(),
                                view: WeakEntity::new_invalid(),
                            }),
                    ),
                )
        }
    }

    #[gpui::test]
    fn a_path_that_exists_shows_as_one_and_copies_as_text(cx: &mut gpui_kit::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("answer-paths-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("build")).unwrap();
        std::fs::write(dir.join("build/ui.js"), "").unwrap();
        let source = "see `build/ui.js` and `origin/dev` now";
        cx.update(gpui_kit::init);
        let lookup = Arc::new(PathLookup::new(dir.clone()));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let view = cx.new(|cx| Answer {
                state: cx.new(|cx| TextViewState::markdown(source, cx)),
                lookup,
                focus,
            });
            gpui_kit::base::Root::new(view, window, cx)
        });
        cx.run_until_parked();
        // `debug_bounds` takes a `'static` selector.
        let path_at = |text: &str| {
            format!("{}-{}", super::NAME, source.find(text).unwrap()).leak() as &'static str
        };
        assert!(cx.debug_bounds(path_at("`build/ui.js`")).is_some());
        // A branch looks like a path but names nothing here: a code span.
        assert!(cx.debug_bounds(path_at("`origin/dev`")).is_none());

        let bounds = cx.debug_bounds("answer").unwrap();
        let (start, end) = (
            point(bounds.left() + px(1.), bounds.center().y),
            point(bounds.right() - px(1.), bounds.center().y),
        );
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        });
        cx.run_until_parked();
        let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
        assert_eq!(
            copied.as_deref(),
            Some("see build/ui.js and origin/dev now")
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
