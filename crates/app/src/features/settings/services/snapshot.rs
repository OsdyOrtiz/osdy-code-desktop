//! Read and enrich one configuration snapshot.
//!
//! Runs only on the background executor. Keep this in the same order as the
//! prior SettingsView::reload implementation: read files, resolve launcher,
//! probe, check Gentle Shell update, check Engram update.

use std::path::Path;

use super::super::snapshot::{Snapshot, probe, read_snapshot};
use crate::shared::i18n::t;

pub(in crate::features::settings) fn load(cwd: &Path, shell_updates: bool) -> Snapshot {
    // Where agents run in WSL, its homes; never this computer's by mistake
    // while they are still being asked for.
    crate::features::backend::ensure_homes();
    let homes = pi_rpc::resolve_homes();
    let mut snapshot = read_snapshot(&homes.agent, &homes.config, cwd);
    let spec = crate::features::backend::shell_spec(cwd)
        .or_else(|_| crate::features::backend::home_shell_spec())
        .ok();
    if let Some(spec) = &spec {
        probe(spec, &mut snapshot);
    }
    // In a backend the launcher is its own `gentle-shell`, named with where;
    // a Windows one WSL sees through interop isn't (as Setup says).
    let choice = crate::features::backend::for_tab(cwd);
    if choice != gentle_core::backend::Backend::Local {
        let label = choice.label();
        snapshot.launcher = crate::features::backend::gentle_shell_path(&choice).map(|path| {
            if gentle_core::backend::is_windows_file(&path) {
                gentle_core::i18n::fill(
                    t("not found in {label} (only Windows' {path})"),
                    &[("label", &label), ("path", &path)],
                )
                .into()
            } else {
                format!("{path} ({label})").into()
            }
        });
    }
    // gentle-ai owns updates of installs it manages; a responding Shell
    // supplies update status in preference to the launcher registry check.
    snapshot.managed = spec
        .as_ref()
        .is_some_and(pi_rpc::LaunchSpec::managed_by_gentle_ai);
    snapshot.skipped_release = crate::features::updates::skipped();
    if !snapshot.managed && !shell_updates {
        snapshot.update = crate::features::updates::current_from_versions(&snapshot.versions)
            .and_then(crate::features::updates::check);
    }
    snapshot.engram_update = crate::features::setup::engram_version()
        .and_then(|v| crate::features::updates::engram::check(&v));
    snapshot
}
