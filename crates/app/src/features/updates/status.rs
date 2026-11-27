//! Gentle Shell's own update status (`get_update_status`), for the
//! Configuration screen when the active agent tab's Shell advertises it:
//! Gentle Shell's release (as the same offer the npm path makes), Pi, and
//! Pi's packages. The npm registry path stays the answer for every other
//! Shell.

use gentle_core::version::offer_for;
use pi_rpc::desktop_rpc::{
    PackageKind, PackageUpdate, PackageUpdateState, ShellChannel, ShellUpdate, UpdateState,
    UpdateStatus,
};

use super::UpdateOffer;
use crate::shared::i18n::t;

/// The "Gentle Shell X is available" offer from Gentle Shell's answer: only
/// a release (or bundled) install with a newer stable release, never a
/// skipped version, and never when gentle-ai manages the install (it updates
/// it itself; `managed` is the app's own launcher check, the `channel` the
/// Shell's).
pub fn offer_from_status(
    shell: &ShellUpdate,
    managed: bool,
    skipped: Option<&str>,
) -> Option<UpdateOffer> {
    if managed
        || shell.state != UpdateState::UpdateAvailable
        || !matches!(shell.channel, ShellChannel::Release | ShellChannel::Bundled)
    {
        return None;
    }
    offer_for(shell.current.as_deref()?, shell.latest.as_deref()?, skipped)
}

/// The commit offered by Update now for a self-managed main install.
pub fn main_update_target(shell: &ShellUpdate, managed: bool) -> Option<&str> {
    if managed || shell.channel != ShellChannel::Main || shell.state != UpdateState::UpdateAvailable
    {
        return None;
    }
    shell.latest.as_deref()
}

/// Pi's update can be offered even when the Shell doesn't name a version.
pub fn pi_update_available(status: &UpdateStatus, managed: bool) -> bool {
    !is_managed(status.gentle_shell.channel, managed)
        && status.pi.state == UpdateState::UpdateAvailable
}

fn is_managed(channel: ShellChannel, managed: bool) -> bool {
    managed || channel == ShellChannel::GentleAiManaged
}

/// The rows shown beside the versions, always: the channel, Gentle Shell's
/// state, the update command Gentle Shell reports,
/// Pi's state, and when the answer was looked up. An `unknown` state is
/// never shown as up to date. A managed install says gentle-ai updates it.
pub fn status_rows(status: &UpdateStatus, managed: bool) -> Vec<(&'static str, String)> {
    let shell = &status.gentle_shell;
    let managed = is_managed(shell.channel, managed);
    let mut rows = vec![(
        t("Channel"),
        channel_label(shell.channel, managed).to_string(),
    )];
    let shell_state = match shell.state {
        _ if managed => t("updated by gentle-ai").to_string(),
        UpdateState::UpToDate => up_to_date(shell.current.as_deref()),
        UpdateState::UpdateAvailable => match (shell.channel, shell.latest.as_deref()) {
            (ShellChannel::Main, Some(latest)) => {
                t("newer commit {latest} on main").replace("{latest}", latest)
            }
            (_, Some(latest)) => t("{latest} is available").replace("{latest}", latest),
            (_, None) => t("an update is available").to_string(),
        },
        UpdateState::Unknown => not_checked(status.offline, shell.checked_at.as_deref()),
    };
    rows.push(("Gentle Shell", shell_state));
    // A launcher the app sees as managed, on a channel that still names
    // `gentle-shell upgrade`: gentle-ai updates it, so no command.
    let foreign = managed && shell.channel != ShellChannel::GentleAiManaged;
    if let Some(command) = shell.command.as_ref().filter(|_| !foreign) {
        rows.push((t("Update command"), command.clone()));
    }
    let pi = &status.pi;
    let pi_state = match pi.state {
        UpdateState::UpToDate => up_to_date(pi.current.as_deref()),
        UpdateState::UpdateAvailable => {
            let latest = pi.latest.as_deref().unwrap_or_else(|| t("a newer version"));
            let current = pi
                .current
                .as_deref()
                .map(|version| t(" (you have {version})").replace("{version}", version))
                .unwrap_or_default();
            gentle_core::i18n::fill(
                t("{latest} is available{current} · run `{command}`"),
                &[
                    ("latest", latest),
                    ("current", &current),
                    ("command", &pi.command),
                ],
            )
        }
        // Gentle Shell looked itself up but not Pi: it checks Pi only on a
        // bundled install.
        UpdateState::Unknown
            if !status.offline && pi.checked_at.is_none() && shell.checked_at.is_some() =>
        {
            t("not checked: Gentle Shell checks Pi only on bundled installs").to_string()
        }
        UpdateState::Unknown => not_checked(status.offline, pi.checked_at.as_deref()),
    };
    rows.push(("Pi", pi_state));
    if let Some(at) = &shell.checked_at {
        rows.push((t("Last checked"), checked_at_label(at)));
    }
    rows
}

fn channel_label(channel: ShellChannel, managed: bool) -> &'static str {
    t(match channel {
        _ if managed => "managed by gentle-ai",
        ShellChannel::Release => "release",
        ShellChannel::Main => "main",
        ShellChannel::Bundled => "bundled",
        ShellChannel::GentleAiManaged => "managed by gentle-ai",
        ShellChannel::Unknown => "unknown",
    })
}

fn up_to_date(current: Option<&str>) -> String {
    current.map_or_else(
        || t("up to date").to_string(),
        |version| t("up to date ({version})").replace("{version}", version),
    )
}

/// Why a state is `unknown`: never "up to date".
fn not_checked(offline: bool, checked_at: Option<&str>) -> String {
    t(if offline {
        "not checked: Gentle Shell is offline (PI_OFFLINE)"
    } else if checked_at.is_none() {
        "not checked yet"
    } else {
        "couldn't check"
    })
    .to_string()
}

/// `2026-11-17T12:00:00.000Z` as `2026-11-17 12:00 UTC`; anything else as
/// Gentle Shell sent it.
fn checked_at_label(at: &str) -> String {
    match (at.get(..10), at.get(10..11), at.get(11..16)) {
        (Some(date), Some("T"), Some(time)) if at.ends_with('Z') => format!("{date} {time} UTC"),
        _ => at.to_string(),
    }
}

/// The chip beside a configured package: an update (with its version when
/// Gentle Shell names one), pinned, or not checked. Nothing for
/// `no_update_reported`: Pi's check lists only packages with updates, so
/// that is not "up to date" either.
pub fn package_chip(item: &PackageUpdate) -> Option<String> {
    match item.state {
        PackageUpdateState::UpdateAvailable => Some(item.latest.as_deref().map_or_else(
            || t("update").to_string(),
            |latest| t("update → {latest}").replace("{latest}", latest),
        )),
        _ if item.pinned == Some(true) => Some(t("pinned").to_string()),
        PackageUpdateState::Skipped | PackageUpdateState::Unknown => {
            Some(t("not checked").to_string())
        }
        PackageUpdateState::NoUpdateReported => None,
    }
}

/// Gentle Shell's row for a package configured as `source` (`npm:name@v`,
/// `git:host/path@ref`, `https://host/path.git`, or a local path).
pub fn package_status<'a>(source: &str, items: &'a [PackageUpdate]) -> Option<&'a PackageUpdate> {
    let npm = crate::features::settings::npm_name(source);
    let git = {
        let rest = source.strip_prefix("git:").unwrap_or(source);
        let rest = rest
            .split_once("://")
            .map_or(rest, |(_, after)| after)
            .split('@')
            .next()
            .unwrap_or(rest);
        rest.strip_suffix(".git").unwrap_or(rest)
    };
    let local = std::path::Path::new(source)
        .file_name()
        .and_then(|n| n.to_str());
    items.iter().find(|item| match item.kind {
        PackageKind::Npm => npm == Some(item.name.as_str()),
        PackageKind::Git => git == item.name,
        PackageKind::Local => local == Some(item.name.as_str()),
        PackageKind::Unknown => false,
    })
}

/// The Packages line when none has an update: what Pi's check can tell,
/// without claiming the packages are up to date.
pub fn packages_note(status: &UpdateStatus, with_updates: usize) -> Option<&'static str> {
    let packages = &status.packages;
    if !packages.available {
        Some(t(
            "Gentle Shell couldn't read Pi's package list, so updates weren't checked",
        ))
    } else if with_updates == 0 && !packages.items.is_empty() {
        Some(t("Pi's check reported no package updates"))
    } else {
        None
    }
}

/// How many configured packages Pi's check says have a newer version.
pub fn packages_with_updates(status: &UpdateStatus) -> usize {
    status
        .packages
        .items
        .iter()
        .filter(|item| item.state == PackageUpdateState::UpdateAvailable)
        .count()
}

#[cfg(test)]
mod tests {
    use pi_rpc::desktop_rpc::{DesktopRecord, SCHEMA, UpdateState, UpdateStatus, decode_payload};

    use super::{
        checked_at_label, main_update_target, offer_from_status, package_chip, package_status,
        packages_note, packages_with_updates, pi_update_available, status_rows,
    };

    fn status(data: serde_json::Value) -> UpdateStatus {
        let line = serde_json::json!({
            "schema": SCHEMA, "kind": "response", "requestId": "desktop-71", "ok": true, "data": data,
        })
        .to_string();
        match decode_payload(&line).unwrap() {
            DesktopRecord::Response(response) => response.update_status().unwrap(),
            other => panic!("expected a response, got {other:?}"),
        }
    }

    /// The spec's example, with `channel` and Pi's state adjustable.
    fn example(channel: &str, latest: &str, pi_state: &str) -> UpdateStatus {
        status(serde_json::json!({
            "refreshed": true,
            "offline": false,
            "gentleShell": { "current": "4.8.0", "latest": latest, "channel": channel, "state": "update_available", "command": "gentle-shell upgrade", "checkedAt": "2026-11-17T12:00:00.000Z" },
            "pi": { "current": "1.5.2", "latest": null, "state": pi_state, "command": "gentle-shell update", "checkedAt": null },
            "packages": {
                "available": true,
                "items": [
                    { "name": "pi-web-access", "kind": "npm", "scope": "user", "current": "1.4.0", "latest": null, "pinned": false, "state": "update_available" },
                    { "name": "github.com/user/repo", "kind": "git", "scope": "project", "current": null, "latest": null, "pinned": true, "state": "skipped" }
                ],
                "truncated": false,
                "command": "gentle-shell update --extensions",
                "checkedAt": "2026-11-17T12:00:00.000Z"
            }
        }))
    }

    #[test]
    fn a_release_update_is_offered_like_the_npm_path() {
        let status = example("release", "4.9.0", "unknown");
        let offer = offer_from_status(&status.gentle_shell, false, None).unwrap();
        assert_eq!(
            (offer.current.as_str(), offer.latest.as_str()),
            ("4.8.0", "4.9.0")
        );
        assert!(offer.notes_url.ends_with("/v4.9.0"));
        // Skipped, managed by gentle-ai (either way), or another channel: none.
        assert_eq!(
            offer_from_status(&status.gentle_shell, false, Some("4.9.0")),
            None
        );
        assert_eq!(offer_from_status(&status.gentle_shell, true, None), None);
        for channel in ["gentle-ai-managed", "main", "unknown"] {
            let other = example(channel, "4.9.0", "unknown");
            assert_eq!(
                offer_from_status(&other.gentle_shell, false, None),
                None,
                "{channel}"
            );
        }
        assert!(
            offer_from_status(
                &example("bundled", "4.9.0", "unknown").gentle_shell,
                false,
                None
            )
            .is_some()
        );
    }

    #[test]
    fn update_action_main_requires_a_new_commit_on_unmanaged_main() {
        let commit = "ce64a9a708e5";
        let mut main = example("main", commit, "unknown").gentle_shell;
        assert_eq!(main_update_target(&main, false), Some(commit));
        assert_eq!(main_update_target(&main, true), None);
        for state in [UpdateState::UpToDate, UpdateState::Unknown] {
            main.state = state;
            assert_eq!(main_update_target(&main, false), None, "{state:?}");
        }
        main.state = UpdateState::UpdateAvailable;
        main.latest = None;
        assert_eq!(main_update_target(&main, false), None);
        for channel in ["release", "bundled", "gentle-ai-managed", "unknown"] {
            let shell = example(channel, "4.9.0", "unknown").gentle_shell;
            assert_eq!(main_update_target(&shell, false), None, "{channel}");
        }
    }

    #[test]
    fn update_action_pi_requires_reported_availability_and_an_unmanaged_install() {
        let bundled = example("bundled", "4.9.0", "update_available");
        // Pi can report an available update without naming the new version.
        assert_eq!(bundled.pi.latest, None);
        assert_eq!(
            [
                pi_update_available(&bundled, false),
                pi_update_available(&bundled, true),
                pi_update_available(
                    &example("gentle-ai-managed", "4.9.0", "update_available"),
                    false,
                ),
                pi_update_available(&example("bundled", "4.9.0", "up_to_date"), false),
                pi_update_available(&example("bundled", "4.9.0", "unknown"), false),
            ],
            [true, false, false, false, false]
        );
    }

    fn rows(status: &UpdateStatus, managed: bool) -> Vec<(&'static str, String)> {
        status_rows(status, managed)
    }

    fn owned(rows: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
        rows.iter().map(|(l, v)| (*l, v.to_string())).collect()
    }

    #[test]
    fn rows_always_name_the_channel_state_and_command() {
        let main = example("main", "0336985abcde", "update_available");
        assert_eq!(
            rows(&main, false),
            owned(&[
                ("Channel", "main"),
                ("Gentle Shell", "newer commit 0336985abcde on main"),
                ("Update command", "gentle-shell upgrade"),
                (
                    "Pi",
                    "a newer version is available (you have 1.5.2) · run `gentle-shell update`"
                ),
                ("Last checked", "2026-11-17 12:00 UTC"),
            ])
        );
        // Up to date on main still names the channel and its command.
        let current = status(serde_json::json!({
            "refreshed": true, "offline": false,
            "gentleShell": { "current": "4.9.0", "latest": "0336985abcde", "channel": "main", "state": "up_to_date", "command": "gentle-shell upgrade --channel main", "checkedAt": "2026-11-17T12:00:00.000Z" },
            "pi": { "current": "1.5.2", "latest": "1.5.2", "state": "up_to_date", "command": "gentle-shell update", "checkedAt": "2026-11-17T12:00:00.000Z" },
            "packages": { "available": true, "items": [], "truncated": false, "command": "gentle-shell update --extensions", "checkedAt": null }
        }));
        assert_eq!(
            rows(&current, false),
            owned(&[
                ("Channel", "main"),
                ("Gentle Shell", "up to date (4.9.0)"),
                ("Update command", "gentle-shell upgrade --channel main"),
                ("Pi", "up to date (1.5.2)"),
                ("Last checked", "2026-11-17 12:00 UTC"),
            ])
        );
    }

    #[test]
    fn unknown_is_never_up_to_date() {
        let offline = status(serde_json::json!({
            "refreshed": false, "offline": true,
            "gentleShell": { "current": "4.9.0", "latest": null, "channel": "release", "state": "unknown", "command": "gentle-shell upgrade", "checkedAt": null },
            "pi": { "current": "1.5.2", "latest": null, "state": "unknown", "command": "gentle-shell update", "checkedAt": null },
            "packages": { "available": false, "items": [], "truncated": false, "command": "gentle-shell update --extensions", "checkedAt": null }
        }));
        let off = "not checked: Gentle Shell is offline (PI_OFFLINE)";
        assert_eq!(
            rows(&offline, false),
            owned(&[
                ("Channel", "release"),
                ("Gentle Shell", off),
                ("Update command", "gentle-shell upgrade"),
                ("Pi", off),
            ])
        );
        assert_eq!(
            packages_note(&offline, 0),
            Some("Gentle Shell couldn't read Pi's package list, so updates weren't checked")
        );
        // Online: never looked up, or looked up and failed.
        let unknown = example("unknown", "4.9.0", "unknown");
        let r = rows(&unknown, false);
        assert_eq!(r[0], ("Channel", "unknown".to_string()));
        assert_eq!(
            r[3],
            (
                "Pi",
                "not checked: Gentle Shell checks Pi only on bundled installs".to_string()
            )
        );
        let never = status(serde_json::json!({
            "refreshed": false, "offline": false,
            "gentleShell": { "current": "4.9.0", "latest": null, "channel": "release", "state": "unknown", "command": "gentle-shell upgrade", "checkedAt": null },
            "pi": { "current": "1.5.2", "latest": null, "state": "unknown", "command": "gentle-shell update", "checkedAt": null },
            "packages": { "available": true, "items": [], "truncated": false, "command": "gentle-shell update --extensions", "checkedAt": null }
        }));
        let r = rows(&never, false);
        assert_eq!(r[1], ("Gentle Shell", "not checked yet".to_string()));
        assert_eq!(r[3], ("Pi", "not checked yet".to_string()));
    }

    #[test]
    fn a_managed_install_says_gentle_ai_updates_it() {
        for (channel, managed) in [("gentle-ai-managed", false), ("release", true)] {
            let r = rows(&example(channel, "4.9.0", "unknown"), managed);
            assert_eq!(r[0], ("Channel", "managed by gentle-ai".to_string()));
            assert_eq!(r[1], ("Gentle Shell", "updated by gentle-ai".to_string()));
        }
        // The Shell's own managed command shows; `gentle-shell upgrade` on a
        // launcher the app sees as managed does not.
        let shell_managed = rows(&example("gentle-ai-managed", "4.9.0", "unknown"), false);
        assert_eq!(
            shell_managed[2],
            ("Update command", "gentle-shell upgrade".to_string())
        );
        let app_managed = rows(&example("release", "4.9.0", "unknown"), true);
        assert!(
            app_managed
                .iter()
                .all(|(label, _)| *label != "Update command")
        );
    }

    #[test]
    fn checked_at_reads_as_utc_or_stays_as_sent() {
        assert_eq!(
            checked_at_label("2026-11-17T12:00:00.000Z"),
            "2026-11-17 12:00 UTC"
        );
        assert_eq!(checked_at_label("yesterday"), "yesterday");
    }

    #[test]
    fn package_chips_never_claim_up_to_date() {
        let status = status(serde_json::json!({
            "refreshed": true, "offline": false,
            "gentleShell": { "current": "4.9.0", "latest": "4.9.0", "channel": "release", "state": "up_to_date", "command": "gentle-shell upgrade", "checkedAt": null },
            "pi": { "current": null, "latest": null, "state": "unknown", "command": "gentle-shell update", "checkedAt": null },
            "packages": { "available": true, "truncated": false, "command": "gentle-shell update --extensions", "checkedAt": null, "items": [
                { "name": "a", "kind": "npm", "scope": "user", "current": "1.0.0", "latest": null, "pinned": false, "state": "update_available" },
                { "name": "b", "kind": "npm", "scope": "user", "current": "1.0.0", "latest": "2.0.0", "pinned": false, "state": "update_available" },
                { "name": "c", "kind": "npm", "scope": "user", "current": "1.0.0", "latest": null, "pinned": true, "state": "skipped" },
                { "name": "d", "kind": "git", "scope": "user", "current": null, "latest": null, "pinned": null, "state": "skipped" },
                { "name": "e", "kind": "npm", "scope": "user", "current": "1.0.0", "latest": null, "pinned": false, "state": "unknown" },
                { "name": "f", "kind": "npm", "scope": "user", "current": "1.0.0", "latest": null, "pinned": false, "state": "no_update_reported" }
            ] }
        }));
        let chips: Vec<Option<String>> = status.packages.items.iter().map(package_chip).collect();
        assert_eq!(
            chips,
            vec![
                Some("update".into()),
                Some("update → 2.0.0".into()),
                Some("pinned".into()),
                Some("not checked".into()),
                Some("not checked".into()),
                None,
            ]
        );
    }

    #[test]
    fn packages_match_their_configured_sources() {
        let status = example("release", "4.9.0", "unknown");
        let items = &status.packages.items;
        assert_eq!(
            package_status("npm:pi-web-access@1.4.0", items)
                .unwrap()
                .name,
            "pi-web-access"
        );
        assert_eq!(
            package_status("git:github.com/user/repo@v2", items)
                .unwrap()
                .name,
            "github.com/user/repo"
        );
        assert_eq!(
            package_status("https://github.com/user/repo.git", items)
                .unwrap()
                .name,
            "github.com/user/repo"
        );
        assert!(package_status("npm:other", items).is_none());
        assert_eq!(packages_with_updates(&status), 1);
        assert_eq!(packages_note(&status, 1), None);
        assert_eq!(
            packages_note(&status, 0),
            Some("Pi's check reported no package updates")
        );
    }
}
