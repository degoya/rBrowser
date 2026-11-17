//! The views' console, errors and network requests for the plugins granted `console-logs`
//! (Charge 6, for RB-060-02): a component lives for one call and the contract delivers no CDP
//! event, so the host keeps the buffers - `rb_mcp::LogStore`, fed by the same collector as MCP's
//! `console_logs` (`crate::mcp::collect`) - and hands them to every call of such a plugin as the
//! argument `$console`: in `call-tool`'s and `run-command`'s arguments and in a check run's
//! context `args`. A `$console` the caller supplied is overwritten, so no caller can forge one.
//!
//! The feed is `{"views": [{"view", "console", "errors", "network", "issues", "navigatedAt"}]}`,
//! the entries in the shapes of `rb_mcp::logs` (camelCase; `issues` the `Audits` issues of CSP
//! violations, mixed content and deprecations, RB-080-09), at most [`MAX_ENTRIES`] of each list
//! per view, newest last; `navigatedAt`, once the view's main frame loaded a new document, is the
//! newest time recorded before it (`LogStore::navigated_at`) - what is newer is the current
//! document's (`console.clearOnNavigate`). The collector runs only while a loaded plugin is granted `console-logs`.
//! A view's `console` list also carries what the plugins' page scripts logged (RB-080-66,
//! `super::page_log`), with the source `rbrowser`, the plugin's id and its sentence.

use std::sync::{Arc, Mutex, Weak};

use rb_engine::{ViewHost as _, ViewId};
use rb_mcp::logs::NetworkEntry;
use rb_mcp::{LogLimits, LogStore};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

/// The argument the feed arrives under.
pub const FEED_KEY: &str = "$console";
/// The most entries of each list per view handed over.
pub const MAX_ENTRIES: usize = 200;

/// The buffers of one window and the collector feeding them.
pub struct ConsoleFeed {
    host: Weak<Host>,
    store: Arc<LogStore>,
    /// Dropped with the feed: the collector stops.
    _stop: watch::Sender<bool>,
}

static FEED: Mutex<Option<Arc<ConsoleFeed>>> = Mutex::new(None);

/// Starts the buffers of `core`'s window (once per window).
pub fn start(core: &Core) {
    let mut feed = lock(&FEED);
    if feed
        .as_ref()
        .is_some_and(|running| std::ptr::eq(running.host.as_ptr(), Arc::as_ptr(core.host())))
    {
        return;
    }
    let user = core.user_settings().mcp;
    let store = Arc::new(LogStore::new(LogLimits {
        console: user.console_log_limit as usize,
        network: user.network_request_limit as usize,
        errors: user.error_limit as usize,
    }));
    let (stop, stopped) = watch::channel(false);
    crate::mcp::collect::spawn(core, Arc::clone(&store), stopped);
    *feed = Some(Arc::new(ConsoleFeed {
        host: Arc::downgrade(core.host()),
        store,
        _stop: stop,
    }));
}

/// The request `id` of `view` as the window's buffers recorded it (RB-080-37, a repeat).
#[must_use]
pub fn request(view: ViewId, id: &str) -> Option<NetworkEntry> {
    let running = lock(&FEED).clone();
    running.and_then(|feed| feed.store.request(view, id))
}

/// The feed of the running window; `{"views": []}` without one.
pub fn current() -> Value {
    let running = lock(&FEED).clone();
    running.map_or_else(|| json!({ "views": [] }), |feed| feed.snapshot())
}

impl ConsoleFeed {
    fn snapshot(&self) -> Value {
        let views: Vec<ViewId> = self
            .host
            .upgrade()
            .map(|host| host.views().iter().map(|view| view.id).collect())
            .unwrap_or_default();
        let mut feed = feed_of(&self.store, &views);
        add_plugin_lines(&mut feed, super::page_log::lines_of);
        feed
    }
}

/// Appends to each view's `console` list of `feed` what the plugins' page scripts logged in it.
pub fn add_plugin_lines(feed: &mut Value, lines_of: impl Fn(ViewId) -> Vec<Value>) {
    let Some(views) = feed["views"].as_array_mut() else {
        return;
    };
    for view in views {
        let Some(id) = view["view"].as_u64().and_then(|id| u32::try_from(id).ok()) else {
            continue;
        };
        let lines = lines_of(ViewId(id));
        if let Some(console) = view["console"].as_array_mut() {
            console.extend(lines);
        }
    }
}

/// The feed of `views` from `store`.
#[must_use]
pub fn feed_of(store: &LogStore, views: &[ViewId]) -> Value {
    let views: Vec<Value> = views
        .iter()
        .map(|view| {
            let mut entry = json!({
                "view": view.0,
                "console": store.console(*view, MAX_ENTRIES),
                "errors": store.errors(*view, MAX_ENTRIES),
                "network": store.network(*view, MAX_ENTRIES),
                "issues": store.issues(*view, MAX_ENTRIES),
            });
            if let Some(at) = store.navigated_at(*view) {
                entry["navigatedAt"] = json!(at);
            }
            entry
        })
        .collect();
    json!({ "views": views })
}

/// `args` with `feed` under [`FEED_KEY`], over anything the caller put there; `null` becomes an
/// object, other non-objects stay as they are.
#[must_use]
pub fn with_feed(args: &Value, feed: Value) -> Value {
    match args {
        Value::Object(map) => {
            let mut map = map.clone();
            map.insert(FEED_KEY.to_owned(), feed);
            Value::Object(map)
        }
        Value::Null => json!({ FEED_KEY: feed }),
        other => other.clone(),
    }
}

/// `args` as JSON text with the current feed, for a check run's context.
#[must_use]
pub fn with_current_feed(args: &str) -> String {
    let parsed: Value = serde_json::from_str(args).unwrap_or(Value::Null);
    with_feed(&parsed, current()).to_string()
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
