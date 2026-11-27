//! gentle-stream, the generative music player behind Gentle Shell's
//! `/gentle:music`: finding it, installing it into the app's own toolchain
//! the way Gentle Shell's bundled install does (a pinned uv, a uv-managed
//! Python 3.13 and the same pinned gentle-stream archive), its optional
//! sampled instruments, and removing what the app installed. The player
//! itself is driven by `features::music` through its `ctl` CLI.

use std::path::{Path, PathBuf};
use std::process::Command;

use gentle_core::backend::Backend;

use super::probe::run_logged;
use super::{Log, fetch, refresh_paths, toolchain_dir, user_home};
use crate::features::backend;
use crate::shared::i18n::t;

/// The gentle-stream commit the app installs, as a GitHub archive (no git
/// needed): the merge of `ctl --json` (gentle-stream#29), which the app needs
/// and Gentle Shell's own `/gentle:music` pin (as of 4.8.0) predates.
const ARCHIVE: &str = "https://github.com/Gentleman-Programming/gentle-stream/archive/af48631627ee20a0f9baa4d52e764bec6bb7290b.tar.gz";
const PYTHON: &str = "3.13";

/// gentle-stream needs POSIX (Unix sockets, `fcntl` locks).
pub const SUPPORTED: bool = cfg!(unix);

/// The player can run where agents run: this computer when it is Unix, or
/// a backend (Linux), whose audio WSLg carries to Windows.
pub fn supported() -> bool {
    SUPPORTED || super::remote::backend().is_some()
}

/// `player` ready to take its arguments: in the backend agents run in,
/// else here with the app's combined PATH.
pub fn player_command(player: &Path) -> Command {
    if let Some(choice) = super::remote::backend()
        && let Some(command) = backend::in_root(&choice, &[&player.to_string_lossy()])
    {
        return command;
    }
    let mut command = pi_rpc::command(player);
    command.env("PATH", pi_rpc::child_path());
    command
}

fn spec(samples: bool) -> String {
    format!(
        "gentle-stream{} @ {ARCHIVE}",
        if samples { "[samples]" } else { "" }
    )
}

/// `<toolchain>/music`: uv, its Python, the player and uv's cache.
fn music_dir() -> Option<PathBuf> {
    toolchain_dir().map(|toolchain| toolchain.join("music"))
}

fn app_player() -> Option<PathBuf> {
    music_dir()
        .map(|dir| dir.join("bin").join("gentle-stream"))
        .filter(|path| path.is_file())
}

/// The player: the app's own, the one on the PATH (`uv tool install`), or
/// the one Gentle Shell's bundled install put in `~/.gentle-shell/music`;
/// in the backend agents run in, when there is one.
pub fn player() -> Option<PathBuf> {
    if let Some(choice) = super::remote::backend() {
        return remote_player(&choice);
    }
    pick_player(
        app_player(),
        pi_rpc::find_program("gentle-stream"),
        user_home().join(".gentle-shell/music/bin/gentle-stream"),
    )
}

/// The first usable player, in preference order: the app's own, the one on
/// the PATH, then Gentle Shell's bundled install (when its file exists).
fn pick_player(app: Option<PathBuf>, path: Option<PathBuf>, bundled: PathBuf) -> Option<PathBuf> {
    if let Some(player) = app {
        return Some(player);
    }
    if let Some(player) = path {
        return Some(player);
    }
    bundled.is_file().then_some(bundled)
}

/// What Settings and the title bar need to know.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicStatus {
    pub player: Option<PathBuf>,
    /// The player answers `ctl … --json`, the only interface the app drives.
    /// Players installed before it (Gentle Shell's `/gentle:music` up to at
    /// least 4.8.0) have a plain-text `ctl` without it.
    pub json_ctl: bool,
    /// The player is the app's own, so Remove can delete it.
    pub app_owned: bool,
    /// `gentle-stream setup-samples` has put the soundfont in place.
    pub samples: bool,
}

impl MusicStatus {
    /// The player can be started and steered.
    pub fn usable(&self) -> bool {
        supported() && self.player.is_some() && self.json_ctl
    }
}

fn samples_installed() -> bool {
    let cache =
        gentle_core::music::cache_dir(std::env::var_os("XDG_CACHE_HOME").as_deref(), &user_home());
    cache.join("FluidR3_GM.sf2").is_file()
}

/// Whether `player` answers `ctl status --json` with a machine envelope (an
/// idle player answers `nothing_playing`, which counts). An older player
/// rejects `--json` on stderr and prints nothing parseable.
fn speaks_json_ctl(player: &Path) -> bool {
    use std::io::Read as _;
    use std::time::{Duration, Instant};
    let Ok(mut child) = player_command(player)
        .args(["ctl", "status", "--json"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    !matches!(
        gentle_core::music::parse_ctl(&stdout),
        Err(gentle_core::music::CtlError::Malformed)
    )
}

/// Blocking: looks for the player, whether it speaks `ctl --json`, and its
/// soundfont (where agents run).
pub fn status() -> MusicStatus {
    if let Some(choice) = super::remote::backend() {
        let player = remote_player(&choice);
        let app = backend::toolchain(&choice)
            .map(|t| PathBuf::from(format!("{t}/music/bin/gentle-stream")));
        return MusicStatus {
            json_ctl: player.as_deref().is_some_and(speaks_json_ctl),
            app_owned: player.is_some() && player == app,
            samples: backend::run(&choice, "sh", &["-c", REMOTE_SAMPLES]).is_some_and(|(ok, _)| ok),
            player,
        };
    }
    let player = if SUPPORTED { player() } else { None };
    MusicStatus {
        json_ctl: player.as_deref().is_some_and(speaks_json_ctl),
        app_owned: player.is_some() && player == app_player(),
        samples: samples_installed(),
        player,
    }
}

/// The player in a backend, in the same order as here.
const REMOTE_PLAYER: &str = r#"for c in "$1/music/bin/gentle-stream" "$(command -v gentle-stream 2>/dev/null)" "$HOME/.gentle-shell/music/bin/gentle-stream"; do if [ -n "$c" ] && [ -x "$c" ]; then echo "$c"; exit 0; fi; done; exit 1"#;

/// The soundfont in a backend's gentle-stream cache.
const REMOTE_SAMPLES: &str =
    r#"test -f "${XDG_CACHE_HOME:-$HOME/.cache}/gentle-stream/FluidR3_GM.sf2""#;

fn remote_player(choice: &Backend) -> Option<PathBuf> {
    let toolchain = backend::toolchain(choice)?;
    let (ok, out) = backend::run(choice, "sh", &["-c", REMOTE_PLAYER, "sh", &toolchain])?;
    let path = out.trim();
    (ok && path.starts_with('/')).then(|| PathBuf::from(path))
}

/// uv and gentle-stream in a backend, the way [`install`] does here: uv
/// for its machine is downloaded and verified here, copied there, and run
/// there with everything inside `~/.local/share/gentle-shell-gpui/music`.
fn remote_install(choice: &Backend, log: &Log) -> Result<(), String> {
    let label = choice.label();
    let reach = || t("Couldn't reach {label}.").replace("{label}", &label);
    let music = format!("{}/music", backend::toolchain(choice).ok_or_else(reach)?);
    let arch = backend::arch(choice)
        .ok_or_else(|| t("Couldn't tell {label}'s machine.").replace("{label}", &label))?;
    let here = toolchain_dir().ok_or_else(|| t("This system has no app data folder."))?;
    let uv_here = here.join(format!(".backend-uv-{}", std::process::id()));
    let fetched = fetch::download_uv(&uv_here, "linux", arch, log)
        .and_then(|()| backend::put_file(choice, &uv_here, &format!("{music}/uv"), true));
    let _ = std::fs::remove_file(&uv_here);
    fetched?;
    log(format!(
        "Installing Python {PYTHON} and gentle-stream in {label}; this can take a few minutes…"
    ));
    let envs = [
        format!("UV_TOOL_DIR={music}/tools"),
        format!("UV_TOOL_BIN_DIR={music}/bin"),
        format!("UV_PYTHON_INSTALL_DIR={music}/python"),
        format!("UV_PYTHON_BIN_DIR={music}/python/bin"),
        format!("UV_CACHE_DIR={music}/cache"),
        format!("UV_PYTHON={PYTHON}"),
        "UV_PYTHON_PREFERENCE=only-managed".to_string(),
        "UV_PYTHON_DOWNLOADS=automatic".to_string(),
        "UV_NO_CONFIG=1".to_string(),
    ];
    let uv = format!("{music}/uv");
    let uv_command = |args: &[&str]| {
        let mut argv: Vec<&str> = vec!["env", "-u", "VIRTUAL_ENV", "-u", "CONDA_PREFIX"];
        argv.extend(envs.iter().map(String::as_str));
        argv.push(&uv);
        argv.extend(args);
        backend::in_root(choice, &argv).ok_or_else(reach)
    };
    let install = |samples: bool| {
        let spec = spec(samples);
        run_logged(
            uv_command(&["tool", "install", "--force", "--python", PYTHON, &spec])?,
            "uv tool install",
            log,
        )
    };
    if install(true).is_err() {
        log(
            "The sampled instruments need the system FluidSynth library; installing without them…"
                .into(),
        );
        install(false)?;
    }
    let _ = run_logged(uv_command(&["cache", "clean"])?, "uv cache clean", &|_| {});
    if remote_player(choice).is_none() {
        return Err(
            t("uv finished, but gentle-stream isn't in {music}/bin.").replace("{music}", &music)
        );
    }
    log("Done. The ♪ button in the title bar plays music.".into());
    Ok(())
}

/// uv's environment for the app's music folder: tools, their executables,
/// the managed Python and the cache all inside it; no system Python, no
/// uv.toml. Inherited `UV_*`, `PYTHON*`, `PIP_*`, `VIRTUAL_ENV` and
/// `CONDA_PREFIX` are dropped, as Gentle Shell's installer does.
fn uv_command(uv: &Path, music: &Path) -> Command {
    let mut command = pi_rpc::command(uv);
    for (key, _) in std::env::vars_os() {
        let upper = key.to_string_lossy().to_uppercase();
        if upper.starts_with("UV_")
            || upper.starts_with("PYTHON")
            || upper.starts_with("PIP_")
            || upper == "VIRTUAL_ENV"
            || upper == "CONDA_PREFIX"
        {
            command.env_remove(&key);
        }
    }
    command
        .current_dir(music)
        .env("UV_TOOL_DIR", music.join("tools"))
        .env("UV_TOOL_BIN_DIR", music.join("bin"))
        .env("UV_PYTHON_INSTALL_DIR", music.join("python"))
        .env("UV_PYTHON_BIN_DIR", music.join("python").join("bin"))
        .env("UV_CACHE_DIR", music.join("cache"))
        .env("UV_PYTHON", PYTHON)
        .env("UV_PYTHON_PREFERENCE", "only-managed")
        .env("UV_PYTHON_DOWNLOADS", "automatic")
        .env("UV_NO_CONFIG", "1");
    command
}

/// uv, Python 3.13 and gentle-stream into `<toolchain>/music`. The sampled
/// instruments' Python binding needs the system FluidSynth library; without
/// it, gentle-stream installs without them (its instruments are synthesized).
pub fn install(log: &Log) -> Result<(), String> {
    if let Some(choice) = super::remote::backend() {
        return remote_install(&choice, log);
    }
    if !SUPPORTED {
        return Err(t("gentle-stream runs on macOS and Linux only.").into());
    }
    let music = music_dir().ok_or_else(|| t("This system has no app data folder."))?;
    std::fs::create_dir_all(&music).map_err(|e| {
        gentle_core::i18n::fill(
            t("Couldn't create {path}: {error}"),
            &[
                ("path", &music.to_string_lossy()),
                ("error", &e.to_string()),
            ],
        )
    })?;
    let uv = music.join("uv");
    fetch::install_uv(&uv, log)?;
    log(format!(
        "Installing Python {PYTHON} and gentle-stream; this can take a few minutes…"
    ));
    let install = |samples: bool| {
        let mut command = uv_command(&uv, &music);
        command.args([
            "tool",
            "install",
            "--force",
            "--python",
            PYTHON,
            &spec(samples),
        ]);
        run_logged(command, "uv tool install", log)
    };
    if install(true).is_err() {
        log(
            "The sampled instruments need the system FluidSynth library; installing without them…"
                .into(),
        );
        install(false)?;
    }
    let mut clean = uv_command(&uv, &music);
    clean.args(["cache", "clean"]);
    let _ = run_logged(clean, "uv cache clean", &|_| {});
    refresh_paths();
    if app_player().is_none() {
        return Err(t("uv finished, but gentle-stream isn't where it should be.").into());
    }
    log("Done. The ♪ button in the title bar plays music.".into());
    Ok(())
}

/// `gentle-stream setup-samples`: the FluidR3_GM soundfont (about 120 MB,
/// once) for sampled instruments.
pub fn install_samples(log: &Log) -> Result<(), String> {
    let player = player().ok_or_else(|| t("Install gentle-stream first."))?;
    log("Downloading the FluidR3_GM soundfont (about 120 MB)…".into());
    let mut command = player_command(&player);
    command.arg("setup-samples");
    run_logged(command, "gentle-stream setup-samples", log)?;
    log("Sampled instruments are in place; the next piece uses them.".into());
    Ok(())
}

/// Stop the music, then delete what the app installed. A gentle-stream you
/// installed yourself is left alone.
pub fn remove(log: &Log) -> Result<(), String> {
    if let Some(choice) = super::remote::backend() {
        return remote_remove(&choice, log);
    }
    if let Some(player) = app_player() {
        log("Stopping the music…".into());
        let mut stop = pi_rpc::command(&player);
        stop.args(["ctl", "stop"]);
        let _ = run_logged(stop, "gentle-stream ctl stop", &|_| {});
    }
    let music = music_dir().ok_or_else(|| t("This system has no app data folder."))?;
    if music.exists() {
        log(format!("Deleting {}…", music.display()));
        std::fs::remove_dir_all(&music).map_err(|e| {
            gentle_core::i18n::fill(
                t("Couldn't delete {path}: {error}"),
                &[
                    ("path", &music.to_string_lossy()),
                    ("error", &e.to_string()),
                ],
            )
        })?;
    }
    refresh_paths();
    Ok(())
}

/// [`remove`] in a backend: what the app installed under
/// `~/.local/share/gentle-shell-gpui/music` there.
fn remote_remove(choice: &Backend, log: &Log) -> Result<(), String> {
    let music = format!(
        "{}/music",
        backend::toolchain(choice)
            .ok_or_else(|| t("Couldn't reach {label}.").replace("{label}", &choice.label()))?
    );
    let player = format!("{music}/bin/gentle-stream");
    log("Stopping the music…".into());
    if let Some(stop) = backend::in_root(choice, &[&player, "ctl", "stop"]) {
        let _ = run_logged(stop, "gentle-stream ctl stop", &|_| {});
    }
    log(format!("Deleting {music} in {}…", choice.label()));
    match backend::run(choice, "rm", &["-rf", &music]) {
        Some((true, _)) => Ok(()),
        _ => Err(gentle_core::i18n::fill(
            t("Couldn't delete {path} in {label}."),
            &[("path", &music), ("label", &choice.label())],
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_first_player_in_preference_order() {
        let app = PathBuf::from("/tc/music/bin/gentle-stream");
        let path = PathBuf::from("/usr/local/bin/gentle-stream");
        let bundled =
            std::env::temp_dir().join(format!("gentle-stream-pick-{}", std::process::id()));

        // The app's own player wins over the PATH and the bundled install.
        assert_eq!(
            pick_player(Some(app.clone()), Some(path.clone()), bundled.clone()),
            Some(app)
        );
        // The PATH wins when there is no app player.
        assert_eq!(
            pick_player(None, Some(path.clone()), bundled.clone()),
            Some(path)
        );
        // A bundled install that isn't there is not a player.
        assert_eq!(pick_player(None, None, bundled), None);
    }

    #[test]
    fn accepts_the_bundled_player_when_its_file_exists() {
        let dir =
            std::env::temp_dir().join(format!("gentle-stream-pick-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bundled = dir.join("gentle-stream");
        std::fs::write(&bundled, b"#!/bin/sh\n").unwrap();
        assert_eq!(pick_player(None, None, bundled.clone()), Some(bundled));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
