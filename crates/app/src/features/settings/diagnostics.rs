//! The Diagnostics section: record what the app uses while a problem
//! happens, then show the file to attach to an issue.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::SettingsView;
use super::layout::section;
use crate::shared::i18n::t;
use crate::{features::diagnostics, shared::ui};

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl SettingsView {
    pub(super) fn diagnostics_section(&self, cx: &mut Context<Self>) -> Div {
        let running = diagnostics::recording(cx).map(file_name);
        let latest = diagnostics::latest();
        let status = match (&running, &latest) {
            (Some(name), _) => h_flex()
                .gap_2()
                .text_color(ui::warning())
                .child(ui::pulse(div().size_2().rounded_full().bg(ui::warning())))
                .child(t("Recording to {name}").replace("{name}", name)),
            (None, Some(path)) => h_flex()
                .text_color(ui::muted())
                .child(t("Last recording: {file}").replace("{file}", &file_name(path))),
            (None, None) => h_flex()
                .text_color(ui::faint())
                .child(t("No recordings yet.")),
        };
        let primary = if running.is_some() {
            Button::new("diagnostics-stop")
                .primary()
                .small()
                .icon(Icon::new(IconName::Square))
                .label(t("Stop and show file"))
                .on_click(cx.listener(|_, _, _, cx| {
                    diagnostics::stop_recording(cx);
                    if let Some(path) = diagnostics::latest() {
                        cx.reveal_path(&path);
                    }
                    cx.notify();
                }))
        } else {
            Button::new("diagnostics-start")
                .ghost()
                .small()
                .icon(Icon::new(IconName::Play))
                .label(t("Start recording"))
                .on_click(cx.listener(|_, _, window, cx| {
                    if let Err(error) = diagnostics::start_recording(cx) {
                        window.push_notification(Notification::warning(error), cx);
                    }
                    cx.notify();
                }))
        };
        section(IconName::Activity, t("Diagnostics")).child(
            v_flex()
                .gap_2()
                .text_xs()
                .child(div().text_color(ui::faint()).child(
                    t("If the app uses too much CPU or memory, start a recording, do what makes it happen for a few minutes, then stop. It saves only numbers and program names (never prompts, conversations or paths) and nothing leaves this computer: attach the file to your issue on GitHub."),
                ))
                .child(status)
                .child(h_flex().gap_2().flex_wrap().child(primary).when_some(
                    latest.filter(|_| running.is_none()),
                    |row, path| {
                        row.child(
                            Button::new("diagnostics-show")
                                .ghost()
                                .small()
                                .icon(Icon::new(IconName::FolderOpen))
                                .label(t("Show last recording"))
                                .on_click(move |_, _, cx| cx.reveal_path(&path)),
                        )
                    },
                )),
        )
    }
}
