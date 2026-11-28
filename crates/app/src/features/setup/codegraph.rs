//! Codegraph's local CLI and its registration in Gentle Shell's MCP config.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::probe::{output_of, run_logged};
use super::{Detail, Fix, Log, Status, gentle, refresh_paths, remote};
use crate::features::backend;
use crate::shared::i18n::t;

const SERVER: &str = "codegraph";
const PACKAGE: &str = "@colbymchenry/codegraph";
pub const LOCAL_ONLY: &str = "Codegraph setup is only available for local agents.";
pub const SUMMARY: &str = "Code graph for the agent: symbols, references and calls (MCP).";

fn registered(mcp_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(mcp_json) else {
        return false;
    };
    let server = &value["mcpServers"][SERVER];
    server.is_object() && server["enabled"] != false
}

fn is_registered() -> bool {
    std::fs::read_to_string(pi_rpc::resolve_homes().agent.join("mcp.json"))
        .is_ok_and(|text| registered(&text))
}

fn version(text: &str) -> Option<&str> {
    static VERSION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\Av?[0-9]+\.[0-9]+\.[0-9]+(?:-[a-zA-Z0-9.-]+)?(?:\+[a-zA-Z0-9.-]+)?\z")
            .unwrap()
    });
    let text = text.trim();
    VERSION.is_match(text).then_some(text)
}

#[derive(Debug)]
pub struct CodegraphStatus {
    pub cli: Option<PathBuf>,
    pub version: Option<String>,
    /// Registration, not a live connection.
    pub connected: bool,
}

fn require_local() -> Result<(), &'static str> {
    if remote::backend().is_none() {
        Ok(())
    } else {
        Err(LOCAL_ONLY)
    }
}

fn cli() -> (Option<PathBuf>, Option<String>) {
    let path = pi_rpc::find_program(SERVER);
    let found = path
        .as_deref()
        .and_then(|path| output_of(path, &["--version"]))
        .and_then(|output| version(&output).map(str::to_owned));
    (path, found)
}

/// Blocking: probe the CLI and config, without starting the MCP server.
pub fn status() -> Result<CodegraphStatus, &'static str> {
    require_local()?;
    let (cli, version) = cli();
    Ok(CodegraphStatus {
        cli,
        version,
        connected: is_registered(),
    })
}

pub(super) fn check() -> (Status, Fix) {
    let status = match status() {
        Ok(status) => status,
        Err(message) => return (Status::Missing(message.into()), Fix::None),
    };
    match (status.version, status.connected) {
        (Some(version), true) => (Status::Ok(format!("Codegraph {version}").into()), Fix::None),
        (Some(_), false) => (Status::Missing("Not registered".into()), Fix::Auto),
        (None, _) => (
            Status::Missing(Detail::from(if status.cli.is_some() {
                "Codegraph CLI did not report a version."
            } else {
                "Not installed"
            })),
            Fix::Auto,
        ),
    }
}

fn launcher() -> Result<pi_rpc::LaunchSpec, String> {
    require_local().map_err(t)?;
    backend::home_shell_spec().map_err(|_| t("Install Gentle Shell first.").into())
}

/// Install a runnable CLI if missing, then register it if not already enabled.
pub fn install(log: &Log) -> Result<(), String> {
    let mut spec = launcher()?;
    if cli().1.is_none() {
        let npm = pi_rpc::find_program("npm")
            .ok_or_else(|| t("npm wasn't found. Install Node.js first."))?;
        let mut command = pi_rpc::command(&npm);
        command.args(["i", "-g", PACKAGE]);
        if let Some(prefix) = gentle::install_prefix(&npm) {
            command.arg("--prefix").arg(prefix);
        }
        log(t("Installing Codegraph with npm…").into());
        run_logged(command, "npm i -g @colbymchenry/codegraph", log)?;
        refresh_paths();
        if cli().1.is_none() {
            return Err(t("Codegraph was installed, but its CLI did not report a version.").into());
        }
        // An app-owned npm bin folder may have just joined the launcher's PATH.
        spec = launcher()?;
    }
    register(&spec, log)
}

/// Register an existing CLI, without installing anything.
pub fn connect(log: &Log) -> Result<(), String> {
    let spec = launcher()?;
    if cli().1.is_none() {
        return Err(t("Codegraph CLI is unavailable. Install Codegraph first.").into());
    }
    register(&spec, log)
}

fn register(spec: &pi_rpc::LaunchSpec, log: &Log) -> Result<(), String> {
    if is_registered() {
        return Ok(());
    }
    let mut args = vec!["mcp", "add", SERVER, "--"];
    if cfg!(windows) {
        args.extend(["cmd", "/c"]);
    }
    args.extend([SERVER, "serve", "--mcp"]);
    log(t("Registering Codegraph with the agent…").into());
    run_logged(spec.launcher_command(args), "gentle-shell mcp add", log)?;
    if !is_registered() {
        return Err(t("Gentle Shell did not enable Codegraph in mcp.json.").into());
    }
    Ok(())
}

/// Remove only the MCP registration; leave the installed CLI alone.
pub fn disconnect(log: &Log) -> Result<(), String> {
    let spec = launcher()?;
    log(t("Disconnecting Codegraph from the agent…").into());
    run_logged(
        spec.launcher_command(["mcp", "remove", SERVER]),
        "gentle-shell mcp remove",
        log,
    )?;
    if is_registered() {
        return Err(t("Codegraph is still enabled in mcp.json.").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codegraph_registration_matches_enabled_server_objects() {
        assert!(registered(
            r#"{"mcpServers":{"maestro":{},"codegraph":{"command":"codegraph","args":["serve","--mcp"]}}}"#
        ));
        assert!(registered(
            r#"{"mcpServers":{"codegraph":{"enabled":true}}}"#
        ));
        assert!(!registered(
            r#"{"mcpServers":{"codegraph":{"enabled":false}}}"#
        ));
        assert!(!registered(r#"{"mcpServers":{"maestro":{}}}"#));
        assert!(!registered(r#"{"mcpServers":{"codegraph":"codegraph"}}"#));
        assert!(!registered("invalid JSON"));
    }

    #[test]
    fn codegraph_version_is_a_trimmed_version_line() {
        assert_eq!(version("  1.4.1\n"), Some("1.4.1"));
        assert_eq!(version("v1.4.1-beta.2"), Some("v1.4.1-beta.2"));
        for output in [
            "",
            "command not found",
            "1",
            "1.4",
            "1.4.x",
            "1.4.1\nwarning",
            "1.4.1 not a version",
        ] {
            assert_eq!(version(output), None, "{output:?}");
        }
    }
}
