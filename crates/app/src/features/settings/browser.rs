//! Browser for Configuration → Devices: where the pages agents open go
//! (inside the app or in the user's browser) and which browser that is. The
//! choices live in `browser::external`; the browsers found here come from
//! `platform::browsers`.

use gentle_core::web_browsers::{AgentPages, SYSTEM_DEFAULT, WebBrowser};
use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;

use super::SettingsView;
use super::layout::{control, section};
use crate::features::browser::external;
use crate::platform::browsers;
use crate::shared::i18n::t;
use crate::shared::ui;

/// The browsers found on this machine, once looked for.
#[derive(Default)]
pub(super) struct Browsers {
    found: Option<Vec<&'static WebBrowser>>,
    _task: Option<Task<()>>,
}

impl SettingsView {
    pub(super) fn refresh_browsers(&mut self, cx: &mut Context<Self>) {
        self.browsers._task = Some(cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async { browsers::detect() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.browsers.found = Some(found.into_iter().map(|i| i.browser).collect());
                cx.notify();
            });
        }));
    }

    pub(super) fn browser_section(&self, cx: &mut Context<Self>) -> Div {
        let pages = external::agent_pages(cx);
        let labels = AgentPages::ALL.map(|pages| t(pages.label()));
        let section = section(IconName::Globe, t("Browser"))
            .child(div().pb_1().text_xs().text_color(ui::muted()).child(t(
                "Where the pages agents open go: a dev server, a preview, the design canvas. \
                     Pages agents drive (gentle-browser) always stay inside the app.",
            )))
            .child(control(
                t("Open pages"),
                false,
                ui::segmented(
                    "agent-pages",
                    &labels,
                    AgentPages::ALL.iter().position(|p| *p == pages),
                    cx.listener(|_, index: &usize, _, cx| {
                        external::set_agent_pages(AgentPages::ALL[*index], cx);
                        cx.notify();
                    }),
                ),
            ));
        if pages != AgentPages::Browser {
            return section.child(div().pt_2().text_xs().text_color(ui::faint()).child(t(
                "Choose In my browser to pick which browser opens them; ↗ on a page opens your \
                     system's default browser.",
            )));
        }
        section.child(
            v_flex()
                .pt_2()
                .gap_0p5()
                .child(div().text_xs().text_color(ui::muted()).child(t(
                    "Which browser · it comes to the front with each page; ↗ uses it too",
                )))
                .child(self.browser_list(cx)),
        )
    }

    /// System default, then every browser found here; the chosen one has
    /// the check.
    fn browser_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(found) = &self.browsers.found else {
            return h_flex()
                .pt_1()
                .gap_2()
                .text_xs()
                .text_color(ui::muted())
                .child(Spinner::new().xsmall().color(ui::accent()))
                .child(t("Looking for browsers…"))
                .into_any_element();
        };
        let chosen = external::browser(cx);
        let mut choices: Vec<Option<&'static WebBrowser>> = vec![None];
        choices.extend(found.iter().copied().map(Some));
        // A saved browser that isn't here any more still shows, so the
        // choice is visible (the system's default opens pages meanwhile).
        if let Some(browser) = chosen
            && !found.iter().any(|b| b.id == browser.id)
        {
            choices.push(Some(browser));
        }
        let missing = |browser: &WebBrowser| !found.iter().any(|b| b.id == browser.id);
        v_flex()
            .pt_1()
            .max_w(rems(420. / 16.))
            .children(choices.into_iter().enumerate().map(|(ix, choice)| {
                let selected = choice.map(|b| b.id) == chosen.map(|b| b.id);
                let label = match choice {
                    None => t(SYSTEM_DEFAULT).to_string(),
                    Some(browser) if missing(browser) => {
                        t("{browser} (not found)").replace("{browser}", browser.name)
                    }
                    Some(browser) => browser.name.to_string(),
                };
                ui::picker_row(
                    ("external-browser", ix),
                    false,
                    selected,
                    Some(if choice.is_none() {
                        IconName::Globe
                    } else {
                        IconName::ArrowUpRight
                    }),
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(if selected { ui::fg() } else { ui::muted() })
                        .child(label),
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    external::set_browser(choice, cx);
                    cx.notify();
                }))
            }))
            .into_any_element()
    }
}
