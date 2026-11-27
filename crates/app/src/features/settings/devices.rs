//! Devices: the two switches (Use devices, Agents can use devices) and what
//! they need from this machine. Turning one on installs its package (the
//! device hub, agent-device) into the app's folder; the work itself is
//! `setup::devices`, the panel and the hub are `features::devices`.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::layout::{row, section};
use super::{SettingsEvent, SettingsView};
use crate::features::onboarding::OnboardingEvent;
use crate::features::setup::devices::{self as setup, DevicesStatus, Prefs};
use crate::shared::i18n::t;
use crate::shared::ui;

/// Install output lines kept on screen.
const LOG_KEEP: usize = 8;

#[derive(Default)]
pub(super) struct Devices {
    status: Option<DevicesStatus>,
    prefs: Prefs,
    job: Option<&'static str>,
    log: Vec<String>,
    note: Option<(String, bool)>,
    task: Option<Task<()>>,
}

/// An install, run on the background executor.
type Work = fn(&(dyn Fn(String) + Send + Sync)) -> Result<(), String>;

/// Which switch a change is for.
#[derive(Clone, Copy)]
enum Switch {
    Use,
    Agents,
}

impl SettingsView {
    pub(super) fn refresh_devices(&mut self, cx: &mut Context<Self>) {
        self.devices.prefs = setup::prefs();
        self.devices.task = Some(cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async { setup::status() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.devices.status = Some(status);
                cx.notify();
            });
        }));
    }

    /// Flip a switch. Turning one on installs what it needs first; it stays
    /// off when that fails.
    fn set_device_switch(&mut self, switch: Switch, on: bool, cx: &mut Context<Self>) {
        if self.devices.job.is_some() {
            return;
        }
        let mut prefs = self.devices.prefs;
        match switch {
            Switch::Use => prefs.enabled = on,
            Switch::Agents => prefs.agents = on,
        }
        if !on {
            self.save_device_prefs(prefs, cx);
            return;
        }
        let (label, work): (&'static str, Work) = match switch {
            Switch::Use => ("Installing the device hub", setup::install_hub),
            Switch::Agents => ("Installing agent-device", setup::install_agent),
        };
        self.devices.job = Some(label);
        self.devices.log.clear();
        self.devices.note = None;
        let (tx, rx) = async_channel::unbounded::<String>();
        self.devices.task = Some(cx.spawn(async move |this, cx| {
            let done = cx.background_executor().spawn(async move {
                let log = move |line: String| {
                    let _ = tx.send_blocking(line);
                };
                work(&log)
            });
            while let Ok(line) = rx.recv().await {
                let alive = this.update(cx, |this, cx| {
                    this.devices.log.push(line);
                    let extra = this.devices.log.len().saturating_sub(LOG_KEEP);
                    this.devices.log.drain(..extra);
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            let result = done.await;
            let status = cx
                .background_executor()
                .spawn(async { setup::status() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.devices.job = None;
                this.devices.status = Some(status);
                match result {
                    Ok(()) => {
                        this.devices.log.clear();
                        this.save_device_prefs(prefs, cx);
                    }
                    Err(error) => this.devices.note = Some((error, false)),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn save_device_prefs(&mut self, prefs: Prefs, cx: &mut Context<Self>) {
        match setup::save_prefs(prefs) {
            Ok(()) => {
                let agents_changed = prefs.agents != self.devices.prefs.agents;
                self.devices.prefs = prefs;
                self.devices.note = agents_changed.then(|| {
                    (
                        t("New agent sessions use it; open tabs pick it up after /reload.")
                            .to_string(),
                        true,
                    )
                });
                cx.emit(SettingsEvent::DevicesChanged(prefs));
            }
            Err(error) => self.devices.note = Some((error, false)),
        }
        cx.notify();
    }

    fn device_switch(
        &self,
        id: &'static str,
        label: &'static str,
        hint: String,
        switch: Switch,
        on: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        v_flex()
            .pt_2()
            .gap_0p5()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .w(rems(160. / 16.))
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(ui::fg())
                            .child(label),
                    )
                    .child(ui::segmented(
                        id,
                        &[t("Off"), t("On")],
                        Some(usize::from(on)),
                        cx.listener(move |this, index: &usize, _, cx| {
                            this.set_device_switch(switch, *index == 1, cx)
                        }),
                    )),
            )
            .child(div().text_xs().text_color(ui::faint()).child(hint))
    }

    pub(super) fn devices_section(&self, cx: &mut Context<Self>) -> Div {
        let section = section(IconName::Smartphone, t("Devices")).child(
            div().pb_1().text_xs().text_color(ui::muted()).child(t(
                "iOS simulators and Android emulators inside the app: live, touchable, docked \
                 at the right or floating. Agents open them here too, never in a separate \
                 Simulator or emulator window.",
            )),
        );
        let Some(status) = &self.devices.status else {
            return section.child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(Spinner::new().xsmall().color(ui::accent()))
                    .child(t("Looking at this machine…")),
            );
        };
        let prefs = self.devices.prefs;
        let installed = |found: bool, package: &setup::Package| {
            if found {
                format!("{} {}", package.name, package.version)
            } else {
                t("{package} (installs when you turn this on)").replace(
                    "{package}",
                    &format!("{} {}", package.name, package.version),
                )
            }
        };
        let node = match &status.node {
            Some((_, (a, b, c))) if status.node_ok() => format!("Node.js {a}.{b}.{c}"),
            Some((_, (a, b, c))) => t("Node.js {version}: 22.12 or newer is needed")
                .replace("{version}", &format!("{a}.{b}.{c}")),
            None => t("Not found: Setup & requirements installs Node.js").into(),
        };
        let ios = match &status.ios {
            Ok(()) => t("Ready (Xcode)").to_string(),
            Err(why) => t(why).to_string(),
        };
        let android = match &status.android_sdk {
            Some(sdk) => sdk.display().to_string(),
            None => t("Not found: install Android Studio and create a virtual device").into(),
        };
        let ffmpeg_missing = status.ffmpeg.is_none() && status.android_sdk.is_some();
        section
            .child(self.device_switch(
                "devices-use",
                t("Use devices"),
                t("Open simulators and emulators in the device panel. {package}")
                    .replace("{package}", &installed(status.hub.is_some(), &setup::HUB)),
                Switch::Use,
                prefs.enabled,
                cx,
            ))
            .child(self.device_switch(
                "devices-agents",
                t("Agents can use devices"),
                t("New agent sessions can open and drive these devices (gentle-device, agent-device). {package}")
                    .replace("{package}", &installed(status.agent.is_some(), &setup::AGENT)),
                Switch::Agents,
                prefs.agents,
                cx,
            ))
            .child(div().h_2())
            .child(row("Node.js", node))
            .child(row(t("iOS simulators"), ios))
            .child(row(t("Android SDK"), android))
            .child(row(
                "ffmpeg",
                match &status.ffmpeg {
                    Some(path) => path.display().to_string(),
                    None => t("Not found: Android emulator video needs it").into(),
                },
            ))
            .when(ffmpeg_missing, |d| {
                d.child(
                    h_flex().pt_2().child(
                        Button::new("devices-ffmpeg")
                            .small()
                            .ghost()
                            .icon(Icon::new(IconName::SquareTerminal))
                            .label(t("Install ffmpeg ({command})").replace("{command}", setup::ffmpeg_command()))
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SettingsEvent::Setup(OnboardingEvent::Terminal(
                                    setup::ffmpeg_command().to_string(),
                                )))
                            })),
                    ),
                )
            })
            .when_some(self.devices.job, |d, job| {
                d.child(
                    h_flex()
                        .pt_2()
                        .gap_2()
                        .text_xs()
                        .text_color(ui::muted())
                        .child(Spinner::new().xsmall().color(ui::accent()))
                        .child(format!("{}…", t(job))),
                )
            })
            .when(!self.devices.log.is_empty(), |d| {
                d.child(
                    ui::well_box().mt_2().px_3().py_2().gap_0p5().children(
                        self.devices
                            .log
                            .iter()
                            .map(|line| div().text_color(ui::muted()).child(line.clone())),
                    ),
                )
            })
            .when_some(self.devices.note.clone(), |d, (text, ok)| {
                d.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(if ok { ui::success() } else { ui::danger() })
                        .child(text),
                )
            })
    }
}
