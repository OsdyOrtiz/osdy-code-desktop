//! The UI templates manager (`/ui-template`): the surfaces the user saved,
//! to insert one into the conversation (the extension shows it as a new
//! surface the agent sees), rename or delete. Saving happens from each
//! surface's Export menu.

use std::path::PathBuf;

use gentle_core::genui::template::Template;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::store;
use crate::shared::ui;

pub enum TemplatesEvent {
    /// Show this template in the conversation.
    Insert(Template),
    Close,
}

pub struct TemplatesView {
    list: Vec<(PathBuf, Template)>,
    /// The template being renamed and its field.
    renaming: Option<(PathBuf, Entity<InputState>, Subscription)>,
    error: Option<crate::shared::i18n::Message>,
    focus: FocusHandle,
}

impl EventEmitter<TemplatesEvent> for TemplatesView {}

/// The header icon of a template's kind.
fn kind_icon(kind: &str) -> IconName {
    match kind {
        "table" => IconName::Table,
        "form" => IconName::ClipboardList,
        "checklist" => IconName::ListChecks,
        "kanban" => IconName::SquareKanban,
        "chart" => IconName::ChartColumn,
        "html" => IconName::CodeXml,
        _ => IconName::Workflow,
    }
}

impl TemplatesView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            list: Vec::new(),
            renaming: None,
            error: None,
            focus: cx.focus_handle(),
        };
        view.reload(cx);
        view
    }

    fn reload(&mut self, cx: &App) {
        self.list = store::dir(cx).map_or_else(Vec::new, |dir| store::templates(&dir));
    }

    /// The saved template named `name` (case-insensitive).
    pub fn find(&self, name: &str) -> Option<Template> {
        let name = name.trim();
        self.list
            .iter()
            .find(|(_, t)| t.name.eq_ignore_ascii_case(name))
            .map(|(_, t)| t.clone())
    }

    pub fn names(&self) -> Vec<String> {
        self.list.iter().map(|(_, t)| t.name.clone()).collect()
    }

    fn start_rename(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((path, template)) = self.list.get(ix) else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(template.name.clone()));
        input.update(cx, |state, cx| state.focus(window, cx));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.finish_rename(true, window, cx),
                InputEvent::Blur => this.finish_rename(false, window, cx),
                _ => {}
            });
        self.renaming = Some((path.clone(), input, subscription));
        cx.notify();
    }

    fn finish_rename(&mut self, keep: bool, _: &mut Window, cx: &mut Context<Self>) {
        let Some((path, input, _)) = self.renaming.take() else {
            return;
        };
        let name = input.read(cx).value().trim().to_string();
        if keep && !name.is_empty() {
            self.rename(&path, &name, cx);
        }
        cx.notify();
    }

    pub(super) fn rename(&mut self, path: &std::path::Path, name: &str, cx: &mut Context<Self>) {
        use crate::shared::i18n::Message;
        self.error = match store::dir(cx) {
            Some(dir) => store::rename_template(&dir, path, name)
                .err()
                .map(|e| Message::with_error("Couldn't rename it: {error}", e)),
            None => Some(Message::new("Couldn't rename it: no data folder")),
        };
        self.reload(cx);
        cx.notify();
    }

    pub(super) fn delete(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some((path, _)) = self.list.get(ix) else {
            return;
        };
        self.error = store::delete_template(path)
            .err()
            .map(|e| crate::shared::i18n::Message::with_error("Couldn't delete it: {error}", e));
        self.reload(cx);
        cx.notify();
    }

    pub(super) fn insert(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some((_, template)) = self.list.get(ix) {
            cx.emit(TemplatesEvent::Insert(template.clone()));
        }
    }

    #[cfg(test)]
    pub(super) fn list(&self) -> &[(PathBuf, Template)] {
        &self.list
    }
}

impl Render for TemplatesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .gap_2()
            .child(
                Icon::new(IconName::LayoutTemplate)
                    .small()
                    .text_color(ui::accent()),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(rems(14. / 16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui::fg())
                    .child(crate::shared::i18n::t("UI templates")),
            )
            .child(
                Button::new("genui-templates-close")
                    .ghost()
                    .xsmall()
                    .text_color(ui::muted())
                    .icon(Icon::new(IconName::X))
                    .tooltip(crate::shared::i18n::t("Close"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(TemplatesEvent::Close))),
            );
        let rows = self.list.iter().enumerate().map(|(ix, (path, template))| {
            let renaming = self
                .renaming
                .as_ref()
                .filter(|(p, ..)| p == path)
                .map(|(_, input, _)| input.clone());
            let name: AnyElement = match renaming {
                Some(input) => ui::field()
                    .flex_1()
                    .h(rems(26. / 16.))
                    .border_color(ui::accent().opacity(0.6))
                    .child(Input::new(&input).appearance(false).xsmall())
                    .into_any_element(),
                None => v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(rems(13. / 16.))
                            .text_color(ui::fg())
                            .child(template.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(rems(11. / 16.))
                            .text_color(ui::faint())
                            .child(format!("{} · {}", template.kind, template.title)),
                    )
                    .into_any_element(),
            };
            let selector = format!("genui-template-{ix}");
            h_flex()
                .id(("genui-template", ix))
                .debug_selector(move || selector)
                .group("genui-template")
                .gap_2()
                .px_2()
                .py_1p5()
                .rounded_lg()
                .hover(|d| d.bg(ui::surface_hover()))
                .child(
                    Icon::new(kind_icon(&template.kind))
                        .small()
                        .text_color(ui::info()),
                )
                .child(name)
                .child(
                    Button::new(("genui-template-rename", ix))
                        .ghost()
                        .xsmall()
                        .text_color(ui::muted())
                        .icon(Icon::new(IconName::Pencil))
                        .tooltip(crate::shared::i18n::t("Rename"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.start_rename(ix, window, cx)
                        })),
                )
                .child(
                    Button::new(("genui-template-delete", ix))
                        .ghost()
                        .xsmall()
                        .text_color(ui::muted())
                        .icon(Icon::new(IconName::Trash))
                        .tooltip(crate::shared::i18n::t("Delete"))
                        .on_click(cx.listener(move |this, _, _, cx| this.delete(ix, cx))),
                )
                .child(
                    Button::new(("genui-template-insert", ix))
                        .primary()
                        .xsmall()
                        .label(crate::shared::i18n::t("Insert"))
                        .on_click(cx.listener(move |this, _, _, cx| this.insert(ix, cx))),
                )
        });
        let empty = self.list.is_empty();
        v_flex()
            .track_focus(&self.focus)
            .w_full()
            .gap_2()
            .child(header)
            .child(
                v_flex()
                    .id("genui-templates-list")
                    .max_h(rems(320. / 16.))
                    .overflow_y_scroll()
                    .gap_0p5()
                    .children(rows)
                    .when(empty, |d| {
                        d.child(
                            div()
                                .px_2()
                                .py_3()
                                .text_size(rems(12. / 16.))
                                .text_color(ui::muted())
                                .whitespace_normal()
                                .child(
                                    crate::shared::i18n::t("No templates yet. Save one from a surface's Export menu (Save as template)."),
                                ),
                        )
                    }),
            )
            .children(self.error.clone().map(|error| {
                div()
                    .text_size(rems(12. / 16.))
                    .text_color(ui::danger())
                    .child(error.render())
            }))
    }
}
