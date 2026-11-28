//! `osdy-gentle`, the Osdy-Pi Codex accounts and profiles package
//! (`/osdy-profile`, `/osdy-account`, the Codex account chip), shipped with
//! the app and installed into Gentle Shell once, on the first start that
//! finds Gentle Shell ready.
//!
//! The package is written to the app's data folder on every start, so an app
//! update brings its new version along: Pi loads a local package from its
//! folder without copying it. Installing it is `gentle-shell install <folder>`,
//! the same command Settings › Extensions runs, so it lands in Gentle Shell's
//! own `packages` list (shared with the terminal) and is removed the same way.
//!
//! Only once: after the install, or when the list already holds the package
//! (a local folder whose `package.json` names it, `npm:osdy-gentle`, or a git
//! URL whose repository is named `osdy-gentle`: a git source is recognized by
//! its repository name only, since its package name needs a clone), the app
//! records it in
//! its settings (`osdyGentle`) and never installs it again, so a removal
//! sticks. Only for the local backend (a WSL distro or a VM can't load a
//! folder on this machine) and never from an isolated copy
//! (`GENTLE_SHELL_GPUI_DATA_DIR`), whose data folder is disposable. A failure is logged and retried on the next start.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The package name in its `package.json`.
const NAME: &str = "osdy-gentle";
/// The key in the app's `settings.json` recording that it was installed.
const KEY: &str = "osdyGentle";

/// The package's files, relative to its folder. Vendored from the
/// `osdy-gentle` repository; update them together.
const FILES: &[(&str, &[u8])] = &[
    ("package.json", include_bytes!("package/package.json")),
    ("README.md", include_bytes!("package/README.md")),
    (
        "assets/osdy-notification.wav",
        include_bytes!("package/assets/osdy-notification.wav"),
    ),
    (
        "src/account-store.ts",
        include_bytes!("package/src/account-store.ts"),
    ),
    ("src/bindings.ts", include_bytes!("package/src/bindings.ts")),
    (
        "src/codex-reset-store.ts",
        include_bytes!("package/src/codex-reset-store.ts"),
    ),
    (
        "src/codex-reset-transport.ts",
        include_bytes!("package/src/codex-reset-transport.ts"),
    ),
    (
        "src/codex-reset.ts",
        include_bytes!("package/src/codex-reset.ts"),
    ),
    (
        "src/codex-usage.ts",
        include_bytes!("package/src/codex-usage.ts"),
    ),
    ("src/commands.ts", include_bytes!("package/src/commands.ts")),
    (
        "src/gentle-profiles.ts",
        include_bytes!("package/src/gentle-profiles.ts"),
    ),
    ("src/index.ts", include_bytes!("package/src/index.ts")),
    (
        "src/pi-auth-backend.ts",
        include_bytes!("package/src/pi-auth-backend.ts"),
    ),
    (
        "src/pi-runtime-auth.ts",
        include_bytes!("package/src/pi-runtime-auth.ts"),
    ),
    (
        "src/profile-widget.ts",
        include_bytes!("package/src/profile-widget.ts"),
    ),
];

/// In the background: write the package to the app's data folder and, the
/// first time, install it into Gentle Shell.
pub fn install_by_default() {
    let spawned = std::thread::Builder::new()
        .name("osdy-gentle".into())
        .spawn(|| {
            if let Err(err) = ensure_installed() {
                eprintln!("gentle-shell-gpui: {NAME} was not installed: {err}");
            }
        });
    if let Err(err) = spawned {
        eprintln!("gentle-shell-gpui: could not start the {NAME} install: {err}");
    }
}

fn ensure_installed() -> Result<(), String> {
    let data = crate::shared::data_dir::app().ok_or("the system has no data folder")?;
    let dir = data.join("packages").join(NAME);
    write_package(&dir).map_err(|e| format!("writing {}: {e}", dir.display()))?;

    let store = crate::shared::themes::store_path().ok_or("no app settings file")?;
    if installed_before(&store) {
        return Ok(());
    }
    if crate::features::backend::saved() != gentle_core::backend::Backend::Local {
        return Ok(());
    }
    // An isolated copy (`GENTLE_SHELL_GPUI_DATA_DIR`) still runs the real
    // Gentle Shell: installing would list this disposable folder in the
    // user's real `packages`. Its agents load the package only if it is
    // already installed.
    if crate::shared::data_dir::overridden() {
        return Ok(());
    }
    // `--version` names the agent folder; no launcher yet (first run,
    // before onboarding) means try again next start.
    let agent = crate::features::setup::launcher_home().ok_or("Gentle Shell isn't ready yet")?;
    let agent = PathBuf::from(agent);
    if !listed(&agent) {
        let home = dirs::home_dir().ok_or("no home folder")?;
        let folder = dir.to_string_lossy();
        crate::features::setup::run_gentle_shell(&home, &["install", &folder])?;
        eprintln!("gentle-shell-gpui: installed {NAME} from {folder}");
    }
    crate::shared::themes::write_key(
        &store,
        KEY,
        json!({ "installed": true, "version": env!("CARGO_PKG_VERSION") }),
    )
    .map_err(|e| e.to_string())
}

/// Write the package's files under `dir`, leaving unchanged ones alone.
fn write_package(dir: &Path) -> std::io::Result<()> {
    for (name, bytes) in FILES {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(*bytes) {
            std::fs::create_dir_all(path.parent().unwrap_or(dir))?;
            std::fs::write(&path, bytes)?;
        }
    }
    Ok(())
}

/// Whether the app recorded installing the package already.
fn installed_before(store: &Path) -> bool {
    read_json(store).is_some_and(|settings| settings[KEY]["installed"] == true)
}

/// Whether Gentle Shell's `packages` list (in `<agent>/settings.json`)
/// already holds the package under any source.
fn listed(agent: &Path) -> bool {
    let Some(settings) = read_json(&agent.join("settings.json")) else {
        return false;
    };
    settings["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().or(p["source"].as_str()))
        .any(|source| {
            let local = local_name(agent, source);
            gentle_core::default_package::names_package(source, local.as_deref(), NAME)
        })
}

/// The `name` in the `package.json` of a local folder `source` (relative
/// paths resolve from the settings file's folder, as Pi resolves them).
fn local_name(agent: &Path, source: &str) -> Option<String> {
    let path = agent.join(source);
    read_json(&path.join("package.json"))?["name"]
        .as_str()
        .map(str::to_string)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh scratch folder for one test, removed when it ends.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(test: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("ocd-{NAME}-{test}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_package_is_written_whole_and_left_alone_when_unchanged() {
        let dir = Scratch::new("write");
        let pkg = dir.path().join(NAME);
        write_package(&pkg).unwrap();
        let manifest = read_json(&pkg.join("package.json")).unwrap();
        assert_eq!(manifest["name"], NAME);
        // Every extension entry the manifest declares ships with it.
        for entry in manifest["pi"]["extensions"].as_array().unwrap() {
            assert!(pkg.join(entry.as_str().unwrap()).is_file(), "{entry}");
        }
        let index = pkg.join("src/index.ts");
        let before = std::fs::metadata(&index).unwrap().modified().unwrap();
        write_package(&pkg).unwrap();
        assert_eq!(
            std::fs::metadata(&index).unwrap().modified().unwrap(),
            before
        );
    }

    #[test]
    fn a_checkout_listed_by_relative_path_counts_as_installed() {
        let home = Scratch::new("listed");
        let agent = home.path().join(".gentle-shell/agent");
        let checkout = home.path().join("Documents/GitHub/IngracionOsdy-Pi");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(checkout.join("package.json"), r#"{"name":"osdy-gentle"}"#).unwrap();
        let settings = agent.join("settings.json");

        std::fs::write(&settings, r#"{"packages":["npm:gentle-engram"]}"#).unwrap();
        assert!(!listed(&agent));
        std::fs::write(
            &settings,
            r#"{"packages":["npm:gentle-engram",{"source":"../../Documents/GitHub/IngracionOsdy-Pi"}]}"#,
        )
        .unwrap();
        assert!(listed(&agent));
    }

    #[test]
    fn a_native_absolute_checkout_path_with_at_uses_the_manifest_name() {
        let root = Scratch::new("absolute-at");
        let agent = root.path().join("agent");
        let checkout = root.path().join("Osdy@Work").join("custom-checkout");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(
            agent.join("settings.json"),
            json!({"packages": [{"source": checkout.to_str().unwrap()}]}).to_string(),
        )
        .unwrap();

        let manifest = checkout.join("package.json");
        std::fs::write(&manifest, r#"{"name":"osdy-gentle-extra"}"#).unwrap();
        assert!(!listed(&agent));
        std::fs::write(&manifest, r#"{"name":"osdy-gentle"}"#).unwrap();
        assert!(listed(&agent));
    }

    #[test]
    fn the_install_is_recorded_once_in_the_app_settings() {
        let dir = Scratch::new("record");
        let store = dir.path().join("settings.json");
        assert!(!installed_before(&store));
        crate::shared::themes::write_key(&store, KEY, json!({ "installed": true })).unwrap();
        assert!(installed_before(&store));
    }
}
