//! Setup's checks when agents run in a backend (a WSL distro, a VM, a
//! container): Node.js, Gentle Shell, its setup, a provider, git and the
//! optional pieces are looked for there, never on this computer. Fixes are
//! commands for a terminal, which opens in the backend too.

use gentle_core::backend::Backend;
use serde_json::Value;

use super::gentle::{self, BRIDGE, Marker};
use super::probe::{parse_version, show_version};
use super::{Check, CheckId, Detail, Fix, Log, MIN_NODE, Status, extras, fetch, meta};
use crate::features::backend;
use crate::shared::i18n::t;

/// The backend agents run in, when they don't run on this computer.
pub(super) fn backend() -> Option<Backend> {
    Some(backend::saved()).filter(|choice| *choice != Backend::Local)
}

/// What `engram version` prints in `choice`.
pub(super) fn engram_version_text(choice: &Backend) -> Option<String> {
    run(choice, "engram", &["version"])
}

/// Where Engram goes in a backend: over the one there when it can be
/// written, else a folder of the user's own on the PATH, else
/// `/usr/local/bin` when it can be written (root in a container), else
/// `~/.local/bin` (which Ubuntu's login profile adds once it exists).
const ENGRAM_TARGET: &str = r#"e="$(command -v engram 2>/dev/null)"; if [ -n "$e" ] && [ -w "$e" ]; then echo "$e"; exit 0; fi; for d in "$HOME/.local/bin" "$HOME/bin" "$HOME/go/bin"; do case ":$PATH:" in *":$d:"*) echo "$d/engram"; exit 0;; esac; done; if [ -w /usr/local/bin ]; then echo /usr/local/bin/engram; exit 0; fi; echo "$HOME/.local/bin/engram""#;

/// The latest Engram release for `choice`'s machine, downloaded and
/// verified here and copied there. The version it runs afterwards.
pub(super) fn install_engram(choice: &Backend, log: &Log) -> Result<String, String> {
    let label = choice.label();
    let arch = backend::arch(choice)
        .ok_or_else(|| t("Couldn't tell {label}'s machine.").replace("{label}", &label))?;
    let (_staging, found, version) = fetch::download_engram("linux", arch, log)?;
    let there = run(choice, "sh", &["-c", ENGRAM_TARGET])
        .map(|out| out.trim().to_string())
        .filter(|path| path.starts_with('/'))
        .ok_or_else(|| {
            t("Couldn't find a folder for Engram in {label}.").replace("{label}", &label)
        })?;
    log(format!("Copying Engram {version} to {there} in {label}…"));
    backend::put_file(choice, &found, &there, true)?;
    backend::forget_paths();
    let text = engram_version_text(choice).ok_or_else(|| {
        t("Engram is in {path}, but it doesn't run there: is that folder on the PATH?")
            .replace("{path}", &there)
    })?;
    extras::engram_version_of(&text).ok_or_else(|| {
        gentle_core::i18n::fill(
            t("Engram is in {path}, but `engram version` says: {output}"),
            &[("path", &there), ("output", &text)],
        )
    })
}

/// Node.js 22 through nvm, which needs only curl and no admin rights.
const INSTALL_NODE: &str = r#"curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash && . "$HOME/.nvm/nvm.sh" && nvm install 22"#;

/// `npm install -g <package>`, into `~/.local` when npm's global prefix
/// isn't the user's (a distro's Node puts it in root-owned `/usr/local`;
/// Ubuntu's login profile puts `~/.local/bin` on the PATH).
fn npm_global(package: &str) -> String {
    format!(
        r#"p="$(npm prefix -g)"; if [ -w "$p/lib/node_modules" ] || [ -w "$p/lib" ]; then npm install -g {package}; else npm install -g --prefix "$HOME/.local" {package}; fi"#
    )
}

/// Blocking: every check, run in `choice`.
pub(super) fn run_checks(choice: &Backend) -> Vec<Check> {
    // What the user installed in a terminal there since the last look.
    backend::forget_paths();
    let label = choice.label();
    let launcher = gentle::launcher();
    let shell_path = backend::gentle_shell_path(choice);
    let version = launcher.as_ref().ok().and_then(|l| l.version.clone());
    let home = version
        .as_ref()
        .and_then(|v| v.home_dir.as_ref())
        .map(|dir| dir.to_string_lossy().into_owned());
    backend::use_home(choice, home.as_deref());
    let read_json = |path: &str| {
        backend::read(choice, path).and_then(|text| serde_json::from_str::<Value>(&text).ok())
    };
    let agent_file = |name: &str| {
        home.as_deref()
            .and_then(|h| read_json(&format!("{h}/{name}")))
    };
    let settings = agent_file("settings.json").unwrap_or(Value::Null);
    let bridge = gentle::settings_list(&settings, BRIDGE).then(|| {
        agent_file(&format!("npm/node_modules/{BRIDGE}/package.json"))
            .map(|p| p["version"].as_str().unwrap_or("?").to_string())
    });
    let claude = run(choice, "claude", &["auth", "status"])
        .map(|out| serde_json::from_str::<Value>(out.trim()).is_ok_and(|v| v["loggedIn"] == true));
    let ids = [
        CheckId::Node,
        CheckId::GentleShell,
        CheckId::Setup,
        CheckId::Provider,
        CheckId::Git,
        CheckId::Engram,
    ];
    ids.into_iter()
        .map(|id| {
            let (status, fix) = match id {
                CheckId::Node => node(choice, &label),
                CheckId::GentleShell => match (&version, &shell_path) {
                    // WSL appends Windows' PATH: a Windows install answers, but
                    // it isn't the distro's own.
                    (_, Some(path)) if gentle_core::backend::is_windows_file(path) => (
                        Status::Missing(Detail::template(
                            "Only Windows' gentle-shell is visible from {label} ({path})",
                            [("{label}", label.clone()), ("{path}", path.clone())],
                        )),
                        terminal(&npm_global("gentle-pi")),
                    ),
                    (Some(_), _) => match gentle::check_gentle_shell(&launcher) {
                        (Status::Ok(found), _) => (Status::Ok(found), Fix::None),
                        (status, _) => (status, terminal("gentle-shell upgrade")),
                    },
                    (None, _) => (
                        Status::Missing(Detail::template(
                            "gentle-shell isn't installed in {label}",
                            [("{label}", label.clone())],
                        )),
                        terminal(&npm_global("gentle-pi")),
                    ),
                },
                CheckId::Setup => setup(choice, version.as_ref(), home.as_deref(), &settings),
                CheckId::Provider => {
                    let mut found = keys(agent_file("auth.json"), None);
                    found.extend(keys(agent_file("models.json"), Some("providers")));
                    if !found.is_empty() {
                        (
                            Status::Ok(Detail::template(
                                "Signed in: {providers}",
                                [("{providers}", found.join(", "))],
                            )),
                            Fix::None,
                        )
                    } else if bridge.as_ref().is_some_and(Option::is_some) && claude == Some(true) {
                        (
                            Status::Ok("Claude through your Claude Code sign-in".into()),
                            Fix::None,
                        )
                    } else {
                        (
                            Status::Missing(
                                "No provider signed in: run /login in Gentle Shell".into(),
                            ),
                            terminal("gentle-shell"),
                        )
                    }
                }
                CheckId::Git => match run(choice, "git", &["--version"]) {
                    Some(text) => (
                        Status::Ok(
                            format!("Git {}", text.trim().trim_start_matches("git version "))
                                .into(),
                        ),
                        Fix::None,
                    ),
                    None => (
                        Status::Missing(Detail::template(
                            "Install git in {label}",
                            [("{label}", label.clone())],
                        )),
                        Fix::None,
                    ),
                },
                CheckId::Engram => match engram_version_text(choice) {
                    Some(text) => extras::engram_status(&text),
                    None => (
                        Status::Missing(Detail::template(
                            "Not installed in {label}",
                            [("{label}", label.clone())],
                        )),
                        Fix::Auto,
                    ),
                },
                // Go builds Gentle AI on Windows itself; a backend's setup brings its own.
                CheckId::Go => (Status::Ok("Not needed".into()), Fix::None),
            };
            let (title, summary, need) = meta(id);
            Check {
                id,
                title,
                summary,
                need,
                status,
                fix,
            }
        })
        .collect()
}

/// `program args` in `choice`: its output when it succeeded.
fn run(choice: &Backend, program: &str, args: &[&str]) -> Option<String> {
    backend::run(choice, program, args)
        .filter(|(ok, _)| *ok)
        .map(|(_, out)| out)
}

fn terminal(command: &str) -> Fix {
    Fix::Terminal(command.to_string())
}

fn node(choice: &Backend, label: &str) -> (Status, Fix) {
    let Some(text) = run(choice, "node", &["--version"]) else {
        return (
            Status::Missing(Detail::template(
                "Node.js isn't installed in {label}",
                [("{label}", label.to_string())],
            )),
            terminal(INSTALL_NODE),
        );
    };
    match parse_version(&text) {
        Some(version) if version >= MIN_NODE => match run(choice, "npm", &["--version"]) {
            Some(npm) => (
                Status::Ok(format!("Node {} · npm {}", show_version(version), npm.trim()).into()),
                Fix::None,
            ),
            None => (
                Status::Missing(Detail::template(
                    "Node {version} found, but not npm",
                    [("{version}", show_version(version))],
                )),
                terminal(INSTALL_NODE),
            ),
        },
        Some(version) => (
            Status::Outdated(Detail::template(
                "Node {version} — needs {minimum} or newer",
                [
                    ("{version}", show_version(version)),
                    ("{minimum}", show_version(MIN_NODE)),
                ],
            )),
            terminal(INSTALL_NODE),
        ),
        None => (
            Status::Failed(Detail::template(
                "Unexpected `node --version`: {output}",
                [("{output}", text.trim().to_string())],
            )),
            terminal(INSTALL_NODE),
        ),
    }
}

/// The launcher's setup marker in the backend's `~/.gentle-shell/config.json`.
fn setup(
    choice: &Backend,
    version: Option<&gentle::LauncherVersion>,
    home: Option<&str>,
    settings: &Value,
) -> (Status, Fix) {
    let (Some(version), Some(home)) = (version, home) else {
        return (
            Status::Missing("Needs a working Gentle Shell first".into()),
            Fix::None,
        );
    };
    if version.home_mode.as_deref() == Some("link") {
        return if gentle::settings_list(settings, "gentle-engram") {
            (
                Status::Ok(Detail::template(
                    "Linked to {home}",
                    [("{home}", home.to_string())],
                )),
                Fix::None,
            )
        } else {
            (
                Status::Missing(Detail::template(
                    "{home} isn't set up yet",
                    [("{home}", home.to_string())],
                )),
                terminal("gentle-shell setup"),
            )
        };
    }
    let config = run(
        choice,
        "sh",
        &[
            "-c",
            r#"cat "${GENTLE_SHELL_CONFIG:-$HOME/.gentle-shell/config.json}""#,
        ],
    )
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    .unwrap_or(Value::Null);
    match gentle::marker_state(&config, home, None, &version.gentle_shell) {
        Marker::Match { at } => (
            Status::Ok(Detail::template(
                "Set up {date}",
                [("{date}", at.get(..10).unwrap_or(&at).to_string())],
            )),
            Fix::None,
        ),
        Marker::Missing => (
            Status::Missing("Not set up yet — the first chat would do it silently".into()),
            terminal("gentle-shell setup"),
        ),
        Marker::StalePin { recorded } => (
            Status::Outdated(Detail::template(
                "Set up with Gentle AI {recorded}",
                [("{recorded}", recorded)],
            )),
            terminal("gentle-shell setup"),
        ),
        Marker::StaleGentlePi { recorded } => (
            Status::Outdated(gentle::stale_launcher_setup(
                recorded.as_deref(),
                &version.gentle_shell,
            )),
            terminal("gentle-shell setup"),
        ),
    }
}

/// The keys of a JSON object (`field` of it, when given).
fn keys(value: Option<Value>, field: Option<&str>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let value = field.map_or(&value, |f| &value[f]);
    value
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}
