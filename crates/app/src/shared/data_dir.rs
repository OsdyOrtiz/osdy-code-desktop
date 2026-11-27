//! The app's own folder: `<data dir>/osdy-code-desktop/` (workspaces,
//! settings, the single-instance record, the toolchain it installs).
//!
//! `GENTLE_SHELL_GPUI_DATA_DIR` replaces it. A build started with it is a
//! separate app next to the installed one (its own window, workspaces and
//! settings) while Gentle Shell, its homes and sign-ins stay the real ones,
//! which is how a pull request is tried without setting providers up again
//! (docs/development.md, Isolated testing).

use std::path::PathBuf;

const OVERRIDE: &str = "GENTLE_SHELL_GPUI_DATA_DIR";

/// Whether this build runs as an isolated copy (`GENTLE_SHELL_GPUI_DATA_DIR`).
pub fn overridden() -> bool {
    std::env::var_os(OVERRIDE).is_some_and(|dir| !dir.is_empty())
}

/// The app's folder; `None` when the system has no data directory.
pub fn app() -> Option<PathBuf> {
    match std::env::var_os(OVERRIDE) {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => dirs::data_dir().map(|dir| dir.join("osdy-code-desktop")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fork_identity_uses_its_own_app_data_directory() {
        let expected = std::env::var_os("GENTLE_SHELL_GPUI_DATA_DIR")
            .filter(|dir| !dir.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::data_dir().map(|dir| dir.join("osdy-code-desktop")));
        assert_eq!(super::app(), expected);
    }
}
