//! Where agents run: this computer, a WSL distro (Windows) or an environment
//! a command reaches (a Linux VM through OrbStack or Lima, a container). The
//! choice is `features::backend`'s; new agent tabs start there, open tabs
//! stay where they started.

use gentle_core::backend::{Backend, Distro};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::SettingsView;
use super::layout::{control, section};
use crate::features::backend;
use crate::shared::i18n::t;
use crate::shared::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Local,
    Wsl,
    Command,
}

impl Kind {
    /// The options this computer offers: WSL only on Windows.
    fn all() -> &'static [Kind] {
        if cfg!(windows) {
            &[Kind::Local, Kind::Wsl, Kind::Command]
        } else {
            &[Kind::Local, Kind::Command]
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Local => t("This computer"),
            Kind::Wsl => "WSL",
            Kind::Command => t("Command"),
        }
    }

    fn of(choice: &Backend) -> Kind {
        match choice {
            Backend::Local => Kind::Local,
            Backend::Wsl { .. } => Kind::Wsl,
            Backend::Command { .. } => Kind::Command,
        }
    }
}

pub(super) struct RunsOn {
    /// What new agent tabs use, as saved.
    choice: Backend,
    /// The option shown: Command shows its field before a command is saved.
    kind: Kind,
    /// WSL's distros and whether each has `gentle-shell`; `None` while
    /// looking.
    distros: Option<Vec<(Distro, bool)>>,
    command: Entity<InputState>,
    _command: Subscription,
    /// Whether the saved backend has `gentle-shell`, once checked.
    found: Option<bool>,
    checking: bool,
    task: Task<()>,
    check: Task<()>,
    /// Pointing Configuration at the new backend's Gentle Shell home.
    homes: Task<()>,
}

impl RunsOn {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SettingsView>) -> Self {
        let choice = backend::saved();
        let text = match &choice {
            Backend::Command { prefix } => prefix.join(" "),
            _ => String::new(),
        };
        let command = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("orb -m dev --");
            state.set_value(text, window, cx);
            state
        });
        let subscription = cx.subscribe(&command, |this, input, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let prefix = pi_rpc::split_args(&input.read(cx).value());
                if !prefix.is_empty() {
                    this.choose_runs_on(Backend::Command { prefix }, cx);
                }
            }
        });
        Self {
            kind: Kind::of(&choice),
            choice,
            distros: None,
            command,
            _command: subscription,
            found: None,
            checking: false,
            task: Task::ready(()),
            check: Task::ready(()),
            homes: Task::ready(()),
        }
    }
}

impl SettingsView {
    /// Look for WSL's distros (Windows) and check the saved backend.
    pub(super) fn refresh_runs_on(&mut self, cx: &mut Context<Self>) {
        if cfg!(windows) {
            self.runs_on.task = cx.spawn(async move |this, cx| {
                let distros = cx
                    .background_executor()
                    .spawn(async {
                        backend::wsl_distros()
                            .into_iter()
                            .map(|distro| {
                                let found = backend::has_gentle_shell(&Backend::Wsl {
                                    distro: distro.name.clone(),
                                });
                                (distro, found)
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.runs_on.distros = Some(distros);
                    cx.notify();
                });
            });
        }
        self.check_runs_on(cx);
    }

    /// Whether the saved backend has `gentle-shell`.
    fn check_runs_on(&mut self, cx: &mut Context<Self>) {
        let choice = self.runs_on.choice.clone();
        if choice == Backend::Local {
            self.runs_on.found = None;
            self.runs_on.checking = false;
            return;
        }
        self.runs_on.checking = true;
        self.runs_on.check = cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn({
                    let choice = choice.clone();
                    async move { backend::has_gentle_shell(&choice) }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.runs_on.choice == choice {
                    this.runs_on.found = Some(found);
                    this.runs_on.checking = false;
                    cx.notify();
                }
            });
        });
    }

    /// Save where new agent tabs run, check it has `gentle-shell`, and read
    /// Configuration from its Gentle Shell home.
    fn choose_runs_on(&mut self, choice: Backend, cx: &mut Context<Self>) {
        self.runs_on.kind = Kind::of(&choice);
        if self.runs_on.choice == choice {
            cx.notify();
            return;
        }
        if let Err(err) = backend::save(&choice) {
            eprintln!("gentle-shell-gpui: could not save where agents run: {err}");
        }
        self.runs_on.choice = choice;
        self.runs_on.found = None;
        self.check_runs_on(cx);
        self.runs_on.homes = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .spawn(async { backend::sync_homes() })
                .await;
            let _ = this.update(cx, |this, cx| this.reload(cx));
        });
        cx.notify();
    }

    /// Picking WSL takes the distro to suggest: the default one with
    /// `gentle-shell`, else any with it, else the default.
    fn suggested_distro(&self) -> Option<String> {
        let distros = self.runs_on.distros.as_ref()?;
        distros
            .iter()
            .find(|(d, found)| d.default && *found)
            .or_else(|| distros.iter().find(|(_, found)| *found))
            .or_else(|| distros.iter().find(|(d, _)| d.default))
            .map(|(d, _)| d.name.clone())
    }

    fn pick_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        match kind {
            Kind::Local => self.choose_runs_on(Backend::Local, cx),
            Kind::Wsl => match self.suggested_distro() {
                Some(distro) => self.choose_runs_on(Backend::Wsl { distro }, cx),
                None => {
                    self.runs_on.kind = kind;
                    cx.notify();
                }
            },
            Kind::Command => {
                self.runs_on.kind = kind;
                cx.notify();
            }
        }
    }

    pub(super) fn runs_on_section(&self, cx: &mut Context<Self>) -> Div {
        let kinds = Kind::all();
        let labels: Vec<&'static str> = kinds.iter().map(|k| k.label()).collect();
        let selected = kinds.iter().position(|k| *k == self.runs_on.kind);
        let pick = cx.listener(move |this, index: &usize, _, cx| {
            if let Some(kind) = Kind::all().get(*index) {
                this.pick_kind(*kind, cx);
            }
        });
        let detail: AnyElement = match self.runs_on.kind {
            Kind::Local => div().into_any_element(),
            Kind::Wsl => self.render_distros(cx).into_any_element(),
            Kind::Command => control(
                t("Command"),
                false,
                ui::field()
                    .flex_1()
                    .min_w_0()
                    .h(rems(30. / 16.))
                    .child(
                        Icon::new(IconName::SquareTerminal)
                            .small()
                            .text_color(ui::faint()),
                    )
                    .child(Input::new(&self.runs_on.command).appearance(false).small()),
            )
            .into_any_element(),
        };
        section(IconName::Server, t("Where agents run"))
            .child(control(
                t("Agents run on"),
                false,
                ui::segmented("runs-on", &labels, selected, pick),
            ))
            .child(detail)
            .child(self.render_runs_on_status())
            .child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(ui::faint())
                    .child(match self.runs_on.kind {
                        Kind::Command => {
                            t("A command that runs a program in that environment, as argv (OrbStack: orb -m dev --, Lima: limactl shell dev, a container: docker exec -i dev). Press Enter to use it.")
                        }
                        _ => {
                            t("Agents, terminals, git and Setup run here; open agent tabs stay where they started. A project opened from \\\\wsl.localhost\\<distro>\\ always runs in that distro.")
                        }
                    }),
            )
    }

    fn render_distros(&self, cx: &mut Context<Self>) -> Div {
        let Some(distros) = &self.runs_on.distros else {
            return control(
                t("Distro"),
                false,
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(Spinner::new().xsmall().color(ui::muted()))
                    .child(t("Looking for WSL distros…")),
            );
        };
        if distros.is_empty() {
            return control(
                t("Distro"),
                false,
                div()
                    .text_xs()
                    .text_color(ui::muted())
                    .child(t("No WSL distros found. Install one with wsl --install.")),
            );
        }
        let current = match &self.runs_on.choice {
            Backend::Wsl { distro } => Some(distro.as_str()),
            _ => None,
        };
        let rows = distros.iter().enumerate().map(|(ix, (distro, found))| {
            let selected = current.is_some_and(|c| c.eq_ignore_ascii_case(&distro.name));
            let name = distro.name.clone();
            let note = match (found, distro.default) {
                (true, true) => t("default · gentle-shell found"),
                (true, false) => t("gentle-shell found"),
                (false, true) => t("default · no gentle-shell"),
                (false, false) => t("no gentle-shell"),
            };
            h_flex()
                .id(("runs-on-distro", ix))
                .h(rems(26. / 16.))
                .px_2()
                .gap_2()
                .rounded_md()
                .cursor_pointer()
                .when(selected, |d| d.bg(ui::raised()))
                .when(!selected, |d| d.hover(|s| s.bg(ui::surface_hover())))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.choose_runs_on(
                        Backend::Wsl {
                            distro: name.clone(),
                        },
                        cx,
                    )
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(if selected { ui::fg() } else { ui::muted() })
                        .child(distro.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(if *found { ui::faint() } else { ui::warning() })
                        .child(note),
                )
        });
        control(
            t("Distro"),
            false,
            v_flex().flex_1().min_w_0().gap_px().children(rows),
        )
    }

    /// Whether the saved backend has `gentle-shell`, under the controls.
    fn render_runs_on_status(&self) -> Div {
        let line = |icon: IconName, color: Hsla, text: String| {
            h_flex()
                .pt_2()
                .gap_2()
                .text_xs()
                .text_color(color)
                .child(Icon::new(icon).xsmall())
                .child(text)
        };
        let label = self.runs_on.choice.label();
        match (
            &self.runs_on.choice,
            self.runs_on.checking,
            self.runs_on.found,
        ) {
            (Backend::Local, _, _) => div(),
            (_, true, _) => h_flex()
                .pt_2()
                .gap_2()
                .text_xs()
                .text_color(ui::muted())
                .child(Spinner::new().xsmall().color(ui::muted()))
                .child(t("Checking {label}…").replace("{label}", &label)),
            (_, false, Some(true)) => line(
                IconName::CircleCheck,
                ui::success(),
                t("New agent tabs run on {label}.").replace("{label}", &label),
            ),
            (_, false, Some(false)) => line(
                IconName::CircleAlert,
                ui::warning(),
                t("gentle-shell wasn't found on {label}. Install it there first.")
                    .replace("{label}", &label),
            ),
            (_, false, None) => div(),
        }
    }
}
