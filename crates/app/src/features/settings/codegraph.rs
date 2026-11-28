//! Settings' Codegraph card: probe registration without starting the server.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::layout::{row, section};
use super::{SettingsEvent, SettingsView};
use crate::features::setup::codegraph::{self, CodegraphStatus};
use crate::shared::{i18n::t, ui};

const LOG_KEEP: usize = 8;
type Work = fn(&(dyn Fn(String) + Send + Sync)) -> Result<(), String>;

#[derive(Default)]
pub(super) struct Codegraph {
    status: Option<Result<CodegraphStatus, &'static str>>,
    job: Option<&'static str>,
    log: Vec<String>,
    error: Option<String>,
    task: Option<Task<()>>,
}

impl SettingsView {
    pub(super) fn refresh_codegraph(&mut self, cx: &mut Context<Self>) {
        self.run_codegraph("Checking Codegraph…", None, cx);
    }

    fn run_codegraph(&mut self, label: &'static str, work: Option<Work>, cx: &mut Context<Self>) {
        if self.codegraph.job.is_some() {
            return;
        }
        // The active project's backend, not only the saved default: a WSL
        // folder or a remote tab must never get the host's Codegraph.
        if crate::features::backend::for_tab(&self.cwd) != gentle_core::backend::Backend::Local {
            self.codegraph.status = Some(Err(codegraph::LOCAL_ONLY));
            cx.notify();
            return;
        }
        self.codegraph.job = Some(label);
        self.codegraph.log.clear();
        self.codegraph.error = None;
        self.codegraph.task = Some(cx.spawn(async move |this, cx| {
            let (tx, rx) = async_channel::unbounded::<String>();
            let done = cx.background_executor().spawn(async move {
                let result = work.map(|work| {
                    work(&|line| {
                        let _ = tx.send_blocking(line);
                    })
                });
                drop(tx);
                (result, codegraph::status())
            });
            while let Ok(line) = rx.recv().await {
                if this
                    .update(cx, |this, cx| {
                        let log = &mut this.codegraph.log;
                        log.push(line);
                        log.drain(..log.len().saturating_sub(LOG_KEEP));
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            let (result, status) = done.await;
            let _ = this.update(cx, |this, cx| {
                this.codegraph.job = None;
                this.codegraph.status = Some(status);
                if let Some(result) = result {
                    this.codegraph.error = result.err();
                    // Reload tabs when idle, and re-read mcp.json for the MCP list.
                    cx.emit(SettingsEvent::Changed);
                    this.reload(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn codegraph_section(&self, cx: &mut Context<Self>) -> Div {
        let section = section(IconName::Bot, t("Useful extras"))
            .child(div().text_sm().text_color(ui::fg()).child("Codegraph"))
            .child(
                div()
                    .pb_2()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(t(codegraph::SUMMARY)),
            );
        let busy = self.codegraph.job.is_some();
        let action = |id: &'static str, icon: IconName, label: &'static str| {
            Button::new(id)
                .small()
                .ghost()
                .icon(Icon::new(icon))
                .label(t(label))
        };
        let section = match &self.codegraph.status {
            Some(Ok(status)) => {
                let runnable = status.version.is_some();
                let (state, color) = match (runnable, status.connected) {
                    // Registration only: the live state is in the MCP servers list.
                    (true, true) => (t("Registered for the agent"), ui::success()),
                    (true, false) => (t("Installed, not registered for the agent"), ui::warning()),
                    (false, true) => (t("Registered, but the CLI is unavailable"), ui::warning()),
                    (false, false) if status.cli.is_some() => (t("CLI unavailable"), ui::warning()),
                    (false, false) => (t("Not installed"), ui::faint()),
                };
                let cli = match (&status.cli, &status.version) {
                    (Some(path), Some(version)) => format!("{version} · {}", path.display()),
                    (Some(path), None) => format!(
                        "{} · {}",
                        path.display(),
                        t("Codegraph CLI did not report a version.")
                    ),
                    (None, _) => t("Not installed").into(),
                };
                section
                    .child(h_flex().pb_2().child(ui::chip(state, color)))
                    .child(row("CLI", cli))
                    .child(row(
                        t("Agent"),
                        if status.connected {
                            t("Registered in Gentle Shell's MCP configuration")
                        } else {
                            t("Not registered")
                        },
                    ))
                    .child(
                        h_flex()
                            .pt_2()
                            .gap_2()
                            .flex_wrap()
                            .when(!runnable || !status.connected, |d| {
                                d.child(
                                    action(
                                        "codegraph-install",
                                        IconName::ArrowDown,
                                        if runnable {
                                            "Connect to the agent"
                                        } else {
                                            "Install Codegraph"
                                        },
                                    )
                                    .disabled(busy)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.run_codegraph(
                                                if runnable {
                                                    "Registering Codegraph with the agent…"
                                                } else {
                                                    "Installing Codegraph…"
                                                },
                                                Some(if runnable {
                                                    codegraph::connect
                                                } else {
                                                    codegraph::install
                                                }),
                                                cx,
                                            );
                                        },
                                    )),
                                )
                            })
                            .when(status.connected, |d| {
                                d.child(
                                    action("codegraph-disconnect", IconName::Unlink, "Disconnect")
                                        .disabled(busy)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.run_codegraph(
                                                "Disconnecting Codegraph from the agent…",
                                                Some(codegraph::disconnect),
                                                cx,
                                            );
                                        })),
                                )
                            }),
                    )
            }
            Some(Err(message)) => {
                section.child(div().text_xs().text_color(ui::muted()).child(t(message)))
            }
            None => section,
        };
        section
            .child(div().pt_2().text_xs().text_color(ui::faint()).child(t(
                "Run codegraph init in each project to build its code graph.",
            )))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        action("codegraph-docs", IconName::ArrowUpRight, "Docs").on_click(
                            |_, _, cx| cx.open_url("https://colbymchenry.github.io/codegraph/"),
                        ),
                    )
                    .child(
                        action("codegraph-refresh", IconName::RefreshCw, "Check again")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_codegraph(cx))),
                    ),
            )
            .when_some(self.codegraph.job, |d, job| {
                d.child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .text_color(ui::muted())
                        .child(Spinner::new().xsmall().color(ui::accent()))
                        .child(t(job)),
                )
            })
            .when(!self.codegraph.log.is_empty(), |d| {
                d.child(
                    ui::well_box().mt_2().px_3().py_2().gap_0p5().children(
                        self.codegraph
                            .log
                            .iter()
                            .map(|line| div().text_color(ui::muted()).child(line.clone())),
                    ),
                )
            })
            .when_some(self.codegraph.error.clone(), |d, error| {
                d.child(div().pt_2().text_xs().text_color(ui::danger()).child(error))
            })
    }
}
