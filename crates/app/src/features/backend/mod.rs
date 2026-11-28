//! Where agents run (`gentle_core::backend`): the choice saved in the app's
//! `settings.json`, the WSL distro a project folder implies, that
//! environment's login PATH and Gentle Shell homes, the WSL distros on this
//! computer, and what runs through it: `gentle-shell` (agent tabs and its
//! commands), terminals and `git`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::Duration;

use anyhow::{Result, anyhow};
use gentle_core::backend::{self, Backend, Distro};
use gentle_core::i18n::fill;
use gpui_kit::App;
use serde_json::Value;

use crate::shared::i18n::{AGENT_LOCALE_ENV, agent_locale_path, language, t};

/// How long the backend may take to answer a probe: a stopped WSL distro
/// or VM boots first.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// The saved choice, read once: `git` asks for it on every refresh.
static SAVED: RwLock<Option<Backend>> = RwLock::new(None);

/// Read the backend's homes, if agents run in one, in the background.
pub fn init(cx: &mut App) {
    cx.background_executor()
        .spawn(async { sync_homes() })
        .detach();
}

/// The backend new agent tabs use; [`Backend::Local`] when none is saved.
pub fn saved() -> Backend {
    if let Some(known) = SAVED.read().ok().and_then(|saved| saved.clone()) {
        return known;
    }
    let choice = crate::shared::themes::store_path()
        .and_then(|path| crate::shared::themes::read_key(&path, backend::KEY))
        .map_or_else(Backend::default, |value| Backend::from_json(&value));
    if let Ok(mut saved) = SAVED.write() {
        *saved = Some(choice.clone());
    }
    choice
}

/// Save the backend new agent tabs use (`null` for this computer).
pub fn save(choice: &Backend) -> std::io::Result<()> {
    let path = crate::shared::themes::store_path()
        .ok_or_else(|| std::io::Error::other(t("no data folder")))?;
    crate::shared::themes::write_key(&path, backend::KEY, choice.to_json().unwrap_or(Value::Null))?;
    if let Ok(mut saved) = SAVED.write() {
        *saved = Some(choice.clone());
    }
    Ok(())
}

/// Where work rooted at `cwd` runs: the distro of a folder under
/// `\\wsl.localhost\<distro>\`, else the saved choice.
pub fn for_tab(cwd: &Path) -> Backend {
    Backend::for_folder(&cwd.to_string_lossy()).unwrap_or_else(saved)
}

/// How `choice` reads `path`, a path on this computer, or why it can't.
fn reach(choice: &Backend, path: &Path) -> Result<String> {
    choice.path(&path.to_string_lossy()).ok_or_else(|| {
        anyhow!(fill(
            t("{backend} can't reach {path}. Open the project from a folder it can reach."),
            &[
                ("backend", &choice.label_in(language())),
                ("path", &path.to_string_lossy())
            ],
        ))
    })
}

/// The launch of an agent tab's `gentle-shell --mode rpc` in `cwd` on
/// `choice`, with `env` set and each `files` variable naming a host file as
/// the backend reads it. Blocking: a backend's first launch asks for its
/// login PATH.
pub fn launch_spec(
    choice: &Backend,
    cwd: &Path,
    env: &[(String, String)],
    files: &[(String, &Path)],
) -> Result<pi_rpc::LaunchSpec> {
    let folder = reach(choice, cwd)?;
    let locale = agent_locale_path();
    let Some(launch) = choice.launch(&folder) else {
        let mut spec = pi_rpc::resolve_backend(cwd)?;
        spec.env
            .extend(env.iter().map(|(k, v)| (k.clone(), v.into())));
        spec.env.extend(
            files
                .iter()
                .map(|(k, p)| (k.clone(), p.as_os_str().to_owned())),
        );
        if let Some(locale) = locale {
            spec.env
                .push((AGENT_LOCALE_ENV.into(), locale.into_os_string()));
        }
        return Ok(spec);
    };
    // Files the agent shares with the app while it runs (its canvas) are read
    // in place; the backend must see them.
    let mut all = env.to_vec();
    for (key, file) in files {
        all.push((key.clone(), reach(choice, file)?));
    }
    // WSL can translate shared host paths; an arbitrary command prefix
    // provides no guarantee that the local app directory is mounted there.
    if !matches!(choice, Backend::Command { .. })
        && let Some(locale) = locale
        && let Ok(path) = reach(choice, &locale)
    {
        all.push((AGENT_LOCALE_ENV.into(), path));
    }
    let path = login_path(choice);
    // WSL appends Windows' PATH: an agent there must not start Windows'
    // install through interop (its packages load over 9P, minutes slower).
    if let Some(found) = own_shell(choice)
        && backend::is_windows_file(&found)
    {
        return Err(anyhow!(fill(
            t(
                "Only Windows' gentle-shell is visible from {backend} ({path}). Install it there: \
                               Setup & requirements has the command."
            ),
            &[("backend", &choice.label_in(language())), ("path", &found)],
        )));
    }
    pi_rpc::remote_backend(&launch, path.as_deref(), &all, |file| stage(choice, file))
}

/// Where `gentle-shell` is in each backend, for tab starts.
static SHELLS: LazyLock<Mutex<HashMap<Backend, Option<String>>>> = LazyLock::new(Mutex::default);

/// Where `gentle-shell` is in `choice`, asked once per backend for tab
/// starts ([`forget_paths`] asks again).
fn own_shell(choice: &Backend) -> Option<String> {
    if let Some(known) = SHELLS.lock().ok()?.get(choice) {
        return known.clone();
    }
    let found = gentle_shell_path(choice);
    SHELLS.lock().ok()?.insert(choice.clone(), found.clone());
    found
}

/// A file the app gives every agent (its instructions, its extensions),
/// copied into `choice`, which may not see this computer's files (a
/// container): its path there, `~/.cache/osdy-code-desktop/<hash>-<name>`.
/// Named by content, so app versions side by side never mix; copied once.
fn stage(choice: &Backend, file: &Path) -> Option<String> {
    static STAGED: LazyLock<Mutex<HashSet<(Backend, String)>>> = LazyLock::new(Mutex::default);
    let bytes = std::fs::read(file).ok()?;
    let hash = {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    };
    let home = remote_home(choice)?;
    let name = file.file_name()?.to_string_lossy();
    let there = format!("{home}/.cache/osdy-code-desktop/{hash:016x}-{name}");
    let key = (choice.clone(), there.clone());
    if STAGED.lock().ok()?.contains(&key) {
        return Some(there);
    }
    put_file(choice, file, &there, false).ok()?;
    STAGED.lock().ok()?.insert(key);
    Some(there)
}

/// How long a copy into the backend may take (Engram, uv, Tower's source:
/// tens of MB); a backend that hangs fails the step instead of holding it.
const COPY_TIMEOUT: Duration = Duration::from_secs(300);

/// Copy `file` from this computer to `there` (absolute) in `choice`,
/// executable when `executable`. Written beside and moved in, so whatever
/// reads it meanwhile never sees half. Blocking, within [`COPY_TIMEOUT`].
pub fn put_file(
    choice: &Backend,
    file: &Path,
    there: &str,
    executable: bool,
) -> Result<(), String> {
    let mode = if executable { "chmod 755" } else { "true" };
    let script =
        format!(r#"mkdir -p "${{1%/*}}" && cat > "$1.$$" && {mode} "$1.$$" && mv -f "$1.$$" "$1""#);
    let mut command = in_root(choice, &["sh", "-c", &script, "sh", there])
        .ok_or_else(|| t("Agents run on this computer."))?;
    command
        .stdin(std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?)
        .stderr(std::process::Stdio::null());
    match pi_rpc::output_within(&mut command, COPY_TIMEOUT) {
        Some((true, _)) => Ok(()),
        Some((false, _)) => Err(fill(
            t("Couldn't write {path} on {backend}."),
            &[("path", there), ("backend", &choice.label_in(language()))],
        )),
        None => Err(fill(
            t("{backend} didn't answer while copying {path}."),
            &[("backend", &choice.label_in(language())), ("path", there)],
        )),
    }
}

/// Copy the folder `dir` from this computer to `there` (absolute) in
/// `choice`, replacing what was there in one step, as a tar stream: only
/// `tar` is needed there. Unix file modes travel; Windows has none, so the
/// caller marks what must run. Blocking.
pub fn put_dir(choice: &Backend, dir: &Path, there: &str) -> Result<(), String> {
    let script = r#"rm -rf "$1.part" && mkdir -p "$1.part" && tar -xf - --no-same-owner -C "$1.part" && rm -rf "$1" && mv "$1.part" "$1""#;
    let mut command = in_root(choice, &["sh", "-c", script, "sh", there])
        .ok_or_else(|| t("Agents run on this computer."))?;
    let unreachable = |e: std::io::Error| {
        fill(
            t("Couldn't reach {backend}: {error}"),
            &[
                ("backend", &choice.label_in(language())),
                ("error", &e.to_string()),
            ],
        )
    };
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(unreachable)?;
    let stdin = child.stdin.take().ok_or_else(|| t("no input"))?;
    let dir = dir.to_path_buf();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        let mut tar = tar::Builder::new(stdin);
        tar.follow_symlinks(false);
        tar.append_dir_all(".", &dir)?;
        tar.into_inner().map(drop)
    });
    let out = child.wait_with_output().map_err(unreachable)?;
    let written = writer
        .join()
        .map_err(|_| t("The copy stopped.").to_string())?;
    match (out.status.success(), written) {
        (true, Ok(())) => Ok(()),
        (_, Err(e)) => Err(fill(
            t("Couldn't copy into {backend}: {error}"),
            &[
                ("backend", &choice.label_in(language())),
                ("error", &e.to_string()),
            ],
        )),
        (false, _) => Err(fill(
            t("Couldn't unpack into {path} on {backend}: {error}"),
            &[
                ("path", there),
                ("backend", &choice.label_in(language())),
                ("error", String::from_utf8_lossy(&out.stderr).trim()),
            ],
        )),
    }
}

/// `argv` in `choice` from its root folder with its login PATH, for the
/// caller to run (and log); `None` on this computer.
pub fn in_root(choice: &Backend, argv: &[&str]) -> Option<Command> {
    let mut launch = choice.launch("/")?;
    launch.push("env".to_string());
    launch.extend(login_path(choice).map(|path| format!("PATH={path}")));
    launch.extend(argv.iter().map(|arg| arg.to_string()));
    let (program, args) = launch.split_first()?;
    let mut command = pi_rpc::command(program);
    // This computer's PATH finds the launcher (wsl.exe, orb, docker); the
    // backend's travels as an argument.
    command.args(args).env("PATH", pi_rpc::child_path());
    Some(command)
}

/// `argv` in the folder `cwd` where its agents run, with that backend's
/// login PATH; `None` when they run on this computer.
pub fn in_folder(cwd: &Path, argv: &[&str]) -> Option<Command> {
    let choice = for_tab(cwd);
    let mut launch = choice.launch(&reach(&choice, cwd).ok()?)?;
    launch.push("env".to_string());
    launch.extend(login_path(&choice).map(|path| format!("PATH={path}")));
    launch.extend(argv.iter().map(|arg| arg.to_string()));
    let (program, args) = launch.split_first()?;
    let mut command = pi_rpc::command(program);
    command.args(args);
    Some(command)
}

/// Where the app keeps what it installs in `choice` (Java, Maestro):
/// `~/.local/share/osdy-code-desktop` there.
pub fn toolchain(choice: &Backend) -> Option<String> {
    Some(format!(
        "{}/.local/share/osdy-code-desktop",
        remote_home(choice)?
    ))
}

/// `choice`'s machine as `std::env::consts::ARCH` names it (`x86_64`,
/// `aarch64`), for downloads built for it.
pub fn arch(choice: &Backend) -> Option<&'static str> {
    let (_, out) = run_in(choice, &["uname".to_string(), "-m".to_string()])?;
    match out.trim() {
        "x86_64" | "amd64" => Some("x86_64"),
        "aarch64" | "arm64" => Some("aarch64"),
        _ => None,
    }
}

/// The backend user's home folder, asked once per backend.
fn remote_home(choice: &Backend) -> Option<String> {
    static HOMES: LazyLock<Mutex<HashMap<Backend, String>>> = LazyLock::new(Mutex::default);
    if let Some(home) = HOMES.lock().ok()?.get(choice) {
        return Some(home.clone());
    }
    let (ok, home) = run_in(
        choice,
        &["sh", "-c", r#"printf %s "$HOME""#].map(str::to_string),
    )?;
    let home = home.trim().trim_end_matches('/').to_string();
    if !ok || !home.starts_with('/') {
        return None;
    }
    HOMES.lock().ok()?.insert(choice.clone(), home.clone());
    Some(home)
}

/// `gentle-shell` where `cwd`'s agents run, for its own commands in that
/// folder (`install -l`, `upgrade`): [`pi_rpc::LaunchSpec::launcher_command`]
/// runs them. Blocking, like [`launch_spec`].
pub fn shell_spec(cwd: &Path) -> Result<pi_rpc::LaunchSpec> {
    launch_spec(&for_tab(cwd), cwd, &[], &[])
}

/// `gentle-shell` where new agent tabs run, for commands about the
/// installation (`--version`, `setup`, `install`): from the user's home
/// here, or the backend's root, which every backend has (a container may not
/// see this computer's home). Blocking, like [`launch_spec`].
pub fn home_shell_spec() -> Result<pi_rpc::LaunchSpec> {
    match saved() {
        Backend::Local => {
            pi_rpc::resolve_backend(&dirs::home_dir().unwrap_or_else(std::env::temp_dir))
        }
        choice => launch_spec(&choice, Path::new("/"), &[], &[]),
    }
}

/// `git` for the repository at `cwd`, run where `cwd`'s agents run (in a
/// WSL distro instead of over `\\wsl.localhost`, which is slow), with `env`
/// set; the caller adds the arguments. A folder the backend can't reach
/// runs on this computer, the only place that can read it.
pub fn git(cwd: &Path, env: &[(&str, &str)]) -> Command {
    let choice = for_tab(cwd);
    let launch = reach(&choice, cwd)
        .ok()
        .and_then(|folder| choice.launch(&folder));
    match launch.as_deref() {
        Some([program, args @ ..]) => {
            let mut command = pi_rpc::command(program);
            command
                .args(args)
                .arg("env")
                .args(env.iter().map(|(key, value)| format!("{key}={value}")))
                .arg("git");
            command
        }
        _ => {
            let mut command = pi_rpc::command("git");
            command.current_dir(cwd).envs(env.iter().copied());
            command
        }
    }
}

/// A path `git` printed for the repository at `cwd` (its top level, a
/// worktree), as this computer reads it: in the same form as `cwd`, which
/// Windows canonicalizes to `\\?\UNC\wsl.localhost\…`, so prefixes compare.
pub fn host_path(cwd: &Path, printed: &str) -> PathBuf {
    let host = for_tab(cwd).host_path(printed);
    match host.strip_prefix(r"\\") {
        Some(share) if cwd.to_string_lossy().starts_with(r"\\?\UNC\") => {
            PathBuf::from(format!(r"\\?\UNC\{share}"))
        }
        _ => PathBuf::from(host),
    }
}

/// `host`, a path on this computer, as the backend that runs work rooted at
/// `cwd` reads it (for `git worktree add <folder>`); as it is when the
/// backend can't reach it.
pub fn there(cwd: &Path, host: &Path) -> String {
    let host = host.to_string_lossy();
    for_tab(cwd)
        .path(&host)
        .unwrap_or_else(|| host.into_owned())
}

/// The argv of a terminal in `cwd` where its agents run: the backend's
/// login shell there; `None` on this computer. The user's home here (where
/// Setup opens its terminal) is the user's home there, not
/// `/mnt/c/Users/…`.
pub fn terminal(cwd: &Path) -> Option<Vec<String>> {
    let choice = for_tab(cwd);
    if dirs::home_dir().as_deref() == Some(cwd) {
        return choice.terminal("~");
    }
    choice.terminal(&reach(&choice, cwd).ok()?)
}

/// Read a file of the saved backend, `path` as the backend sees it: through
/// `\\wsl.localhost` for a WSL distro, with `cat` through a command.
/// Blocking.
pub fn read(choice: &Backend, path: &str) -> Option<String> {
    match choice {
        Backend::Local => std::fs::read_to_string(path).ok(),
        Backend::Wsl { .. } => std::fs::read_to_string(choice.host_path(path)).ok(),
        Backend::Command { .. } => run_in(choice, &["cat".to_string(), path.to_string()])
            .filter(|(ok, _)| *ok)
            .map(|(_, text)| text),
    }
}

/// Run `program` with `args` on `choice` with its login PATH: whether it
/// succeeded, and its output (stdout). Blocking, within the probe timeout.
pub fn run(choice: &Backend, program: &str, args: &[&str]) -> Option<(bool, String)> {
    let mut argv = vec!["env".to_string()];
    argv.extend(login_path(choice).map(|path| format!("PATH={path}")));
    argv.push(program.to_string());
    argv.extend(args.iter().map(|arg| arg.to_string()));
    run_in(choice, &argv)
}

/// The backend whose homes the app last pointed at ([`sync_homes`]).
static SYNCED: Mutex<Option<Backend>> = Mutex::new(None);

/// Point the app at the saved backend's Gentle Shell homes ([`use_home`]).
/// Blocking: it asks `gentle-shell --version` there.
pub fn sync_homes() {
    let choice = saved();
    let home = match choice {
        Backend::Local => None,
        _ => crate::features::setup::launcher_home(),
    };
    use_home(&choice, home.as_deref());
    if let Ok(mut synced) = SYNCED.lock() {
        *synced = Some(choice);
    }
}

/// [`sync_homes`] unless the app already reads the saved backend's homes,
/// so a screen that reads them never shows this computer's by mistake.
/// Blocking.
pub fn ensure_homes() {
    let current = SYNCED.lock().ok().and_then(|synced| synced.clone());
    if current.as_ref() != Some(&saved()) {
        sync_homes();
    }
}

/// Use `home`, the agent dir `gentle-shell --version` names in `choice`, as
/// the app's Gentle Shell home when this computer reaches it (a WSL
/// distro's, through `\\wsl.localhost`), so Configuration, sessions and
/// usage read the files the agents use; this computer's own otherwise.
pub fn use_home(choice: &Backend, home: Option<&str>) {
    let homes = matches!(choice, Backend::Wsl { .. })
        .then_some(home)
        .flatten()
        .map(|dir| PathBuf::from(choice.host_path(dir)))
        .filter(|dir| dir.is_dir())
        .map(pi_rpc::backend_homes);
    pi_rpc::use_backend_homes(homes);
}

/// Each backend's login PATH, asked once and kept.
static PATHS: LazyLock<Mutex<HashMap<Backend, Option<String>>>> = LazyLock::new(Mutex::default);

/// Ask the login PATHs (and what they find) again: something was
/// installed there.
pub fn forget_paths() {
    if let Ok(mut paths) = PATHS.lock() {
        paths.clear();
    }
    if let Ok(mut shells) = SHELLS.lock() {
        shells.clear();
    }
    if let Ok(mut versions) = VERSIONS.lock() {
        versions.clear();
    }
}

/// Each backend's `gentle-shell` version, asked once: every `--version`
/// there starts Node in the backend.
static VERSIONS: LazyLock<Mutex<HashMap<Backend, Option<String>>>> = LazyLock::new(Mutex::default);

/// The version of the `gentle-shell` `spec` runs on `choice`: cached per
/// launcher here, per backend there (Setup's checks ask again).
pub fn shell_version(choice: &Backend, spec: &pi_rpc::LaunchSpec) -> Option<String> {
    if *choice == Backend::Local {
        return pi_rpc::backend_version(spec);
    }
    if let Some(known) = VERSIONS.lock().ok()?.get(choice) {
        return known.clone();
    }
    let version = pi_rpc::backend_version(spec);
    VERSIONS
        .lock()
        .ok()?
        .insert(choice.clone(), version.clone());
    version
}

/// The backend's login PATH, asked once per backend and kept.
fn login_path(choice: &Backend) -> Option<String> {
    if let Some(known) = PATHS.lock().ok()?.get(choice) {
        return known.clone();
    }
    let path =
        run_in(choice, &backend::path_probe()).and_then(|(_, out)| backend::parse_path_probe(&out));
    if let Ok(mut paths) = PATHS.lock() {
        paths.insert(choice.clone(), path.clone());
    }
    path
}

/// Run `argv` on `choice` from its root folder, within [`PROBE_TIMEOUT`]:
/// whether it succeeded, and its output.
fn run_in(choice: &Backend, argv: &[String]) -> Option<(bool, String)> {
    let mut launch = choice.launch("/")?;
    launch.extend(argv.iter().cloned());
    let (program, args) = launch.split_first()?;
    let mut command = pi_rpc::command(program);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    pi_rpc::output_within(&mut command, PROBE_TIMEOUT)
}

/// Where `gentle-shell` is on `choice`'s login PATH; in WSL it may be a
/// Windows install seen through interop ([`backend::is_windows_file`]).
pub fn gentle_shell_path(choice: &Backend) -> Option<String> {
    run(choice, "sh", &["-c", "command -v gentle-shell"])
        .filter(|(ok, _)| *ok)
        .map(|(_, out)| out.trim().to_string())
        .filter(|path| !path.is_empty())
}

/// Whether `choice` has its own `gentle-shell` (a Windows one reached
/// through WSL interop doesn't count).
pub fn has_gentle_shell(choice: &Backend) -> bool {
    gentle_shell_path(choice).is_some_and(|path| !backend::is_windows_file(&path))
}

/// This computer's WSL distros (`wsl.exe --list --verbose`); empty without
/// WSL, and anywhere but Windows.
pub fn wsl_distros() -> Vec<Distro> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let mut command = pi_rpc::command("wsl.exe");
    command
        .args(["--list", "--verbose"])
        // UTF-8 output on current WSL; `parse_distros` copes with UTF-16 too.
        .env("WSL_UTF8", "1")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    pi_rpc::output_within(&mut command, PROBE_TIMEOUT)
        .filter(|(ok, _)| *ok)
        .map(|(_, out)| backend::parse_distros(&out))
        .unwrap_or_default()
}
