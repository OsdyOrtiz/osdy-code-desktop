//! Gentle Tower's web app, built here when Gentle Shell shipped without it.
//! gentle-pi 4.9.0's npm package left `web/tower/dist` out (its
//! `web/tower/.gitignore` ignores `dist/`, and npm honors nested ignore
//! files), so `gentle-shell tower` reports "the web app is not built". Until
//! a release ships it, this downloads the same version's source from GitHub,
//! builds the web app with the Node Gentle Shell runs on (pnpm through npx,
//! the package's own `build:tower` steps) and puts `dist` where the server
//! looks for it. Nothing else in the install changes; on macOS, Linux and
//! Windows alike.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::fetch::{Staging, download};
use super::probe::run_logged;
use super::{Log, toolchain_dir};
use crate::features::app_update::extract_tar_gz;
use crate::features::backend;
use crate::shared::i18n::t;
use gentle_core::backend::Backend;

const REPO_TARBALL: &str =
    "https://codeload.github.com/Gentleman-Programming/gentle-shell/tar.gz/refs/tags";
/// pnpm major the package builds Tower with (`packageManager`).
const PNPM: &str = "pnpm@10";

/// Where Tower stands in the Gentle Shell the app runs.
#[derive(Clone, Debug, PartialEq)]
pub struct TowerStatus {
    /// gentle-pi's package folder; `None` when it can't be found.
    pub package: Option<PathBuf>,
    pub version: Option<String>,
    /// `web/tower/dist/index.html` is there.
    pub built: bool,
}

/// Tower's built page in `package`.
fn index(package: &Path) -> PathBuf {
    package
        .join("web")
        .join("tower")
        .join("dist")
        .join("index.html")
}

/// The package folder a launch runs, by what it executes: a self-contained
/// install names it; otherwise the launcher is (a link to) the package's
/// `bin/gentle-shell.mjs`, or an npm shim beside `node_modules/gentle-pi`.
fn package_root(spec: &pi_rpc::LaunchSpec) -> Option<PathBuf> {
    let is_gentle_pi = |dir: &Path| {
        fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .is_some_and(|json| json["name"] == "gentle-pi")
    };
    if let Some(bundled) = &spec.bundled {
        return Some(bundled.package.clone());
    }
    let launcher = dunce::canonicalize(&spec.launcher).unwrap_or(spec.launcher.clone());
    let candidates = [
        launcher
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf),
        launcher
            .parent()
            .map(|dir| dir.join("node_modules").join("gentle-pi")),
        launcher
            .parent()
            .and_then(Path::parent)
            .map(|dir| dir.join("lib").join("node_modules").join("gentle-pi")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|dir| is_gentle_pi(dir))
}

fn version_of(package: &Path) -> Option<String> {
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(package.join("package.json")).ok()?).ok()?;
    json["version"].as_str().map(str::to_string)
}

/// Checks the Gentle Shell the app would run (in the backend agents run
/// in, when there is one).
pub fn status() -> TowerStatus {
    if let Some(choice) = super::remote::backend() {
        return remote_status(&choice).unwrap_or(TowerStatus {
            package: None,
            version: None,
            built: false,
        });
    }
    let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    let package = pi_rpc::resolve_backend(&home)
        .ok()
        .and_then(|spec| package_root(&spec));
    TowerStatus {
        version: package.as_deref().and_then(version_of),
        built: package.as_deref().is_some_and(|p| index(p).is_file()),
        package,
    }
}

/// gentle-pi's package folder behind `gentle-shell` in a backend (its npm
/// link, or the global `node_modules` beside it), its version and whether
/// Tower is built there.
const REMOTE_LOOK: &str = r#"g="$(command -v gentle-shell)" || exit 3; r="$(readlink -f "$g")"; for p in "$(dirname "$(dirname "$r")")" "$(dirname "$g")/../lib/node_modules/gentle-pi"; do if grep -q '"name": *"gentle-pi"' "$p/package.json" 2>/dev/null; then p="$(cd "$p" && pwd -P)"; echo "package=$p"; echo "version=$(node -p 'require(process.argv[1]).version' "$p/package.json")"; [ -f "$p/web/tower/dist/index.html" ] && echo built=yes; exit 0; fi; done; exit 4"#;

fn remote_status(choice: &Backend) -> Option<TowerStatus> {
    let (ok, out) = backend::run(choice, "sh", &["-c", REMOTE_LOOK])?;
    let field = |key: &str| out.lines().find_map(|line| line.strip_prefix(key));
    ok.then(|| TowerStatus {
        package: field("package=").map(PathBuf::from),
        version: field("version=").map(str::to_string),
        built: field("built=").is_some(),
    })
}

/// Builds Tower in a backend: the source downloaded here, copied there,
/// built with that backend's Node (pnpm through npx) and put in its
/// package, in one step.
const REMOTE_BUILD: &str = r#"set -e; d="$(mktemp -d)"; trap 'rm -rf "$d"' EXIT; tar -xzf "$1" -C "$d"; rm -f "$1"; cd "$d"/*/web/tower; npx -y pnpm@10 install --frozen-lockfile; npx -y pnpm@10 build; test -f dist/index.html; w="$2/web/tower"; mkdir -p "$w"; rm -rf "$w/.dist-$$"; cp -R dist "$w/.dist-$$"; rm -rf "$w/dist"; mv "$w/.dist-$$" "$w/dist""#;

fn remote_install(choice: &Backend, log: &Log) -> Result<(), String> {
    let label = choice.label();
    let found = remote_status(choice).ok_or_else(|| {
        t("Couldn't find Gentle Shell's package folder in {label}.").replace("{label}", &label)
    })?;
    if found.built {
        log("Gentle Tower is already there.".into());
        return Ok(());
    }
    let (Some(package), Some(version)) = (found.package, found.version) else {
        return Err(
            t("Couldn't read Gentle Shell's version in {label}.").replace("{label}", &label)
        );
    };
    let toolchain = toolchain_dir().ok_or_else(|| t("This system has no app data folder."))?;
    fs::create_dir_all(&toolchain).map_err(|e| e.to_string())?;
    let staging = Staging::new(&toolchain, "tower")?;
    log(format!(
        "Downloading Gentle Shell {version}'s source from GitHub…"
    ));
    let archive = staging.0.join("source.tar.gz");
    download(&format!("{REPO_TARBALL}/v{version}"), &archive, log)?;
    let there = format!(
        "{}/tower-source-{version}.tar.gz",
        backend::toolchain(choice)
            .ok_or_else(|| t("Couldn't reach {label}.").replace("{label}", &label))?
    );
    backend::put_file(choice, &archive, &there, false)?;
    log(format!("Building Gentle Tower in {label} (pnpm)…"));
    let package = package.to_string_lossy();
    let command = backend::in_root(choice, &["sh", "-c", REMOTE_BUILD, "sh", &there, &package])
        .ok_or_else(|| t("Couldn't reach {label}.").replace("{label}", &label))?;
    run_logged(command, "Gentle Tower's build", log)?;
    log(format!("Gentle Tower is ready in {package}/web/tower."));
    Ok(())
}

/// npm's `npx` script beside `node`: `lib/node_modules/npm` on macOS and
/// Linux, `node_modules/npm` next to `node.exe` on Windows.
fn npx_script(node: &Path) -> Option<PathBuf> {
    let dir = node.parent()?;
    [
        dir.join("node_modules")
            .join("npm")
            .join("bin")
            .join("npx-cli.js"),
        dir.parent()?
            .join("lib")
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npx-cli.js"),
    ]
    .into_iter()
    .find(|script| script.is_file())
}

/// `npx -y pnpm@10 <args>`, run by the Node Gentle Shell uses.
fn pnpm(node: &Path, args: &[&str], dir: &Path) -> Result<Command, String> {
    let mut command = match npx_script(node) {
        Some(script) => {
            let mut c = Command::new(node);
            c.arg(script);
            c
        }
        None => Command::new(
            pi_rpc::find_program("npx")
                .ok_or_else(|| t("npx was not found next to Node.js or on the PATH."))?,
        ),
    };
    command.arg("-y").arg(PNPM).args(args).current_dir(dir);
    // Its child processes (the build) find the same Node first.
    if let Some(bin) = node.parent() {
        let path = std::env::join_paths(
            std::iter::once(bin.to_path_buf()).chain(std::env::split_paths(&pi_rpc::child_path())),
        )
        .map_err(|e| e.to_string())?;
        command.env("PATH", path);
    }
    Ok(command)
}

/// Copies the folder `from` to `to` (which must not exist).
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Builds Tower's web app for the Gentle Shell the app runs and installs it
/// into its package. Does nothing when it's already there.
pub fn install(log: &Log) -> Result<(), String> {
    if let Some(choice) = super::remote::backend() {
        return remote_install(&choice, log);
    }
    let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    let spec = pi_rpc::resolve_backend(&home).map_err(|e| format!("{e:#}"))?;
    let package =
        package_root(&spec).ok_or_else(|| t("Couldn't find Gentle Shell's package folder."))?;
    if index(&package).is_file() {
        log("Gentle Tower is already there.".into());
        return Ok(());
    }
    let version = version_of(&package).ok_or_else(|| t("Couldn't read Gentle Shell's version."))?;
    let node = match &spec.bundled {
        Some(bundled) => bundled.node.clone(),
        None => pi_rpc::resolve_node(&home).map_err(|e| format!("{e:#}"))?,
    };
    let toolchain = toolchain_dir().ok_or_else(|| t("This system has no app data folder."))?;
    fs::create_dir_all(&toolchain).map_err(|e| e.to_string())?;
    let staging = Staging::new(&toolchain, "tower")?;

    log(format!(
        "Downloading Gentle Shell {version}'s source from GitHub…"
    ));
    let archive = staging.0.join("source.tar.gz");
    download(&format!("{REPO_TARBALL}/v{version}"), &archive, log)?;
    let source = staging.0.join("source");
    extract_tar_gz(&archive, &source)
        .map_err(|e| t("Couldn't unpack the source: {error}").replace("{error}", &e.to_string()))?;
    // GitHub's tarball has one top folder (`gentle-shell-<version>`).
    let tower = fs::read_dir(&source)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("web").join("tower"))
        .find(|dir| dir.join("package.json").is_file())
        .ok_or_else(|| {
            t("Gentle Shell {version}'s source has no web/tower.").replace("{version}", &version)
        })?;

    log("Installing the web app's dependencies (pnpm)…".into());
    run_logged(
        pnpm(&node, &["install", "--frozen-lockfile"], &tower)?,
        "pnpm install",
        log,
    )?;
    log("Building Gentle Tower…".into());
    run_logged(pnpm(&node, &["build"], &tower)?, "pnpm build", log)?;
    let built = tower.join("dist");
    if !built.join("index.html").is_file() {
        return Err(t("The build finished without dist/index.html.").into());
    }

    // Copied beside, then renamed into place: never half a dist.
    let web = package.join("web").join("tower");
    let partial = web.join(format!(".dist-{}", std::process::id()));
    let _ = fs::remove_dir_all(&partial);
    copy_dir(&built, &partial)
        .and_then(|()| fs::rename(&partial, web.join("dist")))
        .map_err(|e| {
            let _ = fs::remove_dir_all(&partial);
            gentle_core::i18n::fill(
                t("Couldn't write into {path}: {error}"),
                &[("path", &web.to_string_lossy()), ("error", &e.to_string())],
            )
        })?;
    log(format!("Gentle Tower is ready in {}.", web.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npx_is_found_where_each_system_installs_npm() {
        let root = std::env::temp_dir().join(format!("tower-npx-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // macOS and Linux: <prefix>/bin/node, <prefix>/lib/node_modules/npm.
        let unix = root.join("unix");
        let script = unix.join("lib/node_modules/npm/bin/npx-cli.js");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::write(&script, "").unwrap();
        assert_eq!(npx_script(&unix.join("bin").join("node")), Some(script));
        // Windows: <prefix>\node.exe, <prefix>\node_modules\npm.
        let win = root.join("win");
        let script = win.join("node_modules/npm/bin/npx-cli.js");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::write(&script, "").unwrap();
        assert_eq!(npx_script(&win.join("node.exe")), Some(script));
        assert_eq!(npx_script(&root.join("none").join("node")), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copies_nested_folders() {
        let root = std::env::temp_dir().join(format!("tower-copy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("from/assets")).unwrap();
        fs::write(root.join("from/index.html"), "i").unwrap();
        fs::write(root.join("from/assets/a.js"), "a").unwrap();
        copy_dir(&root.join("from"), &root.join("to")).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("to/assets/a.js")).unwrap(),
            "a"
        );
        assert!(root.join("to/index.html").is_file());
        let _ = fs::remove_dir_all(&root);
    }
}
