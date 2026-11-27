//! The code-link affordance any kind shows for an item's `ref`: a file icon
//! and `file.rs:42` that opens the file at that line in the app's file
//! viewer ([`DiagramEvent::OpenRef`]); right-click for "Open in <editor>"
//! (the installed editors, found once off the main thread) and Copy path.
//! A file that isn't there shows muted.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use gentle_core::genui::refs::CodeRef;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::*;

use super::diagram::DiagramEvent;
use super::share::{Env, RefState};
use crate::platform::ides;
use crate::shared::ui;

static EDITORS: OnceLock<Vec<ides::Installed>> = OnceLock::new();
static LOOKING: AtomicBool = AtomicBool::new(false);

/// The installed editors; the first call starts looking (it may run the
/// login shell) and gets `None` until that's done.
fn editors() -> Option<&'static [ides::Installed]> {
    if let Some(found) = EDITORS.get() {
        return Some(found);
    }
    if !LOOKING.swap(true, Ordering::Relaxed) {
        std::thread::spawn(|| {
            let _ = EDITORS.set(ides::detect());
        });
    }
    None
}

/// `code_ref` as a link for view `V`; `id` names the element (unique in the
/// view), also its debug selector.
pub fn chip<V: EventEmitter<DiagramEvent>>(
    id: String,
    code_ref: &CodeRef,
    env: &Env,
    cx: &mut Context<V>,
) -> AnyElement {
    let state = env.ref_state(code_ref);
    let (icon, color, tip) = match &state {
        RefState::Found(_) | RefState::Unknown => (
            IconName::FileCode,
            ui::accent(),
            crate::shared::i18n::t("Open {path}").replace("{path}", &code_ref.text()),
        ),
        RefState::Missing(_) => (
            IconName::FileX,
            ui::faint(),
            crate::shared::i18n::t("{path} isn't in the workspace (yet)")
                .replace("{path}", &code_ref.path),
        ),
        RefState::Outside => (
            IconName::FileX,
            ui::faint(),
            crate::shared::i18n::t("{path} is outside the workspace")
                .replace("{path}", &code_ref.path),
        ),
    };
    let file = match &state {
        RefState::Found(path) | RefState::Missing(path) => Some(path.clone()),
        _ => None,
    };
    let selector = id.clone();
    let open = code_ref.clone();
    let menu_ref = code_ref.clone();
    let weak = cx.entity().downgrade();
    div()
        .id(SharedString::from(id))
        .debug_selector(move || selector)
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .max_w(rems(200. / 16.))
        .px_1()
        .rounded_md()
        .cursor_pointer()
        .font_family(ui::mono())
        .text_size(ui::code_rems(11.))
        .text_color(color)
        .hover(|d| d.bg(ui::surface_hover()))
        .child(Icon::new(icon).xsmall())
        .child(div().min_w_0().truncate().child(code_ref.label()))
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.stop_propagation();
            cx.emit(DiagramEvent::OpenRef(open.clone()));
        }))
        .context_menu(move |menu, _, _| {
            let code_ref = menu_ref.clone();
            let mut menu = menu.item(
                PopupMenuItem::new(crate::shared::i18n::t("Open in the file viewer"))
                    .icon(Icon::new(IconName::FileCode))
                    .disabled(file.is_none())
                    .on_click({
                        let (weak, code_ref) = (weak.clone(), code_ref.clone());
                        move |_, _, cx| {
                            let open = DiagramEvent::OpenRef(code_ref.clone());
                            let _ = weak.update(cx, |_, cx| cx.emit(open));
                        }
                    }),
            );
            match (editors(), &file) {
                (Some(found), Some(file)) => {
                    for ide in found {
                        let (ide, file, line) = (ide.clone(), file.clone(), code_ref.line);
                        menu = menu.item(
                            PopupMenuItem::new(
                                crate::shared::i18n::t("Open in {editor}")
                                    .replace("{editor}", ide.name),
                            )
                            .icon(Icon::new(IconName::CodeXml))
                            .on_click(move |_, _, _| {
                                let _ = ides::open_file(&ide, &file, line);
                            }),
                        );
                    }
                }
                (None, Some(_)) => {
                    menu = menu.item(
                        PopupMenuItem::new(crate::shared::i18n::t("Looking for editors…"))
                            .disabled(true),
                    )
                }
                _ => {}
            }
            let path = code_ref.text();
            menu.separator().item(
                PopupMenuItem::new(crate::shared::i18n::t("Copy path"))
                    .icon(Icon::new(IconName::Copy))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
                    }),
            )
        })
        .into_any_element()
}
