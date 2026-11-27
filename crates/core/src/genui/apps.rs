//! The host side of MCP Apps (SEP-1865, protocol `2026-01-26`): JSON-RPC 2.0
//! between a page (the "View") and the app. The page posts requests and
//! notifications; [`Bridge::receive`] answers what it can by itself
//! (`ui/initialize`, `ping`, display modes, the methods a `gentle_ui` page
//! has no server for) and hands the rest to the app as an [`Ask`], which
//! the app answers with [`reply`]/[`error`]. Notifications to the page wait
//! until it says `ui/notifications/initialized`, as the spec requires.
//!
//! Pure: no I/O. The app moves the strings in and out of the web view.

use serde_json::{Map, Value, json};

pub const PROTOCOL_VERSION: &str = "2026-01-26";

/// JSON-RPC error codes.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
/// MCP Apps' "implementation-defined" error (denied, failed).
pub const DENIED: i64 = -32000;

/// The longest link `ui/open-link` accepts.
const MAX_URL: usize = 2048;
/// The longest `ui/message` text accepted, in bytes.
pub const MAX_MESSAGE: usize = 16 * 1024;

/// What the page asked that the app must do. Requests carry the JSON-RPC
/// id to answer with [`reply`] or [`error`].
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// The handshake finished: send the tool input and result now. Any
    /// notifications queued meanwhile come back with it.
    Ready,
    /// `ui/message`: a user message for the agent.
    Message { id: Value, text: String },
    /// `ui/update-model-context`: what the agent should know from now on
    /// (replaces the previous context).
    Context {
        id: Value,
        content: Vec<Value>,
        structured: Option<Value>,
    },
    /// `ui/open-link` with an http(s) URL.
    OpenLink { id: Value, url: String },
    /// `ui/notifications/size-changed` (px).
    Resize {
        width: Option<f64>,
        height: Option<f64>,
    },
    /// `ui/notifications/request-teardown`: the page asks to be closed.
    Close,
    /// The page answered the `ui/resource-teardown` request.
    TornDown,
    /// `notifications/message`: a log line.
    Log { level: String, data: Value },
}

/// One message in: what to post back to the page right away, and what the
/// app must do.
#[derive(Debug, Default, PartialEq)]
pub struct Step {
    pub post: Vec<Value>,
    pub ask: Option<Ask>,
}

pub struct Bridge {
    host_info: Value,
    capabilities: Value,
    /// The host context as last sent (or to send at `ui/initialize`).
    context: Map<String, Value>,
    /// The page finished the handshake.
    ready: bool,
    /// Notifications waiting for the handshake.
    queued: Vec<Value>,
    next_id: u64,
    /// The id of the teardown request in flight.
    teardown: Option<u64>,
}

impl Bridge {
    /// `context` is the MCP Apps `HostContext` object.
    pub fn new(name: &str, version: &str, capabilities: Value, context: Value) -> Self {
        Self {
            host_info: json!({ "name": name, "version": version }),
            capabilities,
            context: context.as_object().cloned().unwrap_or_default(),
            ready: false,
            queued: Vec::new(),
            next_id: 1,
            teardown: None,
        }
    }

    /// A new page instance loads: it handshakes again.
    pub fn reset(&mut self) {
        self.ready = false;
        self.queued.clear();
        self.teardown = None;
    }

    pub fn ready(&self) -> bool {
        self.ready
    }

    /// A message the page posted (its JSON text).
    pub fn receive(&mut self, raw: &str) -> Step {
        let Ok(message) = serde_json::from_str::<Value>(raw) else {
            return Step::post(error(&Value::Null, PARSE_ERROR, "Parse error"));
        };
        let id = message.get("id").filter(|id| is_id(id)).cloned();
        let method = message.get("method").and_then(Value::as_str);
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return match id {
                Some(id) if method.is_some() => Step::post(error(
                    &id,
                    INVALID_REQUEST,
                    "Invalid Request: jsonrpc must be \"2.0\"",
                )),
                _ => Step::default(),
            };
        }
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match (method, id) {
            (Some(method), Some(id)) => self.request(method, id, &params),
            (Some(method), None) => self.notification(method, &params),
            (None, Some(id)) => {
                // A response: only the teardown request is ever in flight.
                if id.as_u64().is_some() && id.as_u64() == self.teardown {
                    self.teardown = None;
                    Step::ask(Ask::TornDown)
                } else {
                    Step::default()
                }
            }
            (None, None) => Step::default(),
        }
    }

    fn request(&mut self, method: &str, id: Value, params: &Value) -> Step {
        match method {
            // One protocol version is supported: whatever the page asks for,
            // the answer is it (the page decides whether it can go on). A new
            // handshake is a new page instance (a reload): it is ready only
            // once it says so.
            "ui/initialize" => {
                self.ready = false;
                Step::post(reply(
                    &id,
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "hostInfo": self.host_info,
                        "hostCapabilities": self.capabilities,
                        "hostContext": Value::Object(self.context.clone()),
                    }),
                ))
            }
            "ping" => Step::post(reply(&id, json!({}))),
            "ui/request-display-mode" => {
                // The page is shown one way; the answer is the mode it has.
                let mode = self
                    .context
                    .get("displayMode")
                    .cloned()
                    .unwrap_or(json!("inline"));
                Step::post(reply(&id, json!({ "mode": mode })))
            }
            "ui/message" => match message_text(params) {
                Ok(text) => Step::ask(Ask::Message { id, text }),
                Err(why) => Step::post(error(&id, INVALID_PARAMS, &why)),
            },
            "ui/update-model-context" => {
                let content = match params.get("content") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(Value::Array(blocks)) => blocks.clone(),
                    Some(_) => {
                        return Step::post(error(
                            &id,
                            INVALID_PARAMS,
                            "content must be an array of content blocks",
                        ));
                    }
                };
                let structured = params
                    .get("structuredContent")
                    .filter(|v| !v.is_null())
                    .cloned();
                if structured.as_ref().is_some_and(|s| !s.is_object()) {
                    return Step::post(error(
                        &id,
                        INVALID_PARAMS,
                        "structuredContent must be an object",
                    ));
                }
                Step::ask(Ask::Context {
                    id,
                    content,
                    structured,
                })
            }
            "ui/open-link" => match params["url"].as_str().map(str::trim) {
                Some(url) if is_web_url(url) => Step::ask(Ask::OpenLink {
                    id,
                    url: url.to_string(),
                }),
                _ => Step::post(error(
                    &id,
                    INVALID_PARAMS,
                    "Invalid URL: only http(s) links open",
                )),
            },
            "tools/call"
            | "tools/list"
            | "resources/read"
            | "resources/list"
            | "resources/templates/list"
            | "prompts/list" => Step::post(error(
                &id,
                METHOD_NOT_FOUND,
                &format!(
                    "{method} is not available: this view was shown by the agent's gentle_ui tool \
                     and has no MCP server (the host does not advertise serverTools/serverResources)"
                ),
            )),
            _ => Step::post(error(
                &id,
                METHOD_NOT_FOUND,
                &format!("Method not found: {method}"),
            )),
        }
    }

    fn notification(&mut self, method: &str, params: &Value) -> Step {
        match method {
            "ui/notifications/initialized" => {
                self.ready = true;
                Step {
                    post: std::mem::take(&mut self.queued),
                    ask: Some(Ask::Ready),
                }
            }
            "ui/notifications/size-changed" => {
                let size = |key: &str| params[key].as_f64().filter(|v| v.is_finite() && *v >= 0.);
                let (width, height) = (size("width"), size("height"));
                if width.is_none() && height.is_none() {
                    return Step::default();
                }
                Step::ask(Ask::Resize { width, height })
            }
            "ui/notifications/request-teardown" => Step::ask(Ask::Close),
            "notifications/message" => Step::ask(Ask::Log {
                level: params["level"].as_str().unwrap_or("info").to_string(),
                data: params.get("data").cloned().unwrap_or(Value::Null),
            }),
            // Everything else (cancellations, unknown notifications) is
            // ignored, as JSON-RPC allows.
            _ => Step::default(),
        }
    }

    /// A notification for the page: returned to post now, or queued until
    /// the handshake finishes (`None`).
    pub fn notify(&mut self, method: &str, params: Value) -> Option<Value> {
        let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if self.ready {
            Some(message)
        } else {
            self.queued.push(message);
            None
        }
    }

    /// The capabilities the next `ui/initialize` answers with (the page's
    /// policy changed, so the page reloads and handshakes again).
    pub fn set_capabilities(&mut self, capabilities: Value) {
        self.capabilities = capabilities;
    }

    /// The host context changed: the fields that differ, as a
    /// `ui/notifications/host-context-changed`. Nothing before the
    /// handshake: `ui/initialize` sends the new context anyway.
    pub fn set_context(&mut self, context: Value) -> Option<Value> {
        let context = context.as_object().cloned().unwrap_or_default();
        let changed: Map<String, Value> = context
            .iter()
            .filter(|(key, value)| self.context.get(*key) != Some(value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        self.context = context;
        if changed.is_empty() || !self.ready {
            return None;
        }
        Some(json!({
            "jsonrpc": "2.0",
            "method": "ui/notifications/host-context-changed",
            "params": Value::Object(changed),
        }))
    }

    /// The `ui/resource-teardown` request to post before the page goes;
    /// `None` when the page never finished the handshake (nothing to warn).
    pub fn teardown(&mut self, reason: &str) -> Option<Value> {
        if !self.ready {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.teardown = Some(id);
        Some(json!({
            "jsonrpc": "2.0", "id": id, "method": "ui/resource-teardown",
            "params": { "reason": reason },
        }))
    }
}

impl Step {
    fn post(message: Value) -> Self {
        Self {
            post: vec![message],
            ask: None,
        }
    }

    fn ask(ask: Ask) -> Self {
        Self {
            post: Vec::new(),
            ask: Some(ask),
        }
    }
}

pub fn reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The capabilities a `gentle_ui` page gets: links (after the user
/// confirms), text messages, model context and logging; no server tools or
/// resources. `csp` is the policy actually applied.
pub fn capabilities(csp: Value) -> Value {
    json!({
        "openLinks": {},
        "logging": {},
        "message": { "text": {} },
        "updateModelContext": { "text": {}, "structuredContent": {} },
        "sandbox": { "csp": csp },
    })
}

/// `ui/notifications/tool-input` params.
pub fn tool_input(arguments: &Value) -> Value {
    json!({ "arguments": arguments })
}

/// `ui/notifications/tool-result` params: a `CallToolResult` with the text
/// the model saw and the page's data.
pub fn tool_result(text: &str, structured: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
    })
}

/// The text of a `ui/message` (role `user`; text blocks joined, other
/// blocks named).
fn message_text(params: &Value) -> Result<String, String> {
    if params["role"].as_str() != Some("user") {
        return Err("Invalid message format: role must be \"user\"".into());
    }
    let blocks: Vec<&Value> = match &params["content"] {
        Value::Array(blocks) => blocks.iter().collect(),
        block @ Value::Object(_) => vec![block],
        _ => return Err("Invalid message format: content must be content blocks".into()),
    };
    let parts: Vec<String> = blocks
        .iter()
        .filter_map(|block| match block["type"].as_str()? {
            "text" => block["text"].as_str().map(str::to_string),
            other => Some(format!("[{other} omitted]")),
        })
        .filter(|part| !part.trim().is_empty())
        .collect();
    let text = parts.join("\n");
    if text.trim().is_empty() {
        return Err("Invalid message format: no text".into());
    }
    if text.len() > MAX_MESSAGE {
        return Err(format!(
            "Invalid message format: longer than {} KiB",
            MAX_MESSAGE / 1024
        ));
    }
    Ok(text)
}

fn is_id(id: &Value) -> bool {
    id.is_string() || id.is_i64() || id.is_u64()
}

fn is_web_url(url: &str) -> bool {
    url.len() <= MAX_URL
        && ["http://", "https://"].iter().any(|scheme| {
            url.get(..scheme.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
                && url.len() > scheme.len()
        })
        && !url.chars().any(char::is_whitespace)
}

/// The text the model gets for a page's model context: text blocks, then
/// the structured content as JSON.
pub fn context_text(content: &[Value], structured: Option<&Value>) -> String {
    let mut parts: Vec<String> = content
        .iter()
        .filter_map(|block| match block["type"].as_str()? {
            "text" => block["text"].as_str().map(str::to_string),
            other => Some(format!("[{other} omitted]")),
        })
        .collect();
    if let Some(structured) = structured {
        parts.push(structured.to_string());
    }
    parts.join("\n")
}

/// The app's look, as CSS colors (`#rrggbb` or `rgba(…)`) and font
/// families, for the page's theme variables.
pub struct Palette<'a> {
    pub dark: bool,
    /// The page's own background (the card body).
    pub background: &'a str,
    pub raised: &'a str,
    pub well: &'a str,
    pub fg: &'a str,
    pub muted: &'a str,
    pub faint: &'a str,
    pub border: &'a str,
    pub border_strong: &'a str,
    pub accent: &'a str,
    pub info: &'a str,
    pub success: &'a str,
    pub danger: &'a str,
    pub warning: &'a str,
    pub font_sans: &'a str,
    pub font_mono: &'a str,
    /// The interface's rem (px), so text sizes follow the app's.
    pub rem: f64,
}

/// MCP Apps' standard theme variables (`HostContext.styles.variables`)
/// filled from the app's palette.
pub fn style_variables(p: &Palette) -> Map<String, Value> {
    let tint = |color: &str| format!("color-mix(in srgb, {color} 14%, transparent)");
    let px = |rem: f64| format!("{}px", (rem * p.rem * 100.).round() / 100.);
    let vars: Vec<(&str, String)> = vec![
        ("--color-background-primary", p.background.into()),
        ("--color-background-secondary", p.raised.into()),
        ("--color-background-tertiary", p.well.into()),
        ("--color-background-inverse", p.fg.into()),
        ("--color-background-ghost", "transparent".into()),
        ("--color-background-info", tint(p.info)),
        ("--color-background-danger", tint(p.danger)),
        ("--color-background-success", tint(p.success)),
        ("--color-background-warning", tint(p.warning)),
        ("--color-background-disabled", p.raised.into()),
        ("--color-text-primary", p.fg.into()),
        ("--color-text-secondary", p.muted.into()),
        ("--color-text-tertiary", p.faint.into()),
        ("--color-text-inverse", p.background.into()),
        ("--color-text-info", p.info.into()),
        ("--color-text-danger", p.danger.into()),
        ("--color-text-success", p.success.into()),
        ("--color-text-warning", p.warning.into()),
        ("--color-text-disabled", p.faint.into()),
        ("--color-text-ghost", p.faint.into()),
        ("--color-border-primary", p.border_strong.into()),
        ("--color-border-secondary", p.border.into()),
        ("--color-border-tertiary", p.border.into()),
        ("--color-border-inverse", p.fg.into()),
        ("--color-border-ghost", "transparent".into()),
        ("--color-border-info", p.info.into()),
        ("--color-border-danger", p.danger.into()),
        ("--color-border-success", p.success.into()),
        ("--color-border-warning", p.warning.into()),
        ("--color-border-disabled", p.border.into()),
        ("--color-ring-primary", p.accent.into()),
        ("--color-ring-secondary", p.muted.into()),
        ("--color-ring-inverse", p.background.into()),
        ("--color-ring-info", p.info.into()),
        ("--color-ring-danger", p.danger.into()),
        ("--color-ring-success", p.success.into()),
        ("--color-ring-warning", p.warning.into()),
        ("--font-sans", p.font_sans.into()),
        ("--font-mono", p.font_mono.into()),
        ("--font-weight-normal", "400".into()),
        ("--font-weight-medium", "500".into()),
        ("--font-weight-semibold", "600".into()),
        ("--font-weight-bold", "700".into()),
        ("--border-radius-xs", px(0.25)),
        ("--border-radius-sm", px(0.375)),
        ("--border-radius-md", px(0.5)),
        ("--border-radius-lg", px(0.75)),
        ("--border-radius-xl", px(1.)),
        ("--border-radius-full", "9999px".into()),
        ("--border-width-regular", "1px".into()),
        ("--shadow-hairline", format!("0 0 0 1px {}", p.border)),
        ("--shadow-sm", "0 1px 2px rgba(0,0,0,0.18)".into()),
        ("--shadow-md", "0 4px 12px rgba(0,0,0,0.22)".into()),
        ("--shadow-lg", "0 12px 32px rgba(0,0,0,0.28)".into()),
    ];
    let mut variables: Map<String, Value> = vars
        .into_iter()
        .map(|(key, value)| (key.to_string(), Value::String(value)))
        .collect();
    // Text and heading sizes with their line heights (rem).
    for (name, size, line) in [
        ("text-xs", 0.75, 1.0),
        ("text-sm", 0.8125, 1.25),
        ("text-md", 0.875, 1.375),
        ("text-lg", 1.0, 1.5),
        ("heading-xs", 0.875, 1.25),
        ("heading-sm", 1.0, 1.375),
        ("heading-md", 1.125, 1.5),
        ("heading-lg", 1.25, 1.625),
        ("heading-xl", 1.5, 1.875),
        ("heading-2xl", 1.875, 2.25),
        ("heading-3xl", 2.25, 2.625),
    ] {
        variables.insert(format!("--font-{name}-size"), Value::String(px(size)));
        variables.insert(
            format!("--font-{name}-line-height"),
            Value::String(px(line)),
        );
    }
    variables
}

/// The `HostContext` for a page: theme and variables, how it is shown
/// (`fullscreen`: in the app's expanded view, `width` fixed, height up to
/// `max_height`), locale and platform.
pub fn host_context(
    palette: &Palette,
    width: f64,
    max_height: f64,
    locale: Option<&str>,
    user_agent: &str,
) -> Value {
    let mut context = json!({
        "theme": if palette.dark { "dark" } else { "light" },
        "styles": { "variables": Value::Object(style_variables(palette)) },
        "displayMode": "fullscreen",
        "availableDisplayModes": ["fullscreen"],
        "containerDimensions": { "width": width.round(), "maxHeight": max_height.round() },
        "userAgent": user_agent,
        "platform": "desktop",
        "deviceCapabilities": { "touch": false, "hover": true },
        "toolInfo": { "tool": {
            "name": super::TOOL,
            "description": "Agent-written HTML shown in Osdy Code Desktop",
            "inputSchema": { "type": "object" },
        }},
    });
    if let Some(locale) = locale {
        context["locale"] = json!(locale);
    }
    context
}

/// A BCP 47 tag from a POSIX locale (`LC_ALL`/`LANG`, `en_US.UTF-8` →
/// `en-US`); `None` for `C`/`POSIX` or anything unreadable.
pub fn locale_tag(posix: &str) -> Option<String> {
    let base = posix.split(['.', '@']).next()?.trim();
    if base.is_empty() || base == "C" || base == "POSIX" {
        return None;
    }
    let tag = base.replace('_', "-");
    tag.split('-')
        .all(|part| {
            !part.is_empty() && part.len() <= 8 && part.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .then_some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge() -> Bridge {
        Bridge::new(
            "gentle-shell-gpui",
            "1.0.0",
            capabilities(json!({})),
            json!({ "theme": "dark", "displayMode": "inline" }),
        )
    }

    fn send(bridge: &mut Bridge, message: Value) -> Step {
        bridge.receive(&message.to_string())
    }

    fn handshake(bridge: &mut Bridge) {
        send(
            bridge,
            json!({"jsonrpc": "2.0", "id": 1, "method": "ui/initialize", "params": {
                "protocolVersion": PROTOCOL_VERSION, "appInfo": {"name": "t", "version": "1"},
                "appCapabilities": {},
            }}),
        );
        send(
            bridge,
            json!({"jsonrpc": "2.0", "method": "ui/notifications/initialized"}),
        );
    }

    #[test]
    fn initialize_answers_with_host_context_and_capabilities() {
        let mut b = bridge();
        let step = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": "a", "method": "ui/initialize", "params": {
                "protocolVersion": "2099-01-01", "appInfo": {"name": "t", "version": "1"},
                "appCapabilities": {"availableDisplayModes": ["inline"]},
            }}),
        );
        assert_eq!(step.ask, None);
        let result = &step.post[0]["result"];
        assert_eq!(step.post[0]["id"], "a");
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["hostInfo"]["name"], "gentle-shell-gpui");
        assert_eq!(result["hostContext"]["theme"], "dark");
        assert!(result["hostCapabilities"]["openLinks"].is_object());
        assert!(result["hostCapabilities"].get("serverTools").is_none());
        assert!(!b.ready());
    }

    #[test]
    fn notifications_wait_for_initialized() {
        let mut b = bridge();
        assert_eq!(
            b.notify("ui/notifications/tool-input", tool_input(&json!({"n": 1}))),
            None
        );
        assert_eq!(b.set_context(json!({"theme": "light"})), None);
        send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 1, "method": "ui/initialize", "params": {}}),
        );
        let step = send(
            &mut b,
            json!({"jsonrpc": "2.0", "method": "ui/notifications/initialized"}),
        );
        assert_eq!(step.ask, Some(Ask::Ready));
        assert_eq!(step.post.len(), 1);
        assert_eq!(step.post[0]["method"], "ui/notifications/tool-input");
        assert_eq!(step.post[0]["params"]["arguments"]["n"], 1);
        let now = b.notify("ui/notifications/tool-result", tool_result("t", &json!({})));
        assert_eq!(now.unwrap()["params"]["content"][0]["text"], "t");
    }

    #[test]
    fn host_context_changes_send_only_the_difference() {
        let mut b = bridge();
        handshake(&mut b);
        assert_eq!(
            b.set_context(json!({"theme": "dark", "displayMode": "inline"})),
            None
        );
        let changed = b
            .set_context(json!({"theme": "light", "displayMode": "inline"}))
            .unwrap();
        assert_eq!(changed["method"], "ui/notifications/host-context-changed");
        assert_eq!(changed["params"], json!({"theme": "light"}));
    }

    #[test]
    fn messages_and_context_go_to_the_app() {
        let mut b = bridge();
        handshake(&mut b);
        let step = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 7, "method": "ui/message", "params": {
                "role": "user", "content": [{"type": "text", "text": "Pick B"}, {"type": "image", "data": ""}],
            }}),
        );
        assert_eq!(
            step.ask,
            Some(Ask::Message {
                id: json!(7),
                text: "Pick B\n[image omitted]".into()
            })
        );
        // The older single-object form is accepted too.
        let single = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 8, "method": "ui/message", "params": {
                "role": "user", "content": {"type": "text", "text": "hi"},
            }}),
        );
        assert!(matches!(single.ask, Some(Ask::Message { ref text, .. }) if text == "hi"));

        let step = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 9, "method": "ui/update-model-context", "params": {
                "content": [{"type": "text", "text": "slider=3"}], "structuredContent": {"v": 3},
            }}),
        );
        let Some(Ask::Context {
            content,
            structured,
            ..
        }) = step.ask
        else {
            panic!("{step:?}");
        };
        assert_eq!(
            context_text(&content, structured.as_ref()),
            "slider=3\n{\"v\":3}"
        );
    }

    #[test]
    fn bad_requests_get_json_rpc_errors() {
        let mut b = bridge();
        let code = |step: &Step| step.post[0]["error"]["code"].as_i64().unwrap();
        assert_eq!(code(&b.receive("{not json")), PARSE_ERROR);
        assert_eq!(
            code(&send(&mut b, json!({"id": 1, "method": "ping"}))),
            INVALID_REQUEST
        );
        let role = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 2, "method": "ui/message", "params": {"role": "assistant", "content": []}}),
        );
        assert_eq!(code(&role), INVALID_PARAMS);
        let empty = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 3, "method": "ui/message", "params": {"role": "user", "content": []}}),
        );
        assert_eq!(code(&empty), INVALID_PARAMS);
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://",
            "https://a b",
        ] {
            let step = send(
                &mut b,
                json!({"jsonrpc": "2.0", "id": 4, "method": "ui/open-link", "params": {"url": url}}),
            );
            assert_eq!(code(&step), INVALID_PARAMS, "{url}");
        }
        let tools = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "x"}}),
        );
        assert_eq!(code(&tools), METHOD_NOT_FOUND);
        assert!(
            tools.post[0]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no MCP server")
        );
        let unknown = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 6, "method": "ui/download-file"}),
        );
        assert_eq!(code(&unknown), METHOD_NOT_FOUND);
        // Notifications never get an answer, even unknown ones.
        assert_eq!(
            send(&mut b, json!({"jsonrpc": "2.0", "method": "x/y"})),
            Step::default()
        );
    }

    #[test]
    fn links_size_ping_and_display_mode() {
        let mut b = bridge();
        let link = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 1, "method": "ui/open-link", "params": {"url": " https://example.com/a "}}),
        );
        assert_eq!(
            link.ask,
            Some(Ask::OpenLink {
                id: json!(1),
                url: "https://example.com/a".into()
            })
        );
        let size = send(
            &mut b,
            json!({"jsonrpc": "2.0", "method": "ui/notifications/size-changed", "params": {"width": 300, "height": 420.5}}),
        );
        assert_eq!(
            size.ask,
            Some(Ask::Resize {
                width: Some(300.),
                height: Some(420.5)
            })
        );
        let ping = send(&mut b, json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
        assert_eq!(ping.post[0]["result"], json!({}));
        let mode = send(
            &mut b,
            json!({"jsonrpc": "2.0", "id": 3, "method": "ui/request-display-mode", "params": {"mode": "pip"}}),
        );
        assert_eq!(mode.post[0]["result"]["mode"], "inline");
    }

    #[test]
    fn teardown_waits_for_the_pages_answer() {
        let mut b = bridge();
        assert_eq!(b.teardown("closed"), None);
        handshake(&mut b);
        let request = b.teardown("closed").unwrap();
        assert_eq!(request["method"], "ui/resource-teardown");
        let id = request["id"].clone();
        let other = send(&mut b, json!({"jsonrpc": "2.0", "id": 999, "result": {}}));
        assert_eq!(other.ask, None);
        let done = send(&mut b, json!({"jsonrpc": "2.0", "id": id, "result": {}}));
        assert_eq!(done.ask, Some(Ask::TornDown));
        b.reset();
        assert!(!b.ready());
    }

    #[test]
    fn host_context_carries_the_theme_variables() {
        let palette = Palette {
            dark: true,
            background: "#111111",
            raised: "#222222",
            well: "#000000",
            fg: "#eeeeee",
            muted: "#aaaaaa",
            faint: "#777777",
            border: "#333333",
            border_strong: "#444444",
            accent: "#ff66aa",
            info: "#66aaff",
            success: "#66cc88",
            danger: "#ff5555",
            warning: "#ffaa33",
            font_sans: "system-ui, sans-serif",
            font_mono: "Menlo, monospace",
            rem: 16.,
        };
        let context = host_context(&palette, 640.4, 900., Some("es-AR"), "gentle-shell-gpui/1");
        let vars = &context["styles"]["variables"];
        assert_eq!(context["theme"], "dark");
        assert_eq!(vars["--color-background-primary"], "#111111");
        assert_eq!(vars["--color-ring-primary"], "#ff66aa");
        assert_eq!(vars["--font-text-md-size"], "14px");
        assert_eq!(vars["--font-heading-3xl-line-height"], "42px");
        assert_eq!(
            context["containerDimensions"],
            json!({"width": 640., "maxHeight": 900.})
        );
        assert_eq!(context["locale"], "es-AR");
        assert_eq!(context["toolInfo"]["tool"]["name"], "gentle_ui");
        assert_eq!(
            context["toolInfo"]["tool"]["description"],
            "Agent-written HTML shown in Osdy Code Desktop"
        );
        // Every standard variable is filled.
        assert_eq!(vars.as_object().unwrap().len(), 54 + 22);
    }

    #[test]
    fn locale_tags_from_posix_locales() {
        assert_eq!(locale_tag("en_US.UTF-8").as_deref(), Some("en-US"));
        assert_eq!(locale_tag("de_DE@euro").as_deref(), Some("de-DE"));
        assert_eq!(locale_tag("C"), None);
        assert_eq!(locale_tag("POSIX"), None);
        assert_eq!(locale_tag("x y"), None);
    }
}
