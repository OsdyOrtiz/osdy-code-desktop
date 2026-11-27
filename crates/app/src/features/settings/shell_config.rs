//! Gentle Shell's settings stores (`persona`, `banner`, `notifications`)
//! over the active agent tab's Desktop channel (`get_config`/`set_config`,
//! `config_changed`), when its Shell serves them. A Shell without the config
//! methods, or one that answers `unknown_store` for a settings store (4.9.0
//! and older), keeps the direct file read (and, for notifications, the file
//! write) of before. Nothing here touches a file on the RPC path.

use std::collections::HashMap;

use gentle_core::notifications::{self, FileState, Policy};
use gpui_kit::*;
use pi_rpc::desktop_rpc::{
    ConfigErrorCode, ConfigState, ConfigStatus, ConfigStore, ConfigWrite, ConfigWriteTarget,
    NotificationsData,
};

use super::SettingsView;
use crate::features::profiles::{ConfigFailure, ConfigOutcome, ConfigSupport, ConfigTicket};
use crate::shared::i18n::t;

/// The stores this screen reads and writes.
pub(super) const SETTINGS_STORES: [ConfigStore; 3] = [
    ConfigStore::Persona,
    ConfigStore::Banner,
    ConfigStore::Notifications,
];

/// Where one settings store comes from on this tab.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum StoreSource {
    /// Gentle Shell's answer, the truth while the screen is open.
    Rpc(ConfigState),
    /// The Shell does not serve this store: the direct file path.
    Files,
}

/// What a ticket asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    Read(ConfigStore),
    Write(ConfigStore),
}

/// The settings stores as the active tab's Gentle Shell reported them.
#[derive(Default)]
pub(super) struct ShellConfig {
    stores: HashMap<ConfigStore, StoreSource>,
    tickets: HashMap<ConfigTicket, Asked>,
    /// Asked while the tab's probe had not answered: asked again once it
    /// does.
    awaiting: bool,
}

impl ShellConfig {
    /// The store's state when Gentle Shell reported it.
    pub(super) fn state(&self, store: ConfigStore) -> Option<&ConfigState> {
        match self.stores.get(&store) {
            Some(StoreSource::Rpc(state)) => Some(state),
            _ => None,
        }
    }

    /// Whether a write of `store` is waiting for its answer.
    fn writing(&self, store: ConfigStore) -> bool {
        self.tickets
            .values()
            .any(|asked| *asked == Asked::Write(store))
    }
}

/// The decision for one answer, pure so it can be tested without a window.
#[derive(Debug, PartialEq)]
enum Landed {
    /// Not this screen's ticket.
    Ignored,
    /// A read or write was adopted.
    Adopted,
    /// The Shell does not serve the store: the file path from now on.
    Files(ConfigStore),
    /// The store changed since it was read: nothing was written; re-read.
    Conflict(ConfigStore),
    /// A read failed: what was shown stays.
    ReadFailed,
    /// A write was refused or lost.
    WriteFailed(String),
}

/// The store a settings write names goes to Gentle Shell only when its
/// `get_config` answered; a store it does not know stays on the files.
fn land(shell: &mut ShellConfig, ticket: ConfigTicket, outcome: ConfigOutcome) -> Landed {
    let Some(asked) = shell.tickets.remove(&ticket) else {
        return Landed::Ignored;
    };
    match (asked, outcome) {
        (Asked::Read(store), ConfigOutcome::Read(state)) if state.store == store => {
            shell.stores.insert(store, StoreSource::Rpc(state));
            Landed::Adopted
        }
        (Asked::Write(store), ConfigOutcome::Written(result)) if result.state.store == store => {
            shell.stores.insert(store, StoreSource::Rpc(result.state));
            Landed::Adopted
        }
        (Asked::Read(store) | Asked::Write(store), ConfigOutcome::Failed(failure))
            if serves_not(&failure) =>
        {
            shell.stores.insert(store, StoreSource::Files);
            Landed::Files(store)
        }
        (
            Asked::Write(store),
            ConfigOutcome::Failed(ConfigFailure::Rejected {
                code: ConfigErrorCode::RevisionConflict,
                ..
            }),
        ) => Landed::Conflict(store),
        (Asked::Write(_), ConfigOutcome::Failed(failure)) => Landed::WriteFailed(failure.message()),
        (Asked::Write(_), _) => {
            Landed::WriteFailed(t("Gentle Shell answered something else.").into())
        }
        (Asked::Read(_), _) => Landed::ReadFailed,
    }
}

/// The Shell does not serve the store at all: no config methods, or a
/// store identifier it does not know (Gentle Shell 4.9.0's `unknown_store`).
fn serves_not(failure: &ConfigFailure) -> bool {
    failure.is_legacy()
        || matches!(
            failure,
            ConfigFailure::Rejected {
                code: ConfigErrorCode::UnknownStore,
                ..
            }
        )
}

/// The stores to read again after `config_changed` events: those shown from
/// Gentle Shell whose revision moved (or may have, after a gap).
fn stale_stores(
    shell: &ShellConfig,
    changes: &[(ConfigStore, Option<String>)],
) -> Vec<ConfigStore> {
    let mut stale = Vec::new();
    for (store, revision) in changes {
        let Some(state) = shell.state(*store) else {
            continue;
        };
        let moved = revision.as_deref() != Some(state.revision.as_str());
        if moved && !stale.contains(store) {
            stale.push(*store);
        }
    }
    stale
}

/// `persona`'s value as shown: the mode in effect, or nothing when no layer
/// holds one (Gentle Shell then uses its default).
pub(super) fn persona_of(state: &ConfigState) -> Option<String> {
    state
        .value
        .as_ref()
        .and_then(|value| value["mode"].as_str())
        .map(str::to_string)
}

/// The notification policy a `notifications` read describes.
pub(super) fn policy_of(state: &ConfigState) -> Policy {
    let store = match state.status {
        ConfigStatus::Ok => FileState::Valid,
        ConfigStatus::Missing => FileState::Missing,
        ConfigStatus::Invalid => FileState::Invalid,
    };
    Policy::from_store(
        crate::features::alerts::policy_file(),
        store,
        state.value.as_ref(),
    )
}

impl SettingsView {
    /// Ask the active tab's Gentle Shell for the settings stores. Off the
    /// RPC path nothing is sent and the files stay the source.
    pub(super) fn ask_shell_config(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.read(cx).config_support() {
            ConfigSupport::Rpc => {}
            ConfigSupport::NotReady => {
                self.shell_config.awaiting = true;
                return;
            }
            ConfigSupport::Legacy | ConfigSupport::Unavailable => return,
        }
        self.shell_config.awaiting = false;
        let stores: Vec<ConfigStore> = SETTINGS_STORES
            .into_iter()
            .filter(|store| self.shell_config.stores.get(store) != Some(&StoreSource::Files))
            .collect();
        self.read_shell_stores(&stores, cx);
    }

    fn read_shell_stores(&mut self, stores: &[ConfigStore], cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        for store in stores {
            let asked = Asked::Read(*store);
            if self
                .shell_config
                .tickets
                .values()
                .any(|open| *open == asked)
            {
                continue;
            }
            if let Ok(ticket) = session.update(cx, |session, cx| session.config_get(*store, cx)) {
                self.shell_config.tickets.insert(ticket, asked);
            }
        }
    }

    /// The tab changed: a probe that answered, config answers and
    /// `config_changed` events for the settings stores.
    pub(super) fn shell_config_changed(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if self.shell_config.awaiting {
            if session.read(cx).config_support() != ConfigSupport::NotReady {
                self.ask_shell_config(cx);
            }
            return;
        }
        if self.shell_config.tickets.is_empty() && self.shell_config.stores.is_empty() {
            return;
        }
        let (outcomes, changes) = session.update(cx, |session, _| {
            (
                session.take_config_outcomes(),
                session.take_config_changes(),
            )
        });
        let mut adopted = false;
        let mut reread = Vec::new();
        for (ticket, outcome) in outcomes {
            match land(&mut self.shell_config, ticket, outcome) {
                Landed::Ignored | Landed::ReadFailed => {}
                Landed::Adopted => {
                    adopted = true;
                    self.notifications.note = None;
                }
                Landed::Files(_) => adopted = true,
                Landed::Conflict(store) => {
                    self.notifications.note = Some(
                        t("Not saved: notifications changed in Gentle Shell meanwhile. Showing the latest; choose again.")
                            .into(),
                    );
                    reread.push(store);
                }
                Landed::WriteFailed(message) => {
                    self.notifications.note =
                        Some(t("Not saved: {message}").replace("{message}", &message));
                }
            }
        }
        let changes: Vec<(ConfigStore, Option<String>)> = changes
            .into_iter()
            .filter(|change| SETTINGS_STORES.contains(&change.store))
            .map(|change| (change.store, change.revision))
            .collect();
        for store in stale_stores(&self.shell_config, &changes) {
            if !reread.contains(&store) {
                reread.push(store);
            }
        }
        if !reread.is_empty() {
            self.read_shell_stores(&reread, cx);
        }
        if adopted {
            self.adopt_shell(cx);
        }
    }

    /// The notification policy the section shows: Gentle Shell's when it
    /// serves the store, the file's otherwise.
    pub(super) fn notification_policy(&self) -> Policy {
        match self.shell_config.state(ConfigStore::Notifications) {
            Some(state) => policy_of(state),
            None => crate::features::alerts::policy(),
        }
    }

    /// Write one notification change: through Gentle Shell's
    /// `notifications` store (guarded by the revision it reported) when the
    /// active tab's Shell serves it, to notifications.json otherwise.
    pub(super) fn set_notification(
        &mut self,
        change: notifications::Change,
        cx: &mut Context<Self>,
    ) {
        let store = ConfigStore::Notifications;
        let (Some(session), Some(state)) = (self.session.clone(), self.shell_config.state(store))
        else {
            self.notifications.note = crate::features::alerts::apply(change).err().map(|e| {
                t("Could not write notifications.json: {error}").replace("{error}", &e.to_string())
            });
            cx.notify();
            return;
        };
        if self.shell_config.writing(store) {
            return;
        }
        let Some(revision) = state.write_revision(ConfigWriteTarget::Global) else {
            self.notifications.note = Some(t("Gentle Shell offered no way to save this.").into());
            cx.notify();
            return;
        };
        let revision = revision.to_string();
        let data = policy_of(state).store_data(change);
        let sent = NotificationsData::new(data)
            .map_err(|error| error.to_string())
            .and_then(|data| {
                session
                    .update(cx, |session, cx| {
                        session.config_set(ConfigWrite::Notifications(data), revision, cx)
                    })
                    .map_err(|failure| failure.message())
            });
        match sent {
            Ok(ticket) => {
                self.shell_config
                    .tickets
                    .insert(ticket, Asked::Write(store));
                self.notifications.note = None;
            }
            Err(message) => {
                self.notifications.note =
                    Some(t("Not saved: {message}").replace("{message}", &message))
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use gentle_core::notifications;
    use pi_rpc::desktop_rpc::{
        ConfigErrorCode, ConfigLayer, ConfigLayerSource, ConfigScope, ConfigState, ConfigStatus,
        ConfigStore, ConfigWriteTarget, ConfigWriteTargetState,
    };
    use serde_json::json;

    use super::{
        Asked, ConfigFailure, ConfigOutcome, Landed, ShellConfig, StoreSource, land, persona_of,
        policy_of, stale_stores,
    };

    fn read(store: ConfigStore, revision: &str, value: serde_json::Value) -> ConfigState {
        ConfigState {
            store,
            scope: store.scope(),
            read_only: false,
            status: ConfigStatus::Ok,
            revision: revision.into(),
            value: Some(value),
            layers: vec![ConfigLayer {
                source: ConfigLayerSource::Global,
                status: ConfigStatus::Ok,
            }],
            write_targets: Some(vec![ConfigWriteTargetState {
                target: ConfigWriteTarget::Global,
                status: ConfigStatus::Ok,
                write_revision: format!("w-{revision}"),
            }]),
        }
    }

    /// Gentle Shell 5.0.0 answers the store: it is shown from the answer.
    /// 4.9.0 answers `unknown_store`: the store goes back to its file.
    #[test]
    fn a_served_store_is_adopted_and_an_unknown_one_keeps_the_file() {
        let mut shell = ShellConfig::default();
        shell.tickets.insert(1, Asked::Read(ConfigStore::Persona));
        shell.tickets.insert(2, Asked::Read(ConfigStore::Banner));
        let persona = read(ConfigStore::Persona, "r1", json!({ "mode": "neutral" }));
        assert_eq!(
            land(&mut shell, 1, ConfigOutcome::Read(persona.clone())),
            Landed::Adopted
        );
        assert_eq!(
            persona_of(shell.state(ConfigStore::Persona).unwrap()).as_deref(),
            Some("neutral")
        );
        assert_eq!(persona.scope, ConfigScope::GlobalProject);

        let unknown = ConfigOutcome::Failed(ConfigFailure::Rejected {
            code: ConfigErrorCode::UnknownStore,
            message: "Unknown Desktop config store".into(),
        });
        assert_eq!(
            land(&mut shell, 2, unknown),
            Landed::Files(ConfigStore::Banner)
        );
        assert_eq!(
            shell.stores.get(&ConfigStore::Banner),
            Some(&StoreSource::Files)
        );
        assert_eq!(
            land(&mut shell, 2, ConfigOutcome::Failed(ConfigFailure::Timeout)),
            Landed::Ignored
        );
    }

    #[test]
    fn a_conflicting_write_rereads_and_other_failures_say_why() {
        let mut shell = ShellConfig::default();
        shell
            .tickets
            .insert(5, Asked::Write(ConfigStore::Notifications));
        assert!(shell.writing(ConfigStore::Notifications));
        let conflict = ConfigOutcome::Failed(ConfigFailure::Rejected {
            code: ConfigErrorCode::RevisionConflict,
            message: "Config changed since expectedRevision was read".into(),
        });
        assert_eq!(
            land(&mut shell, 5, conflict),
            Landed::Conflict(ConfigStore::Notifications)
        );
        shell
            .tickets
            .insert(6, Asked::Write(ConfigStore::Notifications));
        assert!(matches!(
            land(&mut shell, 6, ConfigOutcome::Failed(ConfigFailure::Timeout)),
            Landed::WriteFailed(_)
        ));
    }

    #[test]
    fn only_stores_shown_from_the_shell_are_reread_when_they_move() {
        let mut shell = ShellConfig::default();
        shell.stores.insert(
            ConfigStore::Notifications,
            StoreSource::Rpc(read(ConfigStore::Notifications, "r1", json!({}))),
        );
        shell.stores.insert(ConfigStore::Banner, StoreSource::Files);
        let changes = vec![
            (ConfigStore::Notifications, Some("r1".to_string())),
            (ConfigStore::Banner, Some("r9".to_string())),
        ];
        assert!(stale_stores(&shell, &changes).is_empty(), "own echo");
        let changes = vec![
            (ConfigStore::Notifications, Some("r2".to_string())),
            (ConfigStore::Notifications, None),
        ];
        assert_eq!(
            stale_stores(&shell, &changes),
            vec![ConfigStore::Notifications]
        );
    }

    /// The notifications read keeps the file marker; the switch writes it
    /// back unchanged, which keeps the user's sound file.
    #[test]
    fn notifications_round_trip_the_file_marker() {
        let state = read(
            ConfigStore::Notifications,
            "r1",
            json!({
                "enabled": true,
                "audio": { "backend": "auto", "minimumIntervalMs": 1000, "coalesceWindowMs": 300,
                           "events": { "agent.completed": "file" } }
            }),
        );
        let policy = policy_of(&state);
        assert!(policy.plays(notifications::Alert::TurnEnded));
        let data = policy.store_data(notifications::Change::Enabled(false));
        assert_eq!(data["audio"]["events"]["agent.completed"], "file");
        assert!(data.get("schema").is_none());
    }
}
