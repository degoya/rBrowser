//! Which endpoint a request asks for, whether it may, and the JSON the proxy answers itself.
//!
//! The endpoints are Chrome's: `/json/version`, `/json/list` (also `/json`), `/json/protocol`,
//! and the WebSockets `/devtools/browser/<id>` and `/devtools/page/<targetId>`. Each request is
//! held, in this order, to: a loopback `Host` (no DNS rebinding) and no foreign `Origin` (a web
//! page never gets in, even with a token), the token ([`crate::token::authorize`]), `GET`, a
//! known endpoint, and an upgrade exactly for the two WebSockets. The answers' URLs use the host
//! the client asked for - through the WSL tunnel that is the tunnel's port - and carry the
//! `/t/<token>` prefix only when the client proved the token that way.

use serde_json::{Value, json};

use crate::request::{RequestHead, websocket_key};
use crate::secret::random_hex;
use crate::token::{Proof, ProxyToken, authorize};

/// The only origin a browser may connect from: DevTools' own pages.
pub const DEVTOOLS_ORIGIN: &str = "devtools://devtools";

/// An endpoint of the proxy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Endpoint {
    /// `/json/version`.
    Version,
    /// `/json/list`, `/json`.
    List,
    /// `/json/protocol`.
    Protocol,
    /// `/devtools/browser/<id>`: a browser session of its own.
    Browser,
    /// `/devtools/page/<targetId>`: one page.
    Page(String),
}

/// Why a request is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No loopback host, or a foreign origin.
    Forbidden,
    /// No valid token.
    Unauthorized,
    /// Not `GET`.
    MethodNotAllowed,
    /// No such endpoint.
    NotFound,
    /// A WebSocket endpoint without an upgrade, or an upgrade of a JSON endpoint.
    BadRequest,
    /// The head is longer than [`crate::request::MAX_HEAD`].
    TooLarge,
}

impl Refusal {
    /// The HTTP status.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::TooLarge => 431,
        }
    }

    /// The reason phrase and body text.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::BadRequest => "Bad Request",
            Self::Unauthorized => {
                "Unauthorized: rBrowser's CDP proxy needs its token (Authorization: Bearer or /t/<token>/)"
            }
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::TooLarge => "Request Header Fields Too Large",
        }
    }
}

/// Where the answers' URLs point: the host the client asked for and the token prefix it used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    /// `127.0.0.1:<port>` as the client wrote it.
    pub host: String,
    /// `/t/<token>` when the client proved the token in the path, else empty.
    pub prefix: String,
}

/// What to do with a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Answer with the status.
    Refuse(Refusal),
    /// Answer the JSON endpoint.
    Http(Endpoint, Base),
    /// Upgrade to a WebSocket with the key.
    Upgrade(Endpoint, String),
}

/// The endpoint `path` (token already removed) names; `browser_id` is this proxy's.
#[must_use]
pub fn endpoint(path: &str, browser_id: &str) -> Option<Endpoint> {
    let path = path
        .strip_suffix('/')
        .filter(|p| !p.is_empty())
        .unwrap_or(path);
    match path {
        "/json/version" => return Some(Endpoint::Version),
        "/json" | "/json/list" => return Some(Endpoint::List),
        "/json/protocol" => return Some(Endpoint::Protocol),
        _ => {}
    }
    if path.strip_prefix("/devtools/browser/") == Some(browser_id) {
        return Some(Endpoint::Browser);
    }
    path.strip_prefix("/devtools/page/")
        .filter(|id| !id.is_empty() && !id.contains('/'))
        .map(|id| Endpoint::Page(id.to_owned()))
}

/// Decides `head` for the proxy with `token` and browser id `browser_id`.
#[must_use]
pub fn decide(head: &RequestHead, token: &ProxyToken, browser_id: &str) -> Decision {
    let Some(host) = head.header("host").filter(|host| loopback_host(host)) else {
        return Decision::Refuse(Refusal::Forbidden);
    };
    if head
        .header("origin")
        .is_some_and(|origin| origin != DEVTOOLS_ORIGIN)
    {
        return Decision::Refuse(Refusal::Forbidden);
    }
    let Some(authorized) = authorize(token, head.path(), head.header("authorization")) else {
        return Decision::Refuse(Refusal::Unauthorized);
    };
    if head.method != "GET" {
        return Decision::Refuse(Refusal::MethodNotAllowed);
    }
    let Some(endpoint) = endpoint(&authorized.path, browser_id) else {
        return Decision::Refuse(Refusal::NotFound);
    };
    let socket = matches!(endpoint, Endpoint::Browser | Endpoint::Page(_));
    match (socket, websocket_key(head)) {
        (true, Some(key)) => Decision::Upgrade(endpoint, key.to_owned()),
        (false, None) => {
            let prefix = match authorized.proof {
                Proof::Path => token.path_prefix(),
                Proof::Header => String::new(),
            };
            Decision::Http(
                endpoint,
                Base {
                    host: host.to_owned(),
                    prefix,
                },
            )
        }
        _ => Decision::Refuse(Refusal::BadRequest),
    }
}

/// Whether `host` (a `Host` header) names the loopback: `127.0.0.1`, `localhost` or `[::1]`,
/// with or without a port.
fn loopback_host(host: &str) -> bool {
    let (name, port) = if let Some(rest) = host.strip_prefix('[') {
        match rest.split_once(']') {
            Some((name, port)) => (name, port.strip_prefix(':')),
            None => return false,
        }
    } else {
        match host.rsplit_once(':') {
            Some((name, port)) => (name, Some(port)),
            None => (host, None),
        }
    };
    let port_ok =
        port.is_none_or(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
    let name_ok = name == "127.0.0.1" || name == "::1" || name.eq_ignore_ascii_case("localhost");
    port_ok && name_ok
}

/// `ws://<host><prefix><path>`.
#[must_use]
pub fn ws_url(base: &Base, path: &str) -> String {
    format!("ws://{}{}{path}", base.host, base.prefix)
}

/// The answer of `/json/version` from the browser's `Browser.getVersion` result.
#[must_use]
pub fn version_body(base: &Base, browser_id: &str, version: &Value) -> Value {
    let field = |name: &str| version.get(name).cloned().unwrap_or(Value::from(""));
    json!({
        "Browser": field("product"),
        "Protocol-Version": field("protocolVersion"),
        "User-Agent": field("userAgent"),
        "V8-Version": field("jsVersion"),
        "WebKit-Version": field("revision"),
        "webSocketDebuggerUrl": ws_url(base, &format!("/devtools/browser/{browser_id}")),
    })
}

/// The answer of `/json/list` for the (already filtered) `TargetInfo`s `infos`.
#[must_use]
pub fn list_body(base: &Base, infos: &[Value]) -> Value {
    let field = |info: &Value, name: &str| info.get(name).cloned().unwrap_or(Value::from(""));
    Value::Array(
        infos
            .iter()
            .map(|info| {
                let id = info
                    .get("targetId")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                json!({
                    "description": "",
                    "id": id,
                    "title": field(info, "title"),
                    "type": field(info, "type"),
                    "url": field(info, "url"),
                    "webSocketDebuggerUrl": ws_url(base, &format!("/devtools/page/{id}")),
                })
            })
            .collect(),
    )
}

/// The answer of `/json/protocol`: the domains of `Schema.getDomains` (the pipe has no full
/// protocol description).
#[must_use]
pub fn protocol_body(domains: &Value) -> Value {
    json!({ "domains": domains.get("domains").cloned().unwrap_or(json!([])) })
}

/// A fresh id for `/devtools/browser/<id>` (not a secret: it only names the endpoint).
///
/// # Errors
///
/// The OS gives no randomness.
pub fn new_browser_id() -> std::io::Result<String> {
    random_hex(16)
}

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod tests;
