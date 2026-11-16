//! The heads the portal forwards (RB-080-05): a device's request goes to the upstream with the
//! upstream's `Host`, `Origin` and `Referer`, without the portal's own cookie, uncompressed (so
//! an HTML answer can take the agent) and with `Connection: close` - one request per connection.
//! The answer comes back with `Location`, cookies and CORS pointed at the portal. A WebSocket
//! upgrade (Vite's HMR) keeps its `Upgrade` and `Connection` headers both ways.

use rb_cdp::request::{HeadError, MAX_HEAD, RequestHead, head_end};
use rb_sync::nav_url::origin_of;

use super::origins::{Upstream, rewrite_url};
use super::pairing::COOKIE_NAME;

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

    /// `Content-Length`.
    #[must_use]
    pub fn content_length(&self) -> Option<usize> {
        self.header("content-length")?.parse().ok()
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

/// The head of `request` for `upstream`, sent by the portal at `portal` (`http://ip:port`).
#[must_use]
pub fn request_head(
    request: &RequestHead,
    upstream: &Upstream,
    portal: &str,
    upgrade: bool,
) -> String {
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
            "origin" => push(name, &rewrite_origin(value, portal, upstream.origin())),
            "referer" => push(name, &rewrite_url(value, portal, upstream.origin())),
            "cookie" => {
                let kept = without_portal_cookie(value);
                if !kept.is_empty() {
                    push(name, &kept);
                }
            }
            other if DROPPED.contains(&other) => {}
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

/// The head of `response` for the device; `body_len` when the portal sends a body of its own
/// (the HTML with the agent) instead of the upstream's bytes.
#[must_use]
pub fn response_head(
    response: &ResponseHead,
    upstream: &Upstream,
    portal: &str,
    body_len: Option<usize>,
) -> String {
    let upgrade = response.status == 101;
    let mut out = format!(
        "{} {} {}\r\n",
        response.version, response.status, response.reason
    );
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
                push(name, &rewrite_url(value, upstream.origin(), portal));
            }
            "access-control-allow-origin" => {
                push(name, &rewrite_origin(value, upstream.origin(), portal));
            }
            "set-cookie" => {
                if let Some(cookie) = device_cookie(value) {
                    push(name, &cookie);
                }
            }
            "content-length" | "transfer-encoding" if body_len.is_some() => {}
            other if DROPPED.contains(&other) => {}
            _ => push(name, value),
        }
    }
    if let Some(length) = body_len {
        push("Content-Length", &length.to_string());
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

/// An origin header's value: `to` when it names `from`, unchanged otherwise.
fn rewrite_origin(value: &str, from: &str, to: &str) -> String {
    if origin_of(value).as_deref() == Some(from) {
        to.to_owned()
    } else {
        value.to_owned()
    }
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
