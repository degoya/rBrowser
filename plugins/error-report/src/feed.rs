//! The host's capture, as it hands it to every call (capability `console-logs`).
//!
//! A component lives for one call and the contract delivers no CDP event, so the host keeps
//! per-view buffers (CDP `Runtime`, `Log`, `Network` and `Audits` on every Chromium view) and puts
//! them into the arguments of each `run-command`, `call-tool` and check run under [`FEED_KEY`]:
//!
//! ```json
//! { "$console": { "views": [ {
//!     "view": 3,
//!     "console": [ { "level": "error", "source": "security", "text": "...", "url": "...",
//!                    "line": 12, "time": 1731300000123.0 } ],
//!     "errors":  [ { "text": "Uncaught TypeError: ...\n    at f (...)", "url": "...",
//!                    "line": 3, "column": 9, "time": 1731300000124.0 } ],
//!     "network": [ { "id": "1.2", "url": "...", "method": "GET", "resourceType": "Image",
//!                    "status": 404, "failed": "net::ERR_...", "time": 1731300000125.0 } ],
//!     "issues":  [ { "code": "ContentSecurityPolicyIssue", "kind": "img-src",
//!                    "detail": "kURLViolation", "url": "...", "sourceUrl": "...", "line": 4,
//!                    "column": 1, "reportOnly": true, "time": 1731300000126.0 } ] } ] } }
//! ```
//!
//! The lists are the core's log buffers (`rb_mcp::logs`, camelCase), texts and URLs masked by the
//! host. A feed without `issues` for a view (a host before RB-080-09) leaves CSP violations, mixed
//! content and deprecations to the console's `security` and `deprecation` messages
//! ([`crate::classify`]). Without the key the report is empty and says the host captures nothing.

use rb_plugin_api::ViewId;
use serde::Deserialize;
use serde_json::Value;

/// The argument the host's capture arrives under.
pub const FEED_KEY: &str = "$console";

/// One console message.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Console {
    pub level: String,
    pub source: String,
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u64>,
    pub time: f64,
}

/// One uncaught exception; V8 writes the stack trace into its text after the first line.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Exception {
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub time: f64,
}

/// One request.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Request {
    pub url: String,
    pub method: String,
    pub resource_type: Option<String>,
    pub status: Option<u64>,
    pub mime_type: Option<String>,
    pub failed: Option<String>,
    pub time: f64,
}

/// One `Audits` issue: a CSP violation, mixed content or a deprecation.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub url: Option<String>,
    pub source_url: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub report_only: bool,
    pub time: f64,
}

/// What one view reported.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ViewFeed {
    pub view: ViewId,
    pub console: Vec<Console>,
    pub errors: Vec<Exception>,
    pub network: Vec<Request>,
    /// `None` when the host sends no issues at all (not: none happened).
    pub issues: Option<Vec<Issue>>,
}

impl ViewFeed {
    /// The newest time of any entry; `0` for none.
    #[must_use]
    pub fn newest(&self) -> f64 {
        let times = self
            .console
            .iter()
            .map(|e| e.time)
            .chain(self.errors.iter().map(|e| e.time))
            .chain(self.network.iter().map(|e| e.time))
            .chain(self.issues.iter().flatten().map(|e| e.time));
        times.fold(0.0, f64::max)
    }
}

/// The views' capture.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feed {
    /// The host handed a capture (an empty one included).
    pub captured: bool,
    pub views: Vec<ViewFeed>,
}

impl Feed {
    /// The newest time in the capture: what a clear marks.
    #[must_use]
    pub fn newest(&self) -> f64 {
        self.views.iter().map(ViewFeed::newest).fold(0.0, f64::max)
    }
}

#[derive(Deserialize)]
struct RawFeed {
    #[serde(default)]
    views: Vec<ViewFeed>,
}

/// The capture in `args` ([`FEED_KEY`]).
///
/// # Errors
///
/// The capture is there but has not the shape above; the text names why.
pub fn read(args: &Value) -> Result<Feed, String> {
    let Some(raw) = args.get(FEED_KEY) else {
        return Ok(Feed::default());
    };
    let raw: RawFeed = serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    Ok(Feed {
        captured: true,
        views: raw.views,
    })
}

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
