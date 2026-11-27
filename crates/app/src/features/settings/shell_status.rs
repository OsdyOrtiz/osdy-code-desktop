//! Gentle Shell's own status (`get_status`): the facts `/gentle:status`
//! prints for this project (persona, the `gentle-ai` dev-binary override,
//! managed assets, model routing) and whether it warns, shown in General ›
//! Gentle Shell. Read from the active agent tab's Shell when it advertises
//! the method; any other Shell shows no status, as before.

use pi_rpc::desktop_rpc::{
    AssetStatus, DesktopMethod, DevBinaryState, ModelConfigPresence, RoutingState, ShellStatus,
    StatusSeverity,
};

use super::SettingsView;
use crate::features::session::{ReadFailure, ReadOutcome, ReadRequest, ReadSupport, ReadTicket};
use crate::shared::i18n::t;
use gpui_kit::Context;

/// The status last answered, and the read in flight.
#[derive(Default)]
pub(super) struct ShellStatusRead {
    pub(super) status: Option<ShellStatus>,
    pending: Option<ReadTicket>,
    /// Asked while the tab was still starting (or its Shell restarted
    /// before answering): asked again once the tab decides.
    awaiting: bool,
}

impl SettingsView {
    /// Ask the active agent tab's Gentle Shell for its status. A Shell that
    /// does not advertise `get_status` is never asked, and nothing shows.
    pub(super) fn ask_shell_status(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if self.shell_status.pending.is_some() {
            return;
        }
        match session.read(cx).read_support(DesktopMethod::GetStatus) {
            ReadSupport::Rpc => {}
            ReadSupport::NotReady => {
                self.shell_status.awaiting = true;
                return;
            }
            ReadSupport::Legacy => {
                self.shell_status.awaiting = false;
                self.shell_status.status = None;
                return;
            }
        }
        self.shell_status.awaiting = false;
        if let Ok(ticket) = session.update(cx, |session, cx| session.read(ReadRequest::Status, cx))
        {
            self.shell_status.pending = Some(ticket);
        }
    }

    /// The tab changed: ask once it is ready, or take the answer once it
    /// arrived. A refusal (`status_unavailable`) or failure shows nothing.
    pub(super) fn shell_status_changed(&mut self, cx: &mut Context<Self>) {
        let Some(ticket) = self.shell_status.pending else {
            if self.shell_status.awaiting {
                self.ask_shell_status(cx);
            }
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        let Some(outcome) = session.update(cx, |session, _| session.take_read(ticket)) else {
            return;
        };
        self.shell_status.pending = None;
        match outcome {
            ReadOutcome::Status(status) => self.shell_status.status = Some(status),
            ReadOutcome::Failed(ReadFailure::Dropped) => self.shell_status.awaiting = true,
            _ => self.shell_status.status = None,
        }
        cx.notify();
    }
}

/// Whether the status warns, as `/gentle:status` would.
pub(super) fn warns(status: &ShellStatus) -> bool {
    status.severity == StatusSeverity::Warning
}

/// The status rows, in `/gentle:status`'s order.
pub(super) fn status_rows(status: &ShellStatus) -> Vec<(&'static str, String)> {
    let dev_binary = match (status.dev_binary.state, &status.dev_binary.sha256_prefix) {
        (DevBinaryState::Inactive, _) => t("not overridden").to_string(),
        (DevBinaryState::Active, Some(prefix)) => {
            t("override in use (sha256 {prefix}…)").replace("{prefix}", prefix)
        }
        (DevBinaryState::Active, None) => t("override in use").to_string(),
        (DevBinaryState::Invalid, _) => t("override declared but unusable").to_string(),
    };
    let models = t("models.json {config} · routing {routing}")
        .replace(
            "{config}",
            match status.model_config.global {
                ModelConfigPresence::Present => t("present"),
                ModelConfigPresence::Missing => t("missing"),
            },
        )
        .replace(
            "{routing}",
            match status.model_config.routing {
                RoutingState::Valid => t("valid"),
                RoutingState::Invalid => t("invalid"),
                RoutingState::Missing => t("not saved"),
            },
        );
    vec![
        (t("Persona"), status.persona.clone()),
        (t("Dev binary"), dev_binary),
        (
            t("Delegation assets"),
            assets("delegation", &status.assets.delegation),
        ),
        (t("Review assets"), assets("review", &status.assets.review)),
        (t("Models"), models),
    ]
}

/// One owner's assets: what drifted (and how to refresh it), what the user
/// replaced, and agents that shadow the package's.
fn assets(owner: &str, asset: &AssetStatus) -> String {
    let mut parts = Vec::new();
    if asset.stale > 0 {
        parts.push(
            t("{count} stale (/gentle:install-{owner} --force)")
                .replace("{count}", &asset.stale.to_string())
                .replace("{owner}", owner),
        );
    }
    if asset.user_overrides > 0 {
        parts.push(
            t("{count} replaced by you").replace("{count}", &asset.user_overrides.to_string()),
        );
    }
    if asset.active_agent_overrides > 0 {
        parts.push(
            t("{count} shadowed by your agents")
                .replace("{count}", &asset.active_agent_overrides.to_string()),
        );
    }
    if parts.is_empty() {
        t("up to date").to_string()
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use pi_rpc::desktop_rpc::{DesktopRecord, SCHEMA, ShellStatus, decode_payload};

    use super::{status_rows, warns};

    /// Decode through the adapter's own parser, on the real wire shape.
    fn status(data: serde_json::Value) -> ShellStatus {
        let line = serde_json::json!({
            "schema": SCHEMA, "kind": "response", "requestId": "desktop-95", "ok": true, "data": data,
        })
        .to_string();
        match decode_payload(&line).unwrap() {
            DesktopRecord::Response(response) => response.status().unwrap(),
            other => panic!("expected a response, got {other:?}"),
        }
    }

    #[test]
    fn the_spec_example_reads_as_healthy() {
        let status = status(serde_json::json!({
            "package": "active",
            "odd": "active",
            "persona": "gentleman",
            "devBinary": { "state": "inactive", "sha256Prefix": null },
            "assets": {
                "delegation": { "stale": 0, "userOverrides": 1, "activeAgentOverrides": 0 },
                "review": { "stale": 0, "userOverrides": 0, "activeAgentOverrides": 0 }
            },
            "modelConfig": { "global": "present", "routing": "valid" },
            "severity": "info"
        }));
        assert!(!warns(&status));
        assert_eq!(
            status_rows(&status),
            [
                ("Persona", "gentleman".to_string()),
                ("Dev binary", "not overridden".to_string()),
                ("Delegation assets", "1 replaced by you".to_string()),
                ("Review assets", "up to date".to_string()),
                ("Models", "models.json present · routing valid".to_string()),
            ]
        );
    }

    #[test]
    fn a_warning_names_what_drifted() {
        let status = status(serde_json::json!({
            "package": "active", "odd": "active", "persona": "neutral",
            "devBinary": { "state": "active", "sha256Prefix": "0123456789abcdef" },
            "assets": {
                "delegation": { "stale": 2, "userOverrides": 0, "activeAgentOverrides": 0 },
                "review": { "stale": 0, "userOverrides": 0, "activeAgentOverrides": 1 }
            },
            "modelConfig": { "global": "missing", "routing": "invalid" },
            "severity": "warning"
        }));
        assert!(warns(&status));
        let rows = status_rows(&status);
        assert_eq!(rows[1].1, "override in use (sha256 0123456789abcdef…)");
        assert_eq!(rows[2].1, "2 stale (/gentle:install-delegation --force)");
        assert_eq!(rows[3].1, "1 shadowed by your agents");
        assert_eq!(rows[4].1, "models.json missing · routing invalid");
    }
}
