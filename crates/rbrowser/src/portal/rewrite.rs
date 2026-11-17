//! The heads the portal forwards (RB-080-05): a device's request goes to the upstream with the
//! upstream's `Host`, `Origin` and `Referer`, without the portal's own cookie, uncompressed (so
//! an HTML answer can take the agent) and with `Connection: close` - one request per connection.
//! The answer comes back with `Location`, cookies and CORS pointed at the portal. A WebSocket
//! upgrade (Vite's HMR) keeps its `Upgrade` and `Connection` headers both ways. Every origin is
//! translated through the one [`OriginMap`] (RB-090-65): the page's and its asset origins. A
//! request for what may be rewritten asks for no revalidation (`If-None-Match`,
//! `If-Modified-Since` dropped, RB-090-72): a copy the device cached may predate the portal's
//! rewrite, so such an answer comes whole. An image, a font or a medium (`Sec-Fetch-Dest`) is
//! never rewritten and keeps its revalidation.

use super::origin_map::OriginMap;
use super::origins::Upstream;
use super::pairing::COOKIE_NAME;
use rb_cdp::request::{HeadError, MAX_HEAD, RequestHead, head_end};

/// Hop-by-hop and proxy headers that never pass the portal.
const DROPPED: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "upgrade",
    "strict-transport-security",
    "alt-svc",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
];

/// Request headers that do not reach the upstream for what may be rewritten: a `304` would keep
/// the device's cached copy.
const CONDITIONAL: &[&str] = &["if-none-match", "if-modified-since"];
/// The `Sec-Fetch-Dest` values of what the portal never rewrites; their requests keep
/// [`CONDITIONAL`].
const NEVER_REWRITTEN: &[&str] = &["image", "font", "audio", "video", "track"];

/// Whether `request` asks for something the portal never rewrites (by `Sec-Fetch-Dest`).
fn never_rewritten(request: &RequestHead) -> bool {
    request.header("sec-fetch-dest").is_some_and(|dest| {
        NEVER_REWRITTEN
            .iter()
            .any(|kept| dest.trim().eq_ignore_ascii_case(kept))
    })
}

/// The body the device gets after a response head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutBody {
    /// The upstream's, as it comes, with its framing and validators.
    Upstream,
    /// One of the portal's own of this length (an HTML page with the agent).
    Length(usize),
    /// One of the portal's own, rewritten as it streams and chunked anew (RB-090-72).
    Chunked,
}

/// A parsed response head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseHead {
    /// `HTTP/1.1`.
    pub version: String,
    /// `200`.
    pub status: u16,
    /// `OK`.
    pub reason: String,
    /// The headers, names as sent, values trimmed.
    pub headers: Vec<(String, String)>,
}

impl ResponseHead {
    /// The first header named `name`, in any case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Whether the body is an HTML document.
    #[must_use]
    pub fn is_html(&self) -> bool {
        self.header("content-type").is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/html"))
        })
    }

    /// Whether the body is a style sheet or a script, which may name origins too (Vite writes
    /// `server.origin` into them).
    #[must_use]
    pub fn is_css_or_script(&self) -> bool {
        self.header("content-type").is_some_and(|value| {
            value.split(';').next().is_some_and(|mime| {
                matches!(
                    mime.trim().to_ascii_lowercase().as_str(),
                    "text/css"
                        | "text/javascript"
                        | "application/javascript"
                        | "application/x-javascript"
                        | "text/ecmascript"
                        | "application/ecmascript"
                )
            })
        })
    }

    /// Whether the body is JSON (`application/json` or a `+json` type), which may name origins
    /// with escaped slashes (an API answer naming its images, RB-090-69).
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.header("content-type").is_some_and(|value| {
            value.split(';').next().is_some_and(|mime| {
                let mime = mime.trim().to_ascii_lowercase();
                mime == "application/json"
                    || (mime.starts_with("application/") && mime.ends_with("+json"))
            })
        })
    }

    /// `Content-Length`.
    #[must_use]
    pub fn content_length(&self) -> Option<usize> {
        self.header("content-length")?.parse().ok()
    }

    /// Whether the body has a `Content-Length` of at most `max` bytes and is not chunked: one
    /// the portal can read whole without holding back a stream (JSON, RB-090-69).
    #[must_use]
    pub fn length_within(&self, max: usize) -> bool {
        !self.is_chunked() && self.content_length().is_some_and(|length| length <= max)
    }

    /// Whether the body is chunked.
    #[must_use]
    pub fn is_chunked(&self) -> bool {
        self.header("transfer-encoding")
            .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"))
    }

    /// Whether the body is compressed (the upstream ignored `Accept-Encoding: identity`).
    #[must_use]
    pub fn is_encoded(&self) -> bool {
        self.header("content-encoding")
            .is_some_and(|value| !value.trim().eq_ignore_ascii_case("identity"))
    }
}

/// The response head in `bytes`, up to its blank line.
///
/// # Errors
///
/// [`HeadError::TooLarge`] past [`MAX_HEAD`], [`HeadError::Malformed`] for no HTTP/1.x status.
pub fn parse_response_head(bytes: &[u8]) -> Result<ResponseHead, HeadError> {
    let end = head_end(bytes).unwrap_or(bytes.len());
    if end > MAX_HEAD {
        return Err(HeadError::TooLarge);
    }
    let text = std::str::from_utf8(&bytes[..end]).map_err(|_| HeadError::Malformed)?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().ok_or(HeadError::Malformed)?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().filter(|v| v.starts_with("HTTP/1."));
    let status = parts.next().and_then(|code| code.parse::<u16>().ok());
    let (Some(version), Some(status)) = (version, status) else {
        return Err(HeadError::Malformed);
    };
    let reason = parts.next().unwrap_or("").to_owned();
    let mut headers = Vec::new();
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(HeadError::Malformed)?;
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    Ok(ResponseHead {
        version: version.to_owned(),
        status,
        reason,
        headers,
    })
}

/// The head of `request` for `upstream`; `map` translates the portal's origins back.
#[must_use]
pub fn request_head(
    request: &RequestHead,
    upstream: &Upstream,
    map: &OriginMap,
    upgrade: bool,
) -> String {
    let revalidate = never_rewritten(request);
    let mut out = format!("{} {} HTTP/1.1\r\n", request.method, request.target);
    let mut push = |name: &str, value: &str| {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    };
    push("Host", upstream.authority());
    for (name, value) in &request.headers {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "host" | "accept-encoding" => {}
            "origin" => push(name, &map.origin_to_upstream(value)),
            "referer" => push(name, &map.to_upstream_url(value)),
            "cookie" => {
                let kept = without_portal_cookie(value);
                if !kept.is_empty() {
                    push(name, &kept);
                }
            }
            other if DROPPED.contains(&other) || (!revalidate && CONDITIONAL.contains(&other)) => {}
            _ => push(name, value),
        }
    }
    push("Accept-Encoding", "identity");
    if upgrade {
        push("Upgrade", request.header("upgrade").unwrap_or("websocket"));
        push("Connection", "Upgrade");
    } else {
        push("Connection", "close");
    }
    out.push_str("\r\n");
    out
}

/// The head of `response` for the device; with a body of the portal's own (`body` other than
/// [`OutBody::Upstream`]: the HTML with the agent, a rewritten style sheet, script or JSON) it
/// goes out `no-store`, without the upstream's length, framing and validators.
#[must_use]
pub fn response_head(response: &ResponseHead, map: &OriginMap, body: OutBody) -> String {
    let upgrade = response.status == 101;
    // Chunks need HTTP/1.1; the portal is the device's server, whatever the upstream speaks.
    let version = if body == OutBody::Chunked {
        "HTTP/1.1"
    } else {
        response.version.as_str()
    };
    let mut out = format!("{version} {} {}\r\n", response.status, response.reason);
    let mut push = |name: &str, value: &str| {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    };
    for (name, value) in &response.headers {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "location" | "content-location" => {
                push(name, &map.to_portal_url(value));
            }
            "access-control-allow-origin" => {
                push(name, &map.origin_to_portal(value));
            }
            "set-cookie" => {
                if let Some(cookie) = device_cookie(value) {
                    push(name, &cookie);
                }
            }
            // A body of the portal's own names this run's ports: nothing of it may be cached
            // or revalidated past them.
            "content-length" | "transfer-encoding" | "etag" | "last-modified" | "cache-control"
            | "expires"
                if body != OutBody::Upstream => {}
            other if DROPPED.contains(&other) => {}
            _ => push(name, value),
        }
    }
    match body {
        OutBody::Upstream => {}
        OutBody::Length(length) => push("Content-Length", &length.to_string()),
        OutBody::Chunked => push("Transfer-Encoding", "chunked"),
    }
    if body != OutBody::Upstream {
        push("Cache-Control", "no-store");
    }
    if upgrade {
        push("Upgrade", response.header("upgrade").unwrap_or("websocket"));
        push("Connection", "Upgrade");
    } else {
        push("Connection", "close");
    }
    out.push_str("\r\n");
    out
}

/// A `Cookie` header without the portal's cookie.
fn without_portal_cookie(value: &str) -> String {
    value
        .split(';')
        .map(str::trim)
        .filter(|pair| {
            !pair.is_empty() && pair.split('=').next().map(str::trim) != Some(COOKIE_NAME)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// An upstream cookie as the device keeps it: for the portal's host (no `Domain`), never one
/// named like the portal's own.
fn device_cookie(value: &str) -> Option<String> {
    let mut parts = value.split(';').map(str::trim);
    let pair = parts.next()?;
    if pair.split('=').next().map(str::trim) == Some(COOKIE_NAME) {
        return None;
    }
    let kept: Vec<&str> = std::iter::once(pair)
        .chain(parts.filter(|attribute| {
            !attribute
                .split('=')
                .next()
                .is_some_and(|name| name.trim().eq_ignore_ascii_case("domain"))
        }))
        .collect();
    Some(kept.join("; "))
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
