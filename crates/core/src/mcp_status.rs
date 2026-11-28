//! Read-only MCP status received from the active agent.

pub const WIDGET_KEY: &str = "osdy-gentle:mcp-status";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Connecting,
    Ready,
    Error,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Auth,
    Timeout,
    Transport,
    Config,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub state: State,
    pub tools: Option<u64>,
    pub error: Option<ErrorKind>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Feed {
    Unavailable,
    Available(Vec<Server>),
}

pub fn parse(line: &str) -> Option<Feed> {
    let widget: serde_json::Value = serde_json::from_str(line).ok()?;
    if widget["v"].as_u64() != Some(1) {
        return None;
    }
    if !widget["available"].as_bool()? {
        return Some(Feed::Unavailable);
    }

    let servers = widget["servers"]
        .as_array()?
        .iter()
        .filter_map(|record| {
            let state = match record["state"].as_str()? {
                "connecting" => State::Connecting,
                "ready" => State::Ready,
                "error" => State::Error,
                "disabled" => State::Disabled,
                _ => return None,
            };
            let error = match record["error"].as_str() {
                Some("auth") => Some(ErrorKind::Auth),
                Some("timeout") => Some(ErrorKind::Timeout),
                Some("transport") => Some(ErrorKind::Transport),
                Some("config") => Some(ErrorKind::Config),
                Some("unknown") => Some(ErrorKind::Unknown),
                _ => None,
            };
            Some(Server {
                name: record["name"].as_str()?.to_owned(),
                state,
                tools: record["tools"].as_u64(),
                error,
            })
        })
        .collect();
    Some(Feed::Available(servers))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    Unavailable,
    Connecting,
    Ready,
    Disabled,
    Error(Option<ErrorKind>),
}

/// Merge configured names (with an explicit disabled marker) and the live
/// feed. Configuration alone never shows a server as connected.
pub fn rows<'a>(
    configured: &'a [(String, bool)],
    feed: Option<&'a Feed>,
) -> Vec<(&'a str, RowStatus)> {
    let mut rows: std::collections::BTreeMap<_, _> = configured
        .iter()
        .map(|(name, disabled)| {
            let status = if *disabled {
                RowStatus::Disabled
            } else {
                RowStatus::Unavailable
            };
            (name.as_str(), status)
        })
        .collect();
    if let Some(Feed::Available(servers)) = feed {
        for server in servers {
            let status = match server.state {
                State::Connecting => RowStatus::Connecting,
                State::Ready => RowStatus::Ready,
                State::Error => RowStatus::Error(server.error),
                State::Disabled => RowStatus::Disabled,
            };
            rows.insert(&server.name, status);
        }
    }
    rows.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_requires_the_version_flag_and_available_list() {
        assert_eq!(
            parse(r#"{"v":1,"available":false}"#),
            Some(Feed::Unavailable)
        );
        assert_eq!(
            parse(r#"{"v":1,"available":true,"servers":[]}"#),
            Some(Feed::Available(vec![]))
        );
        for invalid in [
            "not JSON",
            r#"{"v":2,"available":false}"#,
            r#"{"v":1,"available":"false"}"#,
            r#"{"v":1,"servers":[]}"#,
            r#"{"v":1,"available":true}"#,
            r#"{"v":1,"available":true,"servers":{}}"#,
        ] {
            assert_eq!(parse(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn accepts_only_known_states_and_error_kinds() {
        let Some(Feed::Available(servers)) = parse(
            r#"{"v":1,"available":true,"servers":[
                {"name":"a","state":"ready","tools":0,"url":"secret"},
                {"name":"b","state":"connecting","tools":-1},
                {"name":"c","state":"disabled","tools":1.2},
                {"name":"a1","state":"error","error":"auth"},
                {"name":"a2","state":"error","error":"timeout"},
                {"name":"a3","state":"error","error":"transport"},
                {"name":"a4","state":"error","error":"config"},
                {"name":"a5","state":"error","error":"unknown"},
                {"name":"a6","state":"error","error":"raw"},
                {"name":"a7","state":"error","error":{"kind":"auth"}},
                {"name":"drop","state":"READY"},
                {"name":false,"state":"ready"}
            ]}"#,
        ) else {
            panic!("expected available feed");
        };
        assert_eq!(servers.len(), 10);
        assert_eq!(
            servers[0],
            Server {
                name: "a".into(),
                state: State::Ready,
                tools: Some(0),
                error: None,
            }
        );
        assert_eq!(servers[1].state, State::Connecting);
        assert_eq!(servers[2].state, State::Disabled);
        assert!(servers[1..].iter().all(|server| server.tools.is_none()));
        let expected = [
            Some(ErrorKind::Auth),
            Some(ErrorKind::Timeout),
            Some(ErrorKind::Transport),
            Some(ErrorKind::Config),
            Some(ErrorKind::Unknown),
            None,
            None,
        ];
        for (server, error) in servers[3..].iter().zip(expected) {
            assert_eq!(server.state, State::Error);
            assert_eq!(server.error, error);
        }
    }

    #[test]
    fn unavailable_or_empty_feeds_never_imply_connectivity() {
        let configured = [("active".into(), false), ("off".into(), true)];
        for feed in [None, Some(Feed::Unavailable), Some(Feed::Available(vec![]))] {
            assert_eq!(
                rows(&configured, feed.as_ref()),
                [
                    ("active", RowStatus::Unavailable),
                    ("off", RowStatus::Disabled)
                ]
            );
        }
        assert!(rows(&[], None).is_empty());
    }

    #[test]
    fn rows_merge_names_and_prefer_observed_state_over_configuration() {
        let configured = [("z".into(), false), ("a".into(), true), ("b".into(), false)];
        let feed = parse(
            r#"{"v":1,"available":true,"servers":[
                {"name":"a","state":"ready"},
                {"name":"c","state":"connecting"},
                {"name":"d","state":"disabled"},
                {"name":"e","state":"error","error":"timeout"},
                {"name":"f","state":"error"}
            ]}"#,
        );
        assert_eq!(
            rows(&configured, feed.as_ref()),
            [
                ("a", RowStatus::Ready),
                ("b", RowStatus::Unavailable),
                ("c", RowStatus::Connecting),
                ("d", RowStatus::Disabled),
                ("e", RowStatus::Error(Some(ErrorKind::Timeout))),
                ("f", RowStatus::Error(None)),
                ("z", RowStatus::Unavailable),
            ]
        );
    }
}
