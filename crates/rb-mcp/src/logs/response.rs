//! What a request's response tells about its speed, caching and compression (RB-096-06, the
//! network panel's endpoint analysis): read from CDP's `Network.responseReceived` once, kept on
//! the request and handed on in the `$console` feed and `network_requests`. Only these header
//! facts are kept - never another header, never a value that can carry a secret.

use serde::Serialize;
use serde_json::Value;

/// The most characters of a `Cache-Control` value kept.
pub const MAX_CACHE_CONTROL: usize = 200;
/// The most characters of a `Content-Encoding` value kept.
const MAX_ENCODING: usize = 32;

/// The response facts of one request.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseFacts {
    /// Milliseconds from the request sent to the first byte of the answer (DevTools' "waiting
    /// for server response"); `None` from a cache or without timing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<f64>,
    /// `Content-Encoding`, lower case (`br`, `gzip`); `None` without one or `identity`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// `Cache-Control` as sent, cut to [`MAX_CACHE_CONTROL`] characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<String>,
    /// An `Expires` header came.
    pub expires: bool,
    /// An `ETag` came (a revalidation is possible).
    pub etag: bool,
    /// A `Last-Modified` came.
    pub last_modified: bool,
}

fn header<'a>(headers: &'a Value, name: &str) -> Option<&'a str> {
    headers
        .as_object()?
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .find_map(|(_, value)| value.as_str())
}

fn cut(text: &str, most: usize) -> String {
    text.trim().chars().take(most).collect()
}

fn ttfb(timing: &Value) -> Option<f64> {
    let sent = timing["sendEnd"].as_f64().filter(|sent| *sent >= 0.0)?;
    let first = timing["receiveHeadersStart"]
        .as_f64()
        .filter(|start| *start > 0.0)
        .or_else(|| timing["receiveHeadersEnd"].as_f64())?;
    (first >= sent).then_some(first - sent)
}

/// The facts of a CDP `Network.Response` object.
#[must_use]
pub fn facts(response: &Value) -> ResponseFacts {
    let headers = &response["headers"];
    ResponseFacts {
        ttfb_ms: ttfb(&response["timing"]),
        encoding: header(headers, "content-encoding")
            .map(|value| cut(value, MAX_ENCODING).to_ascii_lowercase())
            .filter(|value| !value.is_empty() && value != "identity"),
        cache_control: header(headers, "cache-control")
            .map(|value| cut(value, MAX_CACHE_CONTROL))
            .filter(|value| !value.is_empty()),
        expires: header(headers, "expires").is_some(),
        etag: header(headers, "etag").is_some(),
        last_modified: header(headers, "last-modified").is_some(),
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
