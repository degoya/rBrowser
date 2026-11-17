//! The debugger's parts of a rules document (RB-080-37), beside the rules: the breakpoints and
//! their switch, the person's settings (sent by the first-party plugin from its settings, never
//! honoured from a project plugin) and the asks - what the panel wants done with a held request
//! or sent as a request of its own - with the answer the host leaves in `$network.answers`.
//!
//! ```json
//! { "breakpoints": [ { "id": "bp-1", "match": { "urlPattern": "*/api/*" }, "stage": "request",
//!                      "views": [1, 2], "enabled": true } ],
//!   "breakpointsEnabled": true,
//!   "settings": { "breakpointTimeoutMs": 30000, "maxBodyBytes": 4194304,
//!                 "mapLocalRoots": ["C:\\sites"], "allowComposeFromMcp": false },
//!   "asks": [ { "id": "a1", "kind": "continue", "held": "h3", "allViews": false, "edits": { "status": 500 } },
//!             { "id": "a2", "kind": "abort", "held": "h4" },
//!             { "id": "a3", "kind": "compose", "view": 1, "times": 1,
//!               "request": { "method": "GET", "url": "https://a.test/api", "headers": [], "body": "" } },
//!             { "id": "a4", "kind": "repeat", "view": 1, "requestId": "12.7", "edits": {} } ] }
//! ```

use base64::Engine as _;
use rb_engine::ViewId;
use serde_json::Value;

use super::action::{headers, web_url};
use super::compose::ComposeRequest;
use super::held::Edits;
use super::rule::{self, MAX_ID_CHARS, Matcher, Rejected};

/// The most breakpoints of one document.
pub const MAX_BREAKPOINTS: usize = 128;
/// The most asks of one document.
pub const MAX_ASKS: usize = 64;
/// The longest hold of a request, in milliseconds (10 minutes); 0 is "no timeout".
pub const MAX_TIMEOUT_MS: u64 = 600_000;
/// The default of `breakpointTimeoutMs`.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// The most bytes a response body is read and rewritten with.
pub const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
/// The default of `maxBodyBytes` (4 MiB).
pub const DEFAULT_MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// The most times a compose or repeat sends in a row.
pub const MAX_TIMES: u32 = 10;
/// The largest body of an edit or a composed request.
pub const MAX_EDIT_BODY: usize = 4 * 1024 * 1024;

/// Where a request is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Before it leaves: method, URL, headers and body editable.
    Request,
    /// Once the headers came back: status, headers and body editable.
    Response,
}

impl Stage {
    /// CDP's `requestStage` value.
    #[must_use]
    pub fn cdp(self) -> &'static str {
        match self {
            Self::Request => "Request",
            Self::Response => "Response",
        }
    }

    /// The name the panel and MCP use.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
        }
    }
}

/// One breakpoint.
#[derive(Clone, Debug)]
pub struct Breakpoint {
    pub id: String,
    pub matcher: Matcher,
    pub stage: Stage,
    /// Only these views; every view when none.
    pub views: Option<Vec<ViewId>>,
    pub enabled: bool,
}

/// The person's debugger settings, as the first-party plugin sends them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugSettings {
    /// A held request goes on unchanged after this long; 0 never.
    pub timeout_ms: u64,
    /// Response bodies larger than this are not rewritten and not shown.
    pub max_body_bytes: usize,
    /// Folders map-local rules may serve from, beside the project.
    pub map_local_roots: Vec<String>,
    /// `network_compose` and `network_repeat` over MCP.
    pub allow_compose_from_mcp: bool,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            map_local_roots: Vec::new(),
            allow_compose_from_mcp: false,
        }
    }
}

/// What the plugin asks the host to do.
#[derive(Clone, Debug)]
pub enum Ask {
    /// Let held request `held` go on, with `edits`; `all_views` every held request of its key.
    Continue {
        id: String,
        held: String,
        all_views: bool,
        edits: Edits,
    },
    /// Fail held request `held`; `all_views` every held request of its key.
    Abort {
        id: String,
        held: String,
        all_views: bool,
    },
    /// Send `request` from `view`'s context (its cookies and user agent), `times` in a row.
    Compose {
        id: String,
        view: Option<ViewId>,
        request: ComposeRequest,
        times: u32,
    },
    /// Send captured request `request_id` of `view` again, with `edits`, `times` in a row.
    Repeat {
        id: String,
        view: ViewId,
        request_id: String,
        edits: Edits,
        times: u32,
    },
}

impl Ask {
    /// The ask's id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Continue { id, .. }
            | Self::Abort { id, .. }
            | Self::Compose { id, .. }
            | Self::Repeat { id, .. } => id,
        }
    }
}

fn plain_id(raw: &Value) -> Option<&str> {
    raw.as_str().filter(|id| {
        !id.is_empty()
            && id.len() <= MAX_ID_CHARS
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._:/#@+-".contains(c))
    })
}

fn reject(id: &str, reason: &str) -> Rejected {
    Rejected {
        id: id.to_owned(),
        reason: reason.to_owned(),
    }
}

/// The breakpoints of a document's `breakpoints` list, and the ones refused.
#[must_use]
pub fn parse_breakpoints(raw: &Value) -> (Vec<Breakpoint>, Vec<Rejected>) {
    let mut breakpoints: Vec<Breakpoint> = Vec::new();
    let mut rejected = Vec::new();
    for (index, entry) in raw.as_array().into_iter().flatten().enumerate() {
        let id = plain_id(&entry["id"]).map_or_else(|| format!("#{index}"), str::to_owned);
        if index >= MAX_BREAKPOINTS {
            rejected.push(reject(&id, "too many breakpoints"));
            continue;
        }
        if breakpoints.iter().any(|known| known.id == id) {
            rejected.push(reject(&id, "duplicate id"));
            continue;
        }
        match breakpoint(entry, &id) {
            Ok(breakpoint) => breakpoints.push(breakpoint),
            Err(reason) => rejected.push(reject(&id, reason)),
        }
    }
    (breakpoints, rejected)
}

fn breakpoint(raw: &Value, id: &str) -> Result<Breakpoint, &'static str> {
    plain_id(&raw["id"]).ok_or("bad id")?;
    let stage = match &raw["stage"] {
        Value::Null => Stage::Request,
        Value::String(stage) if stage == "request" => Stage::Request,
        Value::String(stage) if stage == "response" => Stage::Response,
        _ => return Err("bad stage"),
    };
    let views = match &raw["views"] {
        Value::Null => None,
        Value::Array(list) if list.len() <= 64 => Some(
            list.iter()
                .map(|view| {
                    view.as_u64()
                        .and_then(|view| u32::try_from(view).ok())
                        .map(ViewId)
                        .ok_or("bad views")
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => return Err("bad views"),
    };
    Ok(Breakpoint {
        id: id.to_owned(),
        matcher: rule::matcher(&raw["match"])?,
        stage,
        views,
        enabled: raw["enabled"] != Value::Bool(false),
    })
}

/// The settings of a document's `settings` object; the defaults for what is missing.
#[must_use]
pub fn parse_settings(raw: &Value) -> DebugSettings {
    let mut settings = DebugSettings::default();
    if let Some(timeout) = raw["breakpointTimeoutMs"].as_u64() {
        settings.timeout_ms = timeout.min(MAX_TIMEOUT_MS);
    }
    if let Some(bytes) = raw["maxBodyBytes"].as_u64() {
        settings.max_body_bytes = usize::try_from(bytes)
            .unwrap_or(MAX_BODY_BYTES)
            .min(MAX_BODY_BYTES);
    }
    settings.map_local_roots = raw["mapLocalRoots"]
        .as_array()
        .map(|roots| {
            roots
                .iter()
                .filter_map(Value::as_str)
                .filter(|root| !root.is_empty() && root.len() <= 1024)
                .take(32)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    settings.allow_compose_from_mcp = raw["allowComposeFromMcp"] == Value::Bool(true);
    settings
}

/// The asks of a document's `asks` list, and the ones refused.
#[must_use]
pub fn parse_asks(raw: &Value) -> (Vec<Ask>, Vec<Rejected>) {
    let mut asks: Vec<Ask> = Vec::new();
    let mut rejected = Vec::new();
    for (index, entry) in raw.as_array().into_iter().flatten().enumerate() {
        let Some(id) = plain_id(&entry["id"]) else {
            rejected.push(reject(&format!("#{index}"), "bad ask id"));
            continue;
        };
        if index >= MAX_ASKS {
            rejected.push(reject(id, "too many asks"));
            continue;
        }
        if asks.iter().any(|known| known.id() == id) {
            rejected.push(reject(id, "duplicate id"));
            continue;
        }
        match ask(entry, id) {
            Ok(ask) => asks.push(ask),
            Err(reason) => rejected.push(reject(id, reason)),
        }
    }
    (asks, rejected)
}

fn ask(raw: &Value, id: &str) -> Result<Ask, &'static str> {
    let id = id.to_owned();
    let all_views = raw["allViews"] == Value::Bool(true);
    let held = || {
        plain_id(&raw["held"])
            .map(str::to_owned)
            .ok_or("bad held id")
    };
    let view = |value: &Value| {
        value
            .as_u64()
            .and_then(|view| u32::try_from(view).ok())
            .map(ViewId)
            .ok_or("bad view")
    };
    let times = match &raw["times"] {
        Value::Null => 1,
        value => value
            .as_u64()
            .filter(|times| *times >= 1)
            .map(|times| u32::try_from(times).unwrap_or(MAX_TIMES).min(MAX_TIMES))
            .ok_or("bad times")?,
    };
    match raw["kind"].as_str() {
        Some("continue") => Ok(Ask::Continue {
            id,
            held: held()?,
            all_views,
            edits: parse_edits(&raw["edits"])?,
        }),
        Some("abort") => Ok(Ask::Abort {
            id,
            held: held()?,
            all_views,
        }),
        Some("compose") => Ok(Ask::Compose {
            id,
            view: match &raw["view"] {
                Value::Null => None,
                value => Some(view(value)?),
            },
            request: parse_request(&raw["request"])?,
            times,
        }),
        Some("repeat") => Ok(Ask::Repeat {
            id,
            view: view(&raw["view"])?,
            request_id: raw["requestId"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 200)
                .ok_or("bad requestId")?
                .to_owned(),
            edits: parse_edits(&raw["edits"])?,
            times,
        }),
        _ => Err("bad ask kind"),
    }
}

/// A method token.
fn method(raw: &Value) -> Result<Option<String>, &'static str> {
    match raw {
        Value::Null => Ok(None),
        Value::String(method)
            if (1..=16).contains(&method.len())
                && method.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Ok(Some(method.to_ascii_uppercase()))
        }
        _ => Err("bad method"),
    }
}

/// `body` (text) or `bodyBase64`, at most [`MAX_EDIT_BODY`] bytes.
fn body(raw: &Value) -> Result<Option<Vec<u8>>, &'static str> {
    let bytes = match (&raw["body"], &raw["bodyBase64"]) {
        (Value::Null, Value::Null) => return Ok(None),
        (Value::String(text), Value::Null) => text.as_bytes().to_vec(),
        (Value::Null, Value::String(data)) => base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| "bad bodyBase64")?,
        _ => return Err("bad body"),
    };
    if bytes.len() > MAX_EDIT_BODY {
        return Err("body too large");
    }
    Ok(Some(bytes))
}

/// The edits of a continue or a repeat.
///
/// # Errors
///
/// A field that does not read.
pub fn parse_edits(raw: &Value) -> Result<Edits, &'static str> {
    if raw.is_null() {
        return Ok(Edits::default());
    }
    if !raw.is_object() {
        return Err("bad edits");
    }
    let url = match &raw["url"] {
        Value::Null => None,
        Value::String(url) if web_url(url) => Some(url.clone()),
        _ => return Err("bad url"),
    };
    let headers = match &raw["headers"] {
        Value::Null => None,
        list => Some(headers(list)?),
    };
    let status = match &raw["status"] {
        Value::Null => None,
        value => Some(
            value
                .as_u64()
                .filter(|status| (100..=599).contains(status))
                .and_then(|status| u16::try_from(status).ok())
                .ok_or("bad status")?,
        ),
    };
    Ok(Edits {
        method: method(&raw["method"])?,
        url,
        headers,
        body: body(raw)?,
        status,
    })
}

/// A composed request: method, `http(s)` URL, headers and body.
///
/// # Errors
///
/// A field that does not read.
pub fn parse_request(raw: &Value) -> Result<ComposeRequest, &'static str> {
    if !raw.is_object() {
        return Err("bad request");
    }
    let url = raw["url"]
        .as_str()
        .filter(|url| web_url(url))
        .ok_or("bad url")?
        .to_owned();
    Ok(ComposeRequest {
        method: method(&raw["method"])?.unwrap_or_else(|| "GET".to_owned()),
        url,
        headers: headers(&raw["headers"])?,
        body: body(raw)?,
    })
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
