//! The per-view ring buffers behind `console_logs`, `network_requests` and `errors`, fed from
//! CDP events (`Runtime.consoleAPICalled`, `Log.entryAdded`, `Network.*`,
//! `Runtime.exceptionThrown`), and the issues buffer the plugins granted `console-logs` read
//! (`Audits.issueAdded`: CSP violations, mixed content, deprecations; RB-080-09). Only what the tools answer is kept: for a request its URL,
//! method, type, status, MIME type, timing, size and failure - never a header or a body - and
//! every text and URL with its credentials masked (`rb_diag::page_text`). The host feeds
//! [`LogStore::record`] with each event of a view and sets the sizes from the settings
//! (`mcp.consoleLogLimit`, `mcp.networkRequestLimit`, `mcp.errorLimit`).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use rb_util::lock;

use rb_diag::page_text::redact_page_text;
use rb_engine::ViewId;
use serde::Serialize;
use serde_json::Value;

use crate::issues::issue;
pub use crate::issues::{ISSUE_CODES, IssueEntry};

/// The CDP domains the host enables on every view while the server runs.
pub const DOMAINS: &[&str] = &[
    "Runtime.enable",
    "Log.enable",
    "Network.enable",
    "Audits.enable",
];
/// The longest text or URL kept; longer ones are cut with an ellipsis.
pub const MAX_TEXT: usize = 2048;

/// How many entries each buffer keeps per view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogLimits {
    /// Console messages.
    pub console: usize,
    /// Network requests.
    pub network: usize,
    /// Uncaught exceptions.
    pub errors: usize,
}

impl Default for LogLimits {
    fn default() -> Self {
        Self {
            console: 500,
            network: 500,
            errors: 200,
        }
    }
}

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
    /// CDP's monotonic start, for the duration.
    #[serde(skip)]
    started: f64,
    /// The request's headers as sent, for a repeat (RB-080-37); never serialised: they carry
    /// cookies and tokens.
    #[serde(skip)]
    request_headers: Vec<(String, String)>,
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

/// Which buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    /// `console_logs`.
    Console,
    /// `network_requests`.
    Network,
    /// `errors`.
    Errors,
}

#[derive(Debug, Default)]
struct ViewLogs {
    console: VecDeque<ConsoleEntry>,
    network: VecDeque<NetworkEntry>,
    errors: VecDeque<ErrorEntry>,
    issues: VecDeque<IssueEntry>,
    /// The newest time recorded before the last new document ([`LogStore::navigated_at`]).
    navigated_at: Option<f64>,
}

impl ViewLogs {
    /// The newest page time in the buffers; an issue's time is the host's, when it arrived, so
    /// it is left out (it could be later than the new document's first message).
    fn newest(&self) -> Option<f64> {
        let console = self.console.iter().map(|entry| entry.time);
        let errors = self.errors.iter().map(|entry| entry.time);
        let network = self.network.iter().map(|entry| entry.time);
        console.chain(errors).chain(network).reduce(f64::max)
    }
}

/// The buffers of every view.
#[derive(Debug, Default)]
pub struct LogStore {
    views: Mutex<HashMap<ViewId, ViewLogs>>,
    limits: Mutex<LogLimits>,
}

fn push<T>(buffer: &mut VecDeque<T>, entry: T, limit: usize) {
    buffer.push_back(entry);
    while buffer.len() > limit {
        buffer.pop_front();
    }
}

/// `text` cut to [`MAX_TEXT`] characters, its credentials masked
/// ([`rb_diag::page_text::redact_page_text`]): the buffers answer MCP and feed the plugins
/// granted `console-logs` (review finding L1 of Charge 6).
fn cut(text: &str) -> String {
    match text.char_indices().nth(MAX_TEXT) {
        Some((index, _)) => format!("{}...", redact_page_text(&text[..index])),
        None => redact_page_text(text),
    }
}

pub(crate) fn text_of(value: &Value) -> Option<String> {
    value.as_str().map(cut)
}

/// A console argument (a `Runtime.RemoteObject`) as text.
fn argument(object: &Value) -> String {
    match &object["value"] {
        Value::String(text) => text.clone(),
        Value::Null => object["description"]
            .as_str()
            .or_else(|| object["unserializableValue"].as_str())
            .or_else(|| object["type"].as_str())
            .unwrap_or_default()
            .to_owned(),
        other => other.to_string(),
    }
}

impl LogStore {
    /// A store with `limits`.
    #[must_use]
    pub fn new(limits: LogLimits) -> Self {
        Self {
            views: Mutex::new(HashMap::new()),
            limits: Mutex::new(limits),
        }
    }

    /// Sets the sizes; longer buffers lose their oldest entries at the next record.
    pub fn set_limits(&self, limits: LogLimits) {
        *lock(&self.limits) = limits;
    }

    /// Takes the CDP event `method` with `params` of `view`; any other event is ignored.
    pub fn record(&self, view: ViewId, method: &str, params: &Value) {
        let limits = *lock(&self.limits);
        let mut views = lock(&self.views);
        let logs = views.entry(view).or_default();
        match method {
            "Runtime.consoleAPICalled" => {
                let frame = &params["stackTrace"]["callFrames"][0];
                let text = params["args"]
                    .as_array()
                    .map(|args| args.iter().map(argument).collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                let entry = ConsoleEntry {
                    level: params["type"].as_str().unwrap_or("log").to_owned(),
                    source: "console-api".to_owned(),
                    text: cut(&text),
                    url: text_of(&frame["url"]).filter(|url| !url.is_empty()),
                    line: frame["lineNumber"].as_u64().map(|line| line + 1),
                    time: params["timestamp"].as_f64().unwrap_or_default(),
                };
                push(&mut logs.console, entry, limits.console);
            }
            "Log.entryAdded" => {
                let entry = &params["entry"];
                let entry = ConsoleEntry {
                    level: entry["level"].as_str().unwrap_or("info").to_owned(),
                    source: entry["source"].as_str().unwrap_or("other").to_owned(),
                    text: text_of(&entry["text"]).unwrap_or_default(),
                    url: text_of(&entry["url"]),
                    line: entry["lineNumber"].as_u64().map(|line| line + 1),
                    time: entry["timestamp"].as_f64().unwrap_or_default(),
                };
                push(&mut logs.console, entry, limits.console);
            }
            "Runtime.exceptionThrown" => {
                let details = &params["exceptionDetails"];
                let description = details["exception"]["description"].as_str();
                let text = match (details["text"].as_str(), description) {
                    (Some(text), Some(description)) => format!("{text} {description}"),
                    (Some(text), None) => text.to_owned(),
                    (None, Some(description)) => description.to_owned(),
                    (None, None) => "exception".to_owned(),
                };
                let entry = ErrorEntry {
                    text: cut(&text),
                    url: text_of(&details["url"]).filter(|url| !url.is_empty()),
                    line: details["lineNumber"].as_u64().map(|line| line + 1),
                    column: details["columnNumber"].as_u64().map(|column| column + 1),
                    time: params["timestamp"].as_f64().unwrap_or_default(),
                };
                push(&mut logs.errors, entry, limits.errors);
            }
            "Network.requestWillBeSent" => {
                let request = &params["request"];
                let entry = NetworkEntry {
                    id: params["requestId"].as_str().unwrap_or_default().to_owned(),
                    url: text_of(&request["url"]).unwrap_or_default(),
                    method: request["method"].as_str().unwrap_or("GET").to_owned(),
                    resource_type: params["type"].as_str().map(str::to_owned),
                    status: None,
                    mime_type: None,
                    from_cache: false,
                    size: None,
                    time: params["wallTime"].as_f64().unwrap_or_default() * 1000.0,
                    duration_ms: None,
                    failed: None,
                    started: params["timestamp"].as_f64().unwrap_or_default(),
                    request_headers: request["headers"]
                        .as_object()
                        .map(|headers| {
                            headers
                                .iter()
                                .filter_map(|(name, value)| {
                                    Some((name.clone(), value.as_str()?.to_owned()))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                };
                push(&mut logs.network, entry, limits.network);
            }
            "Network.responseReceived" => {
                if let Some(entry) = request_of(&mut logs.network, params) {
                    let response = &params["response"];
                    entry.status = response["status"].as_u64();
                    entry.mime_type = response["mimeType"].as_str().map(str::to_owned);
                    entry.from_cache = response["fromDiskCache"].as_bool().unwrap_or(false)
                        || response["fromServiceWorker"].as_bool().unwrap_or(false)
                        || response["fromPrefetchCache"].as_bool().unwrap_or(false);
                    if let Some(kind) = params["type"].as_str() {
                        entry.resource_type = Some(kind.to_owned());
                    }
                }
            }
            "Network.loadingFinished" => {
                if let Some(entry) = request_of(&mut logs.network, params) {
                    entry.size = params["encodedDataLength"].as_f64();
                    entry.duration_ms = duration(entry.started, &params["timestamp"]);
                }
            }
            "Network.loadingFailed" => {
                if let Some(entry) = request_of(&mut logs.network, params) {
                    let canceled = params["canceled"].as_bool().unwrap_or(false);
                    entry.failed = Some(if canceled {
                        "canceled".to_owned()
                    } else {
                        params["errorText"].as_str().unwrap_or("failed").to_owned()
                    });
                    entry.duration_ms = duration(entry.started, &params["timestamp"]);
                }
            }
            // The main frame's document went (a navigation or a reload; not a same-document
            // one): what was recorded so far is the old document's. CDP delivers the events of a
            // session in order, so the newest time now is the mark.
            "Runtime.executionContextsCleared" => {
                if let Some(newest) = logs.newest() {
                    logs.navigated_at = Some(logs.navigated_at.map_or(newest, |at| at.max(newest)));
                }
            }
            "Audits.issueAdded" => {
                if let Some(entry) = issue(&params["issue"]) {
                    // Issues share the size of the exception buffer (`mcp.errorLimit`).
                    push(&mut logs.issues, entry, limits.errors);
                }
            }
            _ => {}
        }
    }

    /// The console messages of `view`, oldest first, at most the last `limit`.
    #[must_use]
    pub fn console(&self, view: ViewId, limit: usize) -> Vec<ConsoleEntry> {
        lock(&self.views)
            .get(&view)
            .map(|logs| last(&logs.console, limit))
            .unwrap_or_default()
    }

    /// The request `id` of `view`, as recorded.
    #[must_use]
    pub fn request(&self, view: ViewId, id: &str) -> Option<NetworkEntry> {
        let logs = lock(&self.views);
        logs.get(&view)?
            .network
            .iter()
            .rev()
            .find(|entry| entry.id == id)
            .cloned()
    }

    /// The requests of `view`, oldest first, at most the last `limit`.
    #[must_use]
    pub fn network(&self, view: ViewId, limit: usize) -> Vec<NetworkEntry> {
        lock(&self.views)
            .get(&view)
            .map(|logs| last(&logs.network, limit))
            .unwrap_or_default()
    }

    /// The exceptions of `view`, oldest first, at most the last `limit`.
    #[must_use]
    pub fn errors(&self, view: ViewId, limit: usize) -> Vec<ErrorEntry> {
        lock(&self.views)
            .get(&view)
            .map(|logs| last(&logs.errors, limit))
            .unwrap_or_default()
    }

    /// The issues of `view`, oldest first, at most the last `limit`.
    #[must_use]
    pub fn issues(&self, view: ViewId, limit: usize) -> Vec<IssueEntry> {
        lock(&self.views)
            .get(&view)
            .map(|logs| last(&logs.issues, limit))
            .unwrap_or_default()
    }

    /// The newest time (milliseconds since the epoch) `view` recorded before its main frame's
    /// last new document; `None` before the first. A message after it is the current document's
    /// (`console.clearOnNavigate` of the console plugin, through the `$console` feed).
    #[must_use]
    pub fn navigated_at(&self, view: ViewId) -> Option<f64> {
        lock(&self.views)
            .get(&view)
            .and_then(|logs| logs.navigated_at)
    }

    /// Empties `view`'s buffer of `kind`.
    pub fn clear(&self, view: ViewId, kind: LogKind) {
        if let Some(logs) = lock(&self.views).get_mut(&view) {
            match kind {
                LogKind::Console => logs.console.clear(),
                LogKind::Network => logs.network.clear(),
                LogKind::Errors => logs.errors.clear(),
            }
        }
    }

    /// Drops everything of `view` (closed).
    pub fn forget(&self, view: ViewId) {
        lock(&self.views).remove(&view);
    }
}

fn request_of<'a>(
    network: &'a mut VecDeque<NetworkEntry>,
    params: &Value,
) -> Option<&'a mut NetworkEntry> {
    let id = params["requestId"].as_str()?;
    network.iter_mut().rev().find(|entry| entry.id == id)
}

fn duration(started: f64, timestamp: &Value) -> Option<f64> {
    timestamp
        .as_f64()
        .filter(|_| started > 0.0)
        .map(|end| ((end - started) * 1000.0).max(0.0))
}

fn last<T: Clone>(buffer: &VecDeque<T>, limit: usize) -> Vec<T> {
    buffer
        .iter()
        .skip(buffer.len().saturating_sub(limit))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
