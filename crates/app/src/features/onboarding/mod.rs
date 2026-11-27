//! Onboarding: the first-run screen (and "Setup & requirements" any time).
//! It shows what this machine has for Gentle Shell (`crate::features::setup` checks),
//! installs what can be installed unattended with a live log, hands anything
//! that needs a password or an interactive login to a terminal tab, and sends
//! provider sign-in to the Providers screen.

use std::collections::VecDeque;

use gentle_core::backend::Backend;
use gpui_kit::*;

use crate::features::setup::{self, Check, CheckId, Detail};
use crate::shared::i18n::{language, t};

mod checks;
mod install;
mod sequence;
mod view;

use checks::{install_plan, required_ok, usable, with_registry};

/// Where the checklist is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The onboarding screen: hero, checklist, footer, in a card.
    Screen,
    /// Inside Configuration: the checklist and its actions, without the
    /// screen's hero, footer and frame.
    Embedded,
    /// The welcome screen's install sequence: numbered steps on a progress
    /// rail; the welcome draws the heading and footer.
    Sequence,
}

enum Platform {
    Local(String),
    Remote(Backend),
}

impl Platform {
    fn label(&self) -> String {
        match self {
            Self::Local(label) => label.clone(),
            Self::Remote(choice) => {
                t("Agents on {backend}").replace("{backend}", &choice.label_in(language()))
            }
        }
    }
}

/// One look at the machine: what the screen shows and whether it must open.
pub struct Probe {
    platform: Platform,
    pub first_run: bool,
    pub checks: Vec<Check>,
}

impl Probe {
    /// BLOCKING: runs every check; call it on the background executor.
    pub fn run() -> Self {
        // The checks run where agents do: the badge says so.
        let platform = match crate::features::backend::saved() {
            Backend::Local => Platform::Local(setup::platform_label()),
            choice => Platform::Remote(choice),
        };
        Self {
            platform,
            first_run: setup::first_run(),
            checks: setup::run_checks(),
        }
    }

    /// Onboarding opens by itself on a first run or when something required
    /// is missing.
    pub fn needs_onboarding(&self) -> bool {
        self.first_run || !required_ok(&self.checks)
    }
}

#[derive(Clone, Debug)]
pub enum OnboardingEvent {
    /// Continue (after recording completion) or Skip.
    Close,
    /// An install finished; tabs whose backend failed may start now.
    Installed,
    /// Open the Providers screen.
    SignIn,
    /// Open a terminal tab with this command typed (needs the user: a
    /// password, an interactive login).
    Terminal(String),
}

pub struct OnboardingView {
    mode: Mode,
    /// The sequence's steps start coming in after this (the welcome's
    /// intro plays first).
    entrance: std::time::Duration,
    platform: Option<Platform>,
    first_run: bool,
    /// `None` until the first look finishes.
    checks: Option<Vec<Check>>,
    checking: bool,
    installing: Option<CheckId>,
    /// "Install everything": what still runs after the current install.
    queue: VecDeque<CheckId>,
    /// Output of the last install, and the check it belongs to.
    log: Vec<String>,
    log_for: Option<CheckId>,
    failure: Option<(CheckId, String)>,
    /// Emit [`OnboardingEvent::Installed`] once the checks after an install land.
    announce: bool,
    /// The provider check as the active agent tab's Gentle Shell answered
    /// it (`get_auth_status`), applied over every look at the machine.
    registry_sign_in: Option<Result<Detail, Detail>>,
    _probe: Task<()>,
    _job: Task<()>,
}

impl EventEmitter<OnboardingEvent> for OnboardingView {}

impl OnboardingView {
    /// `probe` is a look already taken (the startup check); `None` looks now.
    pub fn new(probe: Option<Probe>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            mode: Mode::Screen,
            entrance: std::time::Duration::ZERO,
            platform: None,
            first_run: false,
            checks: None,
            checking: false,
            installing: None,
            queue: VecDeque::new(),
            log: Vec::new(),
            log_for: None,
            failure: None,
            announce: false,
            registry_sign_in: None,
            _probe: Task::ready(()),
            _job: Task::ready(()),
        };
        match probe {
            Some(probe) => this.apply(probe, cx),
            None => this.refresh(cx),
        }
        this
    }

    /// The checklist for Configuration → Setup & requirements: looks at the
    /// machine now, installs and hands off actions like the screen does.
    pub fn embedded(cx: &mut Context<Self>) -> Self {
        let mut this = Self::new(None, cx);
        this.mode = Mode::Embedded;
        this
    }

    /// The install sequence for the welcome screen; its steps come in after
    /// `entrance`.
    pub fn sequence(
        probe: Option<Probe>,
        entrance: std::time::Duration,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::new(probe, cx);
        this.mode = Mode::Sequence;
        this.entrance = entrance;
        this
    }

    /// Steps come in right away from now on (the intro already played).
    pub fn skip_entrance(&mut self) {
        self.entrance = std::time::Duration::ZERO;
    }

    /// An install is running: the screen must stay open until it ends.
    pub fn busy(&self) -> bool {
        self.installing.is_some()
    }

    /// gentle-shell can start, as of the last look.
    pub fn usable(&self) -> bool {
        self.checks.as_deref().is_some_and(usable)
    }

    /// Everything required is ready, as of the last look.
    pub fn ready(&self) -> bool {
        self.checks.as_deref().is_some_and(required_ok)
    }

    /// No model provider is signed in yet (it can wait; Continue doesn't).
    pub fn needs_sign_in(&self) -> bool {
        self.checks.as_deref().is_some_and(|checks| {
            checks
                .iter()
                .any(|c| c.fix == setup::Fix::SignIn && !checks::is_ok(c))
        })
    }

    /// The first look at the machine is still running.
    pub fn looking(&self) -> bool {
        self.checks.is_none()
    }

    /// A look is running (the first, or "Check again").
    pub fn checking(&self) -> bool {
        self.checking
    }

    /// "Install everything" has something to run.
    pub fn can_install(&self) -> bool {
        self.checks
            .as_deref()
            .is_some_and(|checks| !install_plan(checks).is_empty())
    }

    /// Run every unattended install that's needed, in order.
    pub fn install_everything(&mut self, cx: &mut Context<Self>) {
        self.install_all(cx);
    }

    /// Look at the machine again.
    pub fn check_again(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
    }

    /// Gentle Shell's model registry answered which providers have
    /// credentials (by id): the provider check follows it from now on,
    /// instead of reading `auth.json`, `models.json` and the environment.
    pub fn adopt_signed_in(&mut self, configured: &[&str], cx: &mut Context<Self>) {
        let answer = gentle_core::setup::registry_sign_in(configured.iter().copied())
            .map(|_| {
                Detail::template(
                    "Signed in: {providers}",
                    [("{providers}", configured.join(", "))],
                )
            })
            .map_err(|_| Detail::from("No provider signed in"));
        if let Some(checks) = self.checks.as_mut() {
            with_registry(checks, &answer);
        }
        self.registry_sign_in = Some(answer);
        cx.notify();
    }
}
