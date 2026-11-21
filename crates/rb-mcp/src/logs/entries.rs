//! The entries the log buffers keep (split from `logs.rs`, 500-line rule): a console message, a
//! network request and an error, as the tools answer them.

use serde::Serialize;

use super::ResponseFacts;

/// One console message.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleEntry {
    /// `log`, `info`, `warning`, `error`, `debug`, ... (`console.*` or the browser's level).
    pub level: String,
    /// `console-api` for the page's `console.*`, else the browser's source (`network`,
    /// `security`, `violation`, ...).
    pub source: String,
    /// The message.
    pub text: String,
    /// The script's URL, where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The line (1-based), where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// Milliseconds since the Unix epoch.
    pub time: f64,
}

/// One network request.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEntry {
    /// CDP's request id.
    pub id: String,
    /// The URL.
    pub url: String,
    /// `GET`, `POST`, ...
    pub method: String,
    /// `Document`, `Script`, `XHR`, `Fetch`, `Image`, ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    /// The HTTP status, once a response came.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u64>,
    /// The response's MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Served from a cache.
    pub from_cache: bool,
    /// The bytes received, once finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// When it started, milliseconds since the Unix epoch.
    pub time: f64,
    /// How long it took, in milliseconds, once finished or failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
    /// Why it failed (`net::ERR_...`), when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
    /// The response's waiting time, caching and compression, once it came (RB-096-06).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ResponseFacts>,
    /// The body's bytes after decoding (the sum of `Network.dataReceived`), once any came.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decoded_size: Option<f64>,
    /// CDP's monotonic start, for the duration.
    #[serde(skip)]
    pub(super) started: f64,
    /// The request's headers as sent, for a repeat (RB-080-37); never serialised: they carry
    /// cookies and tokens.
    #[serde(skip)]
    pub(super) request_headers: Vec<(String, String)>,
}

impl NetworkEntry {
    /// The request's headers as sent (never part of the feed or an MCP answer).
    #[must_use]
    pub fn request_headers(&self) -> &[(String, String)] {
        &self.request_headers
    }
}

/// One uncaught exception.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntry {
    /// The message (`Uncaught TypeError: ...`) with the exception's description.
    pub text: String,
    /// The script's URL, where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The line (1-based), where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// The column (1-based), where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
    /// Milliseconds since the Unix epoch.
    pub time: f64,
}
