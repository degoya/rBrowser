//! The host's capture: what the views reported, as the host hands it to every call.
//!
//! A component lives for one call, so it keeps no buffer of its own: the host captures (CDP
//! `Runtime.consoleAPICalled`, `Runtime.exceptionThrown`, `Log.entryAdded` and `Network.*` per
//! Chromium view, BiDi `log.entryAdded` per Firefox view, the sidecar's `console`/`pageerror`
//! per WebKit view) into its per-view buffers and hands them to each call of this plugin under
//! the argument [`FEED_KEY`] - in `call-tool`'s and `run-command`'s arguments and in the check
//! context's `args`, the way the core hands `visual-regression` its comparisons:
//!
//! ```json
//! { "$console": { "views": [ {
//!     "view": 3, "navigatedAt": 1731300000000.0,
//!     "console": [ { "level": "warning", "source": "console-api", "text": "...",
//!                    "url": "...", "line": 12, "column": 4, "stack": "...", "time": 1731300000123.0 } ],
//!     "errors":  [ { "text": "Uncaught TypeError: ...", "url": "...", "line": 3, "column": 9,
//!                    "stack": "...", "time": 1731300000124.0 } ],
//!     "network": [ { "url": "...", "method": "GET", "resourceType": "Fetch", "status": 404,
//!                    "failed": "net::ERR_...", "time": 1731300000125.0 } ] } ] } }
//! ```
//!
//! The three lists are the core's log buffers (`rb_mcp::logs`: `ConsoleEntry`, `ErrorEntry`,
//! `NetworkEntry`, camelCase) with an optional `stack` and `column`; `navigatedAt` is when the
//! view's top frame last navigated (milliseconds since the epoch). Without the key the console
//! is empty and says the host captures nothing ([`Feed::captured`]).

use rb_plugin_api::ViewId;
use serde::Deserialize;
use serde_json::Value;

use crate::normalize::cut;

/// The argument the host's capture arrives under.
pub const FEED_KEY: &str = "$console";
/// The source of what rBrowser's plugins logged in a view (the host's page-script log).
pub const RBROWSER_SOURCE: &str = "rbrowser";
/// The longest message text kept; longer ones are cut.
pub const MAX_TEXT_CHARS: usize = 2000;
/// The longest stack trace kept.
pub const MAX_STACK_CHARS: usize = 8000;

/// A console level; every one is filtered on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Error,
    Warning,
    Info,
    Log,
    Debug,
    /// A fetch or XHR request, failed or (with `networkLevel: all`) any.
    Network,
}

impl Level {
    /// Every level, in the panel's order.
    pub const ALL: [Self; 6] = [
        Self::Error,
        Self::Warning,
        Self::Info,
        Self::Log,
        Self::Debug,
        Self::Network,
    ];

    /// Its name in arguments, answers and settings.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Log => "log",
            Self::Debug => "debug",
            Self::Network => "network",
        }
    }

    /// The level named `name`, as [`Level::as_str`] writes it.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.as_str() == name)
    }

    /// The level of a console message's level as the engines name it: `console.assert` is an
    /// error, `warn` a warning, `verbose` debug, everything unknown (`dir`, `table`, `trace`) a
    /// log.
    #[must_use]
    pub fn of_console(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "error" | "assert" => Self::Error,
            "warning" | "warn" => Self::Warning,
            "info" => Self::Info,
            "debug" | "verbose" => Self::Debug,
            _ => Self::Log,
        }
    }
}

/// Where a message came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The page's `console.*`.
    Console,
    /// An uncaught exception.
    Exception,
    /// The browser itself (`network`, `security`, `violation`, ...).
    Browser,
    /// A fetch or XHR request.
    Network,
    /// rBrowser's own: what a plugin's page script logged (RB-080-66, source `rbrowser`), no
    /// message of the page; shown only with `console.showRbrowserMessages`, a page script's
    /// failure (with its stack) always.
    Rbrowser,
}

impl Kind {
    /// Its name in answers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Exception => "exception",
            Self::Browser => "browser",
            Self::Network => "network",
            Self::Rbrowser => "rbrowser",
        }
    }
}

/// Which requests reach the level network (`console.networkLevel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkLevel {
    Off,
    /// Failed ones: an error status (400 and up) or no answer.
    Errors,
    All,
}

impl NetworkLevel {
    /// The level named `name`; `errors` for anything else.
    #[must_use]
    pub fn parse(name: &str) -> Self {
        match name {
            "off" => Self::Off,
            "all" => Self::All,
            _ => Self::Errors,
        }
    }
}

/// One message of one view.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub view: ViewId,
    pub level: Level,
    pub kind: Kind,
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub stack: Option<String>,
    /// Milliseconds since the epoch.
    pub time: f64,
}

/// How the capture is read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeedOptions {
    /// The newest this many messages of each view are kept (`console.bufferSize`).
    pub buffer_size: usize,
    /// A view's messages before its last navigation are dropped (`console.clearOnNavigate`):
    /// those at or before the feed's `navigatedAt`, the newest time of the document before.
    pub clear_on_navigate: bool,
    pub network: NetworkLevel,
    /// Messages at or before this time were cleared (the clear mark, `state.rs`).
    pub cleared_at: f64,
    /// rBrowser's own messages ([`Kind::Rbrowser`]) are kept (`console.showRbrowserMessages`).
    pub rbrowser: bool,
}

/// The views' messages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feed {
    /// The host handed a capture (an empty one included).
    pub captured: bool,
    /// Every kept message, oldest first.
    pub entries: Vec<Entry>,
    /// The newest time in the capture, cleared messages included: what a clear marks.
    pub newest: f64,
}

#[derive(Deserialize)]
struct RawFeed {
    #[serde(default)]
    views: Vec<RawView>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawView {
    view: ViewId,
    #[serde(default)]
    navigated_at: Option<f64>,
    #[serde(default)]
    console: Vec<RawConsole>,
    #[serde(default)]
    errors: Vec<RawError>,
    #[serde(default)]
    network: Vec<RawNetwork>,
}

#[derive(Deserialize)]
struct RawConsole {
    #[serde(default)]
    level: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    text: String,
    url: Option<String>,
    line: Option<u64>,
    column: Option<u64>,
    stack: Option<String>,
    #[serde(default)]
    time: f64,
}

#[derive(Deserialize)]
struct RawError {
    #[serde(default)]
    text: String,
    url: Option<String>,
    line: Option<u64>,
    column: Option<u64>,
    stack: Option<String>,
    #[serde(default)]
    time: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNetwork {
    #[serde(default)]
    url: String,
    #[serde(default)]
    method: String,
    resource_type: Option<String>,
    status: Option<u64>,
    failed: Option<String>,
    #[serde(default)]
    time: f64,
}

/// The capture in `args` ([`FEED_KEY`]), read with `options`.
///
/// # Errors
///
/// The capture is there but has not the shape above; the text names why.
pub fn read(args: &Value, options: &FeedOptions) -> Result<Feed, String> {
    let Some(raw) = args.get(FEED_KEY) else {
        return Ok(Feed::default());
    };
    let raw: RawFeed = serde_json::from_value(raw.clone()).map_err(|error| error.to_string())?;
    let mut feed = Feed {
        captured: true,
        ..Feed::default()
    };
    for view in raw.views {
        let mut entries = view_entries(&view, options.network);
        if !options.rbrowser {
            // A page script's failure (it has a stack) shows all the same: rBrowser's own
            // failure, not noise (review of RB-080-66, 4).
            entries.retain(|entry| entry.kind != Kind::Rbrowser || entry.stack.is_some());
        }
        feed.newest = entries.iter().map(|e| e.time).fold(feed.newest, f64::max);
        let since = match view.navigated_at {
            Some(at) if options.clear_on_navigate => at,
            _ => f64::NEG_INFINITY,
        };
        entries.retain(|entry| entry.time > options.cleared_at && entry.time > since);
        entries.sort_by(|a, b| a.time.total_cmp(&b.time));
        let drop = entries.len().saturating_sub(options.buffer_size);
        feed.entries.extend(entries.into_iter().skip(drop));
    }
    feed.entries.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(feed)
}

fn view_entries(view: &RawView, network: NetworkLevel) -> Vec<Entry> {
    let mut entries = Vec::new();
    for message in &view.console {
        let kind = match message.source.as_str() {
            "" | "console-api" => Kind::Console,
            RBROWSER_SOURCE => Kind::Rbrowser,
            _ => Kind::Browser,
        };
        entries.push(Entry {
            view: view.view,
            level: Level::of_console(&message.level),
            kind,
            text: cut(&message.text, MAX_TEXT_CHARS),
            url: message.url.clone().filter(|url| !url.is_empty()),
            line: message.line,
            column: message.column,
            stack: message.stack.as_deref().map(|s| cut(s, MAX_STACK_CHARS)),
            time: message.time,
        });
    }
    for error in &view.errors {
        entries.push(Entry {
            view: view.view,
            level: Level::Error,
            kind: Kind::Exception,
            text: cut(&error.text, MAX_TEXT_CHARS),
            url: error.url.clone().filter(|url| !url.is_empty()),
            line: error.line,
            column: error.column,
            stack: error.stack.as_deref().map(|s| cut(s, MAX_STACK_CHARS)),
            time: error.time,
        });
    }
    entries.extend(
        view.network
            .iter()
            .filter_map(|request| network_entry(view.view, request, network)),
    );
    entries
}

fn network_entry(view: ViewId, request: &RawNetwork, level: NetworkLevel) -> Option<Entry> {
    let fetch_or_xhr = request
        .resource_type
        .as_deref()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("fetch") || kind.eq_ignore_ascii_case("xhr"));
    let failed = request.failed.is_some() || request.status.is_some_and(|status| status >= 400);
    let shown = match level {
        NetworkLevel::Off => false,
        NetworkLevel::Errors => failed,
        NetworkLevel::All => true,
    };
    if !fetch_or_xhr || !shown {
        return None;
    }
    let method = if request.method.is_empty() {
        "GET"
    } else {
        request.method.as_str()
    };
    let outcome = match (&request.failed, request.status) {
        (Some(reason), _) => reason.clone(),
        (None, Some(status)) => status.to_string(),
        (None, None) => "pending".to_owned(),
    };
    Some(Entry {
        view,
        level: Level::Network,
        kind: Kind::Network,
        text: cut(
            &format!("{method} {} {outcome}", request.url),
            MAX_TEXT_CHARS,
        ),
        url: Some(cut(&request.url, MAX_TEXT_CHARS)).filter(|url| !url.is_empty()),
        line: None,
        column: None,
        stack: None,
        time: request.time,
    })
}

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
