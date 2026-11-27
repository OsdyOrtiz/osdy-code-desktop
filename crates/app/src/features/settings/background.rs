//! The Gentle AI section, chiefly the background-subagents policy. When the
//! active agent tab's Gentle Shell advertises `get_background_subagents` and
//! `set_background_subagents` (5.0.0), the policy is read from it and changed
//! in two calls: Gentle Shell's confirmation is shown, and only the human's
//! approval resends its token. Otherwise it is resolved from the files as the
//! shell does, and changed through the shell's own command.

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use pi_rpc::desktop_rpc::{
    BackgroundConfirmation, BackgroundPolicy as ShellPolicy, BackgroundSource, BackgroundSubagents,
    DesktopMethod, SetBackgroundSubagentsOutcome,
};
use serde_json::Value;

use super::layout::{or_dash, row, section};
use super::snapshot::Snapshot;
use super::{SettingsEvent, SettingsView};
use crate::features::session::{ReadOutcome, ReadRequest, ReadSupport, ReadTicket};
use crate::shared::i18n::{Message, t};
use crate::shared::ui;

/// Which source decided the background-subagents policy, in Gentle Shell's
/// order: project file, then global file, then environment, then off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PolicySource {
    ProjectFile,
    GlobalFile,
    Environment,
    #[default]
    Default,
}

/// The effective background-subagents policy, resolved as
/// `lib/background-subagents-policy.ts` does so the app and the shell agree.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BackgroundPolicy {
    pub on: bool,
    pub source: PolicySource,
    /// The deciding file exists but fails the strict decode (reads as off).
    pub malformed: bool,
    pub project_file: PathBuf,
}

pub const BACKGROUND_SUBAGENTS_FILE: &str = "background-subagents.json";
const BACKGROUND_SUBAGENTS_SCHEMA: &str = "gentle-pi.background-subagents/v1";

/// Strict decode: exactly `{"schema": …/v1, "policy": "on"|"off"}`.
fn parse_policy_file(raw: &str) -> Option<bool> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    if object.len() != 2 || object.get("schema")?.as_str()? != BACKGROUND_SUBAGENTS_SCHEMA {
        return None;
    }
    match object.get("policy")?.as_str()? {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// First hit wins; a present but malformed file fails closed to off and keeps
/// the decision (it does not fall through), exactly like the shell.
pub fn resolve_background_policy(cwd: &Path, config: &Path, env: Option<&str>) -> BackgroundPolicy {
    let project_file = cwd
        .join(".pi")
        .join("gentle-ai")
        .join(BACKGROUND_SUBAGENTS_FILE);
    let global_file = config.join(BACKGROUND_SUBAGENTS_FILE);
    for (source, path) in [
        (PolicySource::ProjectFile, &project_file),
        (PolicySource::GlobalFile, &global_file),
    ] {
        if !path.exists() {
            continue;
        }
        let decoded = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| parse_policy_file(&raw));
        return BackgroundPolicy {
            on: decoded == Some(true),
            source,
            malformed: decoded.is_none(),
            project_file: project_file.clone(),
        };
    }
    match env {
        Some(value @ ("on" | "off")) => BackgroundPolicy {
            on: value == "on",
            source: PolicySource::Environment,
            malformed: false,
            project_file,
        },
        _ => BackgroundPolicy {
            project_file,
            ..BackgroundPolicy::default()
        },
    }
}

impl BackgroundPolicy {
    /// A display summary of the policy and the source that determined it.
    pub fn describe(&self) -> String {
        let source = match self.source {
            PolicySource::ProjectFile => t("project file"),
            PolicySource::GlobalFile => t("global file"),
            PolicySource::Environment => "GENTLE_PI_BACKGROUND_SUBAGENTS",
            PolicySource::Default => t("default"),
        };
        let state = if self.on { t("on") } else { t("off") };
        if self.malformed {
            format!("{state} · {source} {}", t("(malformed)"))
        } else {
            format!("{state} · {source}")
        }
    }

    /// The policy Gentle Shell reported. No path crosses the wire, so the
    /// project file keeps the name the files' read gave it.
    pub(super) fn from_shell(state: &BackgroundSubagents, project_file: PathBuf) -> Self {
        Self {
            on: state.effective == ShellPolicy::On,
            source: match state.source {
                BackgroundSource::Project => PolicySource::ProjectFile,
                BackgroundSource::Global => PolicySource::GlobalFile,
                BackgroundSource::Environment => PolicySource::Environment,
                BackgroundSource::Default => PolicySource::Default,
            },
            malformed: state.malformed,
            project_file,
        }
    }
}

/// What a background-subagents ticket asked.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Asked {
    Read,
    Set(ShellPolicy),
}

/// UI feedback for the shell-owned background helper policy, and Gentle
/// Shell's own answer when the active tab's Shell serves it.
#[derive(Default)]
pub(super) struct BackgroundUiState {
    pub(super) pending: bool,
    pub(super) note: Option<(Message, bool)>,
    /// The policy as Gentle Shell reported it (RPC path only).
    pub(super) shell: Option<BackgroundSubagents>,
    /// The dialog Gentle Shell asked to show, for the policy asked.
    confirmation: Option<(ShellPolicy, BackgroundConfirmation)>,
    ticket: Option<(ReadTicket, Asked)>,
    /// Asked while the tab's probe had not answered.
    awaiting: bool,
}

impl SettingsView {
    /// Whether the active tab's Gentle Shell serves `method`.
    fn shell_serves(&self, method: DesktopMethod, cx: &App) -> ReadSupport {
        self.session
            .as_ref()
            .map_or(ReadSupport::Legacy, |session| {
                session.read(cx).read_support(method)
            })
    }

    /// Read the policy from the active tab's Gentle Shell when it serves it;
    /// otherwise the files' read (the snapshot) stays the source.
    pub(super) fn ask_background(&mut self, cx: &mut Context<Self>) {
        match self.shell_serves(DesktopMethod::GetBackgroundSubagents, cx) {
            ReadSupport::Rpc => {}
            ReadSupport::NotReady => {
                self.background.awaiting = true;
                return;
            }
            ReadSupport::Legacy => {
                self.background.awaiting = false;
                self.background.shell = None;
                return;
            }
        }
        self.background.awaiting = false;
        if self.background.ticket.is_some() {
            return;
        }
        self.send_background(ReadRequest::BackgroundSubagents, Asked::Read, cx);
    }

    fn send_background(&mut self, request: ReadRequest, asked: Asked, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.update(cx, |session, cx| session.read(request, cx)) {
            Ok(ticket) => self.background.ticket = Some((ticket, asked)),
            Err(failure) => {
                self.background.pending = false;
                if matches!(asked, Asked::Set(_)) {
                    self.background.note = Some((Message::raw(failure.message()), true));
                }
            }
        }
    }

    /// The tab changed: a probe that answered, or the answer to the
    /// background-subagents ticket.
    pub(super) fn background_changed(&mut self, cx: &mut Context<Self>) {
        if self.background.awaiting {
            if self.shell_serves(DesktopMethod::GetBackgroundSubagents, cx) != ReadSupport::NotReady
            {
                self.ask_background(cx);
                cx.notify();
            }
            return;
        }
        let (Some((ticket, asked)), Some(session)) = (self.background.ticket, self.session.clone())
        else {
            return;
        };
        let Some(outcome) = session.update(cx, |session, _| session.take_read(ticket)) else {
            return;
        };
        self.background.ticket = None;
        match (asked, outcome) {
            (Asked::Read, ReadOutcome::BackgroundSubagents(state)) => {
                self.background.shell = Some(state);
            }
            (Asked::Set(policy), ReadOutcome::BackgroundSubagentsSet(result)) => {
                self.background.pending = false;
                self.background.shell = Some(result.state);
                match result.outcome {
                    SetBackgroundSubagentsOutcome::ConfirmationRequired(confirmation) => {
                        // A stale token asks again: the fresh dialog replaces it.
                        self.background.confirmation = Some((policy, confirmation));
                    }
                    SetBackgroundSubagentsOutcome::Applied { changed } => {
                        self.background.note = Some((
                            if changed {
                                Message::new(
                                    "Gentle Shell applied it. Helpers launched from now on use it.",
                                )
                            } else {
                                Message::new(
                                    "Gentle Shell applied it; what decides here did not change.",
                                )
                            },
                            false,
                        ));
                    }
                }
            }
            // The read failed: the files' read stays shown.
            (Asked::Read, _) => {}
            (Asked::Set(_), ReadOutcome::Failed(failure)) => {
                self.background.pending = false;
                self.background.note = Some((
                    Message::with_error("Not changed: {error}", failure.message()),
                    true,
                ));
                self.ask_background(cx);
            }
            (Asked::Set(_), _) => self.background.pending = false,
        }
        self.adopt_shell(cx);
    }

    /// Change the policy machine-wide. Through Gentle Shell's
    /// `set_background_subagents` when the tab serves it (the first call only
    /// asks for the confirmation to show); otherwise as typing
    /// `/gentle:background-subagents enable|disable` would: the shell owns the
    /// file and its validation, and posts its own status report.
    fn set_background(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.background.pending {
            return;
        }
        self.background.pending = true;
        self.background.note = None;
        self.background.confirmation = None;
        let rpc = self.background.shell.is_some()
            && self.shell_serves(DesktopMethod::SetBackgroundSubagents, cx) == ReadSupport::Rpc;
        if rpc {
            let policy = if on {
                ShellPolicy::On
            } else {
                ShellPolicy::Off
            };
            self.send_background(
                ReadRequest::SetBackgroundSubagents {
                    policy,
                    confirmation_token: None,
                },
                Asked::Set(policy),
                cx,
            );
        } else {
            let verb = if on { "enable" } else { "disable" };
            cx.emit(SettingsEvent::ShellCommand(format!(
                "/gentle:background-subagents {verb}"
            )));
        }
        cx.notify();
    }

    /// The human answered Gentle Shell's confirmation: approving resends its
    /// token, declining sends nothing.
    fn answer_background(&mut self, approve: bool, cx: &mut Context<Self>) {
        let Some((policy, confirmation)) = self.background.confirmation.take() else {
            return;
        };
        if approve && !self.background.pending {
            self.background.pending = true;
            self.send_background(
                ReadRequest::SetBackgroundSubagents {
                    policy,
                    confirmation_token: Some(confirmation.token),
                },
                Asked::Set(policy),
                cx,
            );
        }
        cx.notify();
    }

    /// The shell's answer to a [`SettingsEvent::ShellCommand`]; re-reads the
    /// files so the row shows what the shell actually wrote.
    pub fn shell_done(&mut self, result: Result<(), Message>, cx: &mut Context<Self>) {
        self.background.pending = false;
        self.background.note = Some(match result {
            Ok(()) => (
                Message::new(
                    "Gentle Shell applied it; its report is in 🔔. Helpers launched from now on use it.",
                ),
                false,
            ),
            Err(error) => (error, true),
        });
        // Called from inside the tab's update (its command's answer), and the
        // reload reads the tab: it waits until that update is over.
        let view = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = view.update(cx, |this, cx| this.reload(cx));
        });
    }

    /// Gentle Shell's confirmation, exactly as sent, with its two answers.
    fn background_confirm_card(
        &self,
        policy: ShellPolicy,
        confirmation: &BackgroundConfirmation,
        cx: &mut Context<Self>,
    ) -> Div {
        let action = match policy {
            ShellPolicy::On => t("Turn on"),
            ShellPolicy::Off => t("Turn off"),
        };
        v_flex()
            .gap_2()
            .p_3()
            .rounded_xl()
            .bg(ui::surface())
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ui::fg())
                    .child(confirmation.title.clone()),
            )
            .when(!confirmation.message.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(ui::muted())
                        .whitespace_normal()
                        .child(confirmation.message.clone()),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("background-confirm")
                            .primary()
                            .small()
                            .icon(Icon::new(IconName::Check))
                            .label(action)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.answer_background(true, cx)),
                            ),
                    )
                    .child(
                        Button::new("background-cancel")
                            .ghost()
                            .small()
                            .label(t("Cancel"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.answer_background(false, cx)),
                            ),
                    ),
            )
    }

    /// Gentle Shell's agent behavior: permissions, persona and background
    /// subagents (with their existing shell-command switch).
    pub(super) fn gentle_ai_section(&self, s: &Snapshot, cx: &mut Context<Self>) -> Div {
        section(IconName::ShieldCheck, "Gentle AI")
            .child(row(
                t("Permissions (new sessions)"),
                or_dash(&s.permission_mode, "normal"),
            ))
            .child(row("Persona", or_dash(&s.persona, "gentleman")))
            .child(row(
                t("Background subagents"),
                s.background_subagents.describe(),
            ))
            .child(
                v_flex()
                    .pt_2()
                    .pb_1()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(ui::muted())
                                    .child(t("Send helpers to the background")),
                            )
                            .child(ui::segmented(
                                "background-subagents",
                                &[t("Off"), t("On")],
                                Some(usize::from(s.background_subagents.on)),
                                cx.listener(|this, index: &usize, _, cx| {
                                    this.set_background(*index == 1, cx)
                                }),
                            ))
                            .when(self.background.pending, |d| {
                                d.child(Spinner::new().xsmall())
                            }),
                    )
                    .child(
                        div().text_xs().text_color(ui::faint()).child(t(
                            "On: el Gentleman launches helpers in the background and keeps working; each answer arrives when it finishes. Applied by Gentle Shell (/gentle:background-subagents), so the terminal sees the same setting.",
                        )),
                    )
                    // The switch writes the global file; a project file wins over it.
                    .when(
                        s.background_subagents.source == PolicySource::ProjectFile,
                        |d| {
                            d.child(
                                div().text_xs().text_color(ui::warning()).child(
                                    t("{file} decides for this project, so the switch (global) won't change it here.")
                                        .replace("{file}", &s.background_subagents.project_file.display().to_string()),
                                ),
                            )
                        },
                    )
                    .when_some(
                        self.background
                            .confirmation
                            .as_ref()
                            .map(|(policy, confirmation)| {
                                self.background_confirm_card(*policy, confirmation, cx)
                            }),
                        |d, card| d.child(card),
                    )
                    .when_some(self.background.note.clone(), |d, (text, failed)| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(if failed {
                                    ui::danger()
                                } else {
                                    ui::success()
                                })
                                .child(text.render()),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pi_rpc::desktop_rpc::{
        BackgroundCapability, BackgroundPolicy as ShellPolicy, BackgroundSource,
        BackgroundSubagents, PolicyFile,
    };

    use super::{
        BACKGROUND_SUBAGENTS_FILE, BackgroundPolicy, PolicySource, resolve_background_policy,
    };

    /// Gentle Shell's answer reads as the files' read would: the same
    /// source names, the fail-closed `malformed`, and the project file's name
    /// (no path crosses the wire).
    #[test]
    fn the_shell_policy_reads_like_the_files() {
        let state = BackgroundSubagents {
            effective: ShellPolicy::Off,
            source: BackgroundSource::Project,
            malformed: true,
            project_file: PolicyFile::Present,
            global_file: PolicyFile::Absent,
            capability: BackgroundCapability::Ready,
        };
        let file = PathBuf::from("repo/.pi/gentle-ai").join(BACKGROUND_SUBAGENTS_FILE);
        let policy = BackgroundPolicy::from_shell(&state, file.clone());
        assert_eq!(policy.describe(), "off · project file (malformed)");
        assert_eq!(
            (
                policy.on,
                policy.source,
                policy.malformed,
                policy.project_file
            ),
            (false, PolicySource::ProjectFile, true, file)
        );
        let global = BackgroundSubagents {
            effective: ShellPolicy::On,
            source: BackgroundSource::Global,
            malformed: false,
            ..state
        };
        assert_eq!(
            BackgroundPolicy::from_shell(&global, PathBuf::new()).describe(),
            "on · global file"
        );
    }

    /// Same order and fail-closed rules as the shell's
    /// `resolveBackgroundSubagentsPolicy`, so the row never disagrees with it.
    #[test]
    fn background_policy_resolves_like_the_shell() {
        let root = std::env::temp_dir().join(format!("gs-bg-test-{}", std::process::id()));
        let (cwd, config) = (root.join("repo"), root.join("gentle-ai"));
        let project = cwd.join(".pi").join("gentle-ai");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        let on = r#"{"schema":"gentle-pi.background-subagents/v1","policy":"on"}"#;
        let off = r#"{"schema":"gentle-pi.background-subagents/v1","policy":"off"}"#;
        let resolve = |env| resolve_background_policy(&cwd, &config, env);

        // Nothing set: off by default; the environment applies next.
        assert_eq!(
            (resolve(None).on, resolve(None).source),
            (false, PolicySource::Default)
        );
        assert_eq!(
            (resolve(Some("on")).on, resolve(Some("on")).source),
            (true, PolicySource::Environment)
        );
        assert_eq!(
            resolve(Some("yes")).source,
            PolicySource::Default,
            "unknown env is inert"
        );

        // The global file beats the environment.
        std::fs::write(config.join(BACKGROUND_SUBAGENTS_FILE), on).unwrap();
        let global = resolve(Some("off"));
        assert_eq!((global.on, global.source), (true, PolicySource::GlobalFile));

        // A project file beats the global one.
        std::fs::write(project.join(BACKGROUND_SUBAGENTS_FILE), off).unwrap();
        let local = resolve(None);
        assert_eq!((local.on, local.source), (false, PolicySource::ProjectFile));

        // Malformed (an extra key) fails closed and keeps the decision there.
        std::fs::write(
            project.join(BACKGROUND_SUBAGENTS_FILE),
            r#"{"schema":"gentle-pi.background-subagents/v1","policy":"on","x":1}"#,
        )
        .unwrap();
        let broken = resolve(Some("on"));
        assert_eq!(
            (broken.on, broken.source, broken.malformed),
            (false, PolicySource::ProjectFile, true)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
