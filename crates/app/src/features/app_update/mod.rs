//! Updates for this app itself, from its GitHub releases.
//!
//! The latest release is checked on startup (stable builds only, cached for
//! six hours) and on demand from Configuration → App. Nothing is installed
//! until the user clicks: the platform archive is downloaded, verified against
//! the release's `SHA256SUMS`, extracted and swapped in place of the running
//! app, and "Restart now" relaunches it once this process has quit.
//!
//! The cache and the "Skip this version" choice live in
//! `<data dir>/osdy-code-desktop/app-update.json`.

use std::io;
use std::path::PathBuf;

use gpui_kit::{App, BorrowAppContext as _, Global, Task};

use crate::shared::i18n::t;

mod archive;
mod cache;
mod install;
mod network;
mod relaunch;
mod release;

pub use archive::{extract_tar_gz, extract_tar_xz, extract_zip};
pub use release::{Release, expected_digest, is_dev};

use cache::{check_blocking, skip_blocking};
use install::install_blocking;
use relaunch::{remove_leftovers, spawn_relaunch};
use release::{Outcome, platform_asset};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/OsdyOrtiz/osdy-code-desktop/releases";
const SUMS: &str = "SHA256SUMS";
/// The executable inside every archive (`.exe` on Windows).
const BINARY: &str = "gentle-shell-gpui";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub message: String,
    /// Somewhere to go instead (the releases page).
    pub link: Option<String>,
    key: &'static str,
    args: Vec<(&'static str, String)>,
    cause: Option<String>,
}

impl Problem {
    fn new(key: &'static str) -> Self {
        Self {
            message: key.into(),
            link: None,
            key,
            args: Vec::new(),
            cause: None,
        }
    }

    fn with_releases(key: &'static str) -> Self {
        Self {
            link: Some(RELEASES_URL.to_string()),
            ..Self::new(key)
        }
    }

    fn arg(mut self, name: &'static str, value: impl ToString) -> Self {
        self.args.push((name, value.to_string()));
        self.message = self.format(self.key);
        self
    }

    fn cause(mut self, error: impl ToString) -> Self {
        self.cause = Some(error.to_string());
        self.message = self.format(self.key);
        self
    }

    fn format(&self, text: &str) -> String {
        let mut text = text.to_string();
        for (name, value) in &self.args {
            text = text.replace(name, value);
        }
        if let Some(cause) = &self.cause {
            text.push_str(": ");
            text.push_str(cause);
        }
        text
    }
}

fn fail(what: &'static str) -> impl FnOnce(io::Error) -> Problem {
    move |e| Problem::new(what).cause(e)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Downloading(u64, Option<u64>),
    Installing,
}

// ---------------------------------------------------------------------------
// App state

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No answer yet (development builds don't check on startup).
    Idle,
    Checking,
    UpToDate,
    Skipped(String),
    Available(Release),
    Downloading {
        release: Release,
        done: u64,
        total: Option<u64>,
    },
    Installing(Release),
    /// Installed; "Restart now" opens `relaunch`.
    Ready {
        release: Release,
        relaunch: PathBuf,
    },
    /// The last check or install failed; `release` can be retried.
    Failed {
        problem: Problem,
        release: Option<Release>,
    },
}

impl Status {
    /// Checking, installing, or waiting for the restart: no new check.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            Status::Checking
                | Status::Downloading { .. }
                | Status::Installing(_)
                | Status::Ready { .. }
        )
    }

    /// A release the user can install right now.
    pub fn installable(&self) -> Option<&Release> {
        match self {
            Status::Available(release)
            | Status::Failed {
                release: Some(release),
                ..
            } => Some(release),
            _ => None,
        }
    }
}

pub struct AppUpdate {
    pub status: Status,
    _job: Task<()>,
}

impl Global for AppUpdate {}

fn set_status(status: Status, cx: &mut App) {
    cx.update_global::<AppUpdate, _>(|update, _| update.status = status);
}

pub fn status(cx: &App) -> Status {
    let mut status = cx.global::<AppUpdate>().status.clone();
    if let Status::Failed { problem, .. } = &mut status {
        problem.message = problem.format(t(problem.key));
    }
    status
}

/// The title-bar pill: `(label, tooltip)` while an update waits for the user.
pub fn badge(cx: &App) -> Option<(&'static str, String)> {
    match &cx.try_global::<AppUpdate>()?.status {
        Status::Available(r) => Some((
            t("Update"),
            t("Osdy Code Desktop {version} is available").replace("{version}", &r.version),
        )),
        Status::Downloading { release: r, .. } | Status::Installing(r) => Some((
            t("Updating…"),
            t("Installing Osdy Code Desktop {version}").replace("{version}", &r.version),
        )),
        Status::Ready { release: r, .. } => Some((
            t("Restart to update"),
            t("Osdy Code Desktop {version} is installed").replace("{version}", &r.version),
        )),
        _ => None,
    }
}

/// Register the state and, for release builds, check in the background.
pub fn init(cx: &mut App) {
    cx.set_global(AppUpdate {
        status: Status::Idle,
        _job: Task::ready(()),
    });
    cx.background_executor()
        .spawn(async { remove_leftovers() })
        .detach();
    if !is_dev(CURRENT) {
        check(false, cx);
    }
}

/// Ask GitHub (or, on startup, the six-hour cache). Startup failures stay
/// quiet; a manual check reports them.
pub fn check(manual: bool, cx: &mut App) {
    if status(cx).busy() {
        return;
    }
    set_status(Status::Checking, cx);
    let job = cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { check_blocking(manual) })
            .await;
        let status = match result {
            Ok(Outcome::UpToDate) => Status::UpToDate,
            Ok(Outcome::Skipped(version)) => Status::Skipped(version),
            Ok(Outcome::Available(release)) => Status::Available(release),
            Err(problem) if manual => Status::Failed {
                problem,
                release: None,
            },
            Err(_) => Status::Idle,
        };
        cx.update(|cx| set_status(status, cx));
    });
    cx.global_mut::<AppUpdate>()._job = job;
}

/// Download & install the offered release; only ever from a user's click.
pub fn install(cx: &mut App) {
    let Some(release) = status(cx).installable().cloned() else {
        return;
    };
    let size = platform_asset(&release).map(|a| a.size).filter(|s| *s > 0);
    set_status(
        Status::Downloading {
            release: release.clone(),
            done: 0,
            total: size,
        },
        cx,
    );
    let (tx, rx) = async_channel::unbounded::<Phase>();
    let job = cx.spawn(async move |cx| {
        let work = cx.background_executor().spawn({
            let release = release.clone();
            async move {
                install_blocking(&release, &mut |phase| {
                    let _ = tx.try_send(phase);
                })
            }
        });
        while let Ok(phase) = rx.recv().await {
            let status = match phase {
                Phase::Downloading(done, total) => Status::Downloading {
                    release: release.clone(),
                    done,
                    total,
                },
                Phase::Installing => Status::Installing(release.clone()),
            };
            cx.update(|cx| set_status(status, cx));
        }
        let status = match work.await {
            Ok(relaunch) => Status::Ready { release, relaunch },
            Err(problem) => Status::Failed {
                problem,
                release: Some(release),
            },
        };
        cx.update(|cx| set_status(status, cx));
    });
    cx.global_mut::<AppUpdate>()._job = job;
}

/// "Skip this version": the startup check stays quiet until a newer one.
pub fn skip(cx: &mut App) {
    let Some(release) = status(cx).installable().cloned() else {
        return;
    };
    let status = match skip_blocking(&release.version) {
        Ok(()) => Status::Skipped(release.version),
        Err(e) => Status::Failed {
            problem: Problem::new("Couldn't remember the skip").cause(e),
            release: Some(release),
        },
    };
    set_status(status, cx);
}

/// Schedule the relaunch, then quit. Workspaces are saved by the
/// workspace's `on_app_quit` hook. Quit directly: `App::dispatch_action`
/// routes through the active window, and from a click handler that window
/// is already being updated, so the action would be dropped.
pub fn restart(cx: &mut App) {
    let Status::Ready { release, relaunch } = status(cx).clone() else {
        return;
    };
    match spawn_relaunch(&relaunch) {
        Ok(()) => cx.quit(),
        Err(e) => set_status(
            Status::Failed {
                problem: Problem::new("Osdy Code Desktop {version} is installed, but the restart couldn't be scheduled ({error}). Quit and reopen the app.")
                    .arg("{version}", &release.version)
                    .arg("{error}", e),
                release: None,
            },
            cx,
        ),
    }
}

/// Scratch directories, releases and bundles shared by the children's tests.
#[cfg(test)]
mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::cache::now_ms;
    use super::{BINARY, RELEASES_URL, Release};

    pub(super) fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gs-app-update-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(super) fn release(version: &str) -> Release {
        Release {
            version: version.into(),
            notes_url: format!("{RELEASES_URL}/tag/v{version}"),
            assets: Vec::new(),
        }
    }

    pub(super) fn fake_bundle(root: &Path, name: &str, body: &str) -> PathBuf {
        let bundle = root.join(name);
        let macos = bundle.join("Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        fs::write(macos.join(BINARY), body).unwrap();
        bundle
    }
}
