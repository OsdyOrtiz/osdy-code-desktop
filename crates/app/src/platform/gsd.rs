//! `ocd`: the user's command for opening Osdy Code Desktop from a
//! terminal, like `gs` opens Gentle Shell. `ocd` opens the current folder as
//! a workspace and `ocd <folder>` that folder; with the app already running,
//! the single-instance server switches its window to that workspace.
//!
//! The command is a generated shim that runs this executable with `--cwd`
//! and returns to the prompt at once for normal launches. Workspace commands
//! run synchronously so their output and exit status reach the calling shell.
//! Unlike the agents' shims it goes on
//! the user's own PATH, but only when they ask (palette → Install `ocd`
//! command) or through the Windows installer, which ships its own copy.

use std::path::{Path, PathBuf};

/// The line that marks an `ocd` file as ours, so it is never overwritten or
/// refreshed when it belongs to something else.
const MARKER: &str = "osdy-code-desktop ocd shim";

/// Where the app keeps the shim: `~/.local/bin` on macOS and Linux (usually on
/// PATH already), its own data folder on Windows.
fn shim_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        Some(crate::shared::data_dir::app()?.join("bin").join("ocd.cmd"))
    }
    #[cfg(not(windows))]
    {
        Some(dirs::home_dir()?.join(".local").join("bin").join("ocd"))
    }
}

#[cfg(windows)]
fn shim_body(exe: &Path) -> String {
    // `start` returns to the prompt instead of waiting for the window; cmd.exe
    // would expand a `%` in the path.
    let exe = exe.to_string_lossy().replace('%', "%%");
    format!(
        "@echo off\r\nrem {MARKER}\r\nif \"%~1\"==\"workspace\" goto workspace\r\nstart \"\" \"{exe}\" --cwd %*\r\nexit /b %errorlevel%\r\n:workspace\r\n\"{exe}\" %*\r\nexit /b %errorlevel%\r\n"
    )
}

#[cfg(not(windows))]
fn shim_body(exe: &Path) -> String {
    let exe = exe.to_string_lossy().replace('\'', "'\\''");
    format!(
        "#!/bin/sh\n# {MARKER}\nif [ \"$1\" = workspace ]; then\n  exec '{exe}' \"$@\"\nfi\n'{exe}' --cwd \"$@\" >/dev/null 2>&1 &\n"
    )
}

fn is_ours(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|body| body.contains(MARKER))
}

fn write_shim(path: &Path, exe: &Path) -> std::io::Result<()> {
    let body = shim_body(exe);
    if std::fs::read_to_string(path).ok().as_deref() == Some(body.as_str()) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// At startup: point an installed shim at this executable again (an update
/// or a moved app changes its path). Never creates one, and an isolated copy
/// (`GENTLE_SHELL_GPUI_DATA_DIR`) leaves the user's `ocd` alone.
pub fn refresh() {
    if crate::shared::data_dir::overridden() {
        return;
    }
    let (Some(path), Ok(exe)) = (shim_path(), std::env::current_exe()) else {
        return;
    };
    if is_ours(&path) {
        let _ = write_shim(&path, &exe);
    }
}

/// Palette → Install `ocd` command. Blocking (Windows edits the user PATH
/// through PowerShell): call off the UI thread. The text says what happened.
pub fn install() -> Result<String, String> {
    use crate::shared::i18n::{Message, t};

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // The Windows installer ships `bin\ocd.cmd` next to the exe: only make
    // sure its folder is on PATH.
    #[cfg(windows)]
    if let Some(bin) = exe.parent().map(|dir| dir.join("bin"))
        && bin.join("ocd.cmd").is_file()
    {
        return add_to_path(&bin);
    }
    let path = shim_path().ok_or_else(|| t("This system has no home or app data folder."))?;
    if path.exists() && !is_ours(&path) {
        return Err(
            t("{path} already exists and isn't Osdy Code Desktop's; remove it first.")
                .replace("{path}", &path.display().to_string()),
        );
    }
    write_shim(&path, &exe).map_err(|e| {
        Message::with(
            "Could not write {path}: {error}",
            [
                ("{path}", path.display().to_string()),
                ("{error}", e.to_string()),
            ],
        )
        .render()
    })?;
    let dir = path.parent().unwrap_or(&path);
    add_to_path(dir)
}

#[cfg(windows)]
fn add_to_path(dir: &Path) -> Result<String, String> {
    use crate::shared::i18n::{Message, t};

    // Read and write the raw value: `[Environment]::SetEnvironmentVariable`
    // would expand `%USERPROFILE%`-style entries for good. Setting and
    // clearing a dummy variable through it broadcasts the change, so new
    // terminals see it without signing out.
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$dir = $env:OCD_DIR
$key = Get-Item 'HKCU:\Environment'
$raw = $key.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
$entries = @($raw -split ';' | Where-Object { $_ -ne '' })
if ($entries | Where-Object { $_.TrimEnd('\') -ieq $dir.TrimEnd('\') }) { 'present'; exit 0 }
$value = (@($entries) + $dir) -join ';'
[Microsoft.Win32.Registry]::SetValue('HKEY_CURRENT_USER\Environment', 'Path', $value, 'ExpandString')
[Environment]::SetEnvironmentVariable('OCD_PATH_BROADCAST', '1', 'User')
[Environment]::SetEnvironmentVariable('OCD_PATH_BROADCAST', $null, 'User')
'added'
"#;
    let out = pi_rpc::command("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("OCD_DIR", dir)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| t("Could not run PowerShell: {error}").replace("{error}", &e.to_string()))?;
    if !out.status.success() {
        return Err(Message::with(
            "Could not add {path} to your PATH: {error}",
            [
                ("{path}", dir.display().to_string()),
                (
                    "{error}",
                    String::from_utf8_lossy(&out.stderr).trim().to_string(),
                ),
            ],
        )
        .render());
    }
    Ok(
        if String::from_utf8_lossy(&out.stdout).trim() == "present" {
            t("ocd is installed. Type ocd in any folder to open it here.").into()
        } else {
            t("ocd is installed. Open a new terminal and type ocd in any folder.").into()
        },
    )
}

#[cfg(not(windows))]
fn add_to_path(dir: &Path) -> Result<String, String> {
    use crate::shared::i18n::t;

    let on_path =
        std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|p| p == dir));
    Ok(if on_path {
        t("ocd is installed. Type ocd in any folder to open it here.").into()
    } else {
        t("ocd is installed in {path}, which isn't on your PATH yet: add it in your shell profile.")
            .replace("{path}", &dir.display().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::{is_ours, shim_body, shim_path, write_shim};

    #[test]
    fn fork_identity_owns_only_its_own_terminal_shim() {
        let root = std::env::temp_dir().join(format!(
            "ocd-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("ocd");
        let exe = std::path::Path::new("/Apps/Osdy Code Desktop/gentle-shell-gpui");
        write_shim(&path, exe).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        let ours = is_ours(&path);
        std::fs::write(&path, "# gentle-shell-gpui gsd shim\n").unwrap();
        let upstream = is_ours(&path);
        std::fs::remove_dir_all(&root).unwrap();

        let filename = if cfg!(windows) { "ocd.cmd" } else { "ocd" };
        assert_eq!(shim_path().unwrap().file_name().unwrap(), filename);
        assert!(body.lines().any(|line| {
            line == "# osdy-code-desktop ocd shim" || line == "rem osdy-code-desktop ocd shim"
        }));
        assert!(ours);
        assert!(!upstream);
    }

    #[test]
    fn shim_passes_cwd_and_is_recognized_as_ours() {
        let dir = std::env::temp_dir().join(format!("ocd-shim-test-{}", std::process::id()));
        let path = dir.join("ocd");
        let exe = std::path::Path::new("/Apps/Osdy Code Desktop/gentle-shell-gpui");
        write_shim(&path, exe).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body, shim_body(exe));
        assert!(body.contains("--cwd") && body.contains("Osdy Code Desktop"));
        assert!(body.contains("workspace") && body.contains("exit /b") == cfg!(windows));
        assert!(is_ours(&path));

        // Someone else's `ocd` is never taken for ours.
        std::fs::write(&path, "#!/bin/sh\necho other\n").unwrap();
        assert!(!is_ours(&path));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_shim_escapes_percent_and_returns_to_the_prompt() {
        let body = shim_body(std::path::Path::new(
            r"C:@%\Osdy Code Desktop\gentle-shell-gpui.exe",
        ));
        assert!(
            body.contains(r#"start "" "C:@%%\Osdy Code Desktop\gentle-shell-gpui.exe" --cwd %*"#)
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_shim_quotes_the_path_and_runs_in_the_background() {
        let body = shim_body(std::path::Path::new("/Apps/It's/gs"));
        assert!(body.contains(r#"'/Apps/It'\''s/gs' --cwd "$@" >/dev/null 2>&1 &"#));
    }

    #[test]
    fn rewriting_the_same_shim_leaves_it_untouched() {
        let dir = std::env::temp_dir().join(format!("ocd-idem-test-{}", std::process::id()));
        let path = dir.join("ocd");
        let exe = std::path::Path::new("/x/gs");
        write_shim(&path, exe).unwrap();
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();
        write_shim(&path, exe).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
