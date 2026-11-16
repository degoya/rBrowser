//! What the host hands every call: `$network` (capability `network-rules`: this run's session,
//! workspace trust, the hits and rejected rules, the views and whether each is intercepted, and
//! the debugger's parts of RB-080-37 - the held requests, the answers to the asks, the composed
//! results, the breakpoints' state) and the settings in effect. A component keeps nothing
//! between calls; this is read anew each time.

use std::collections::BTreeMap;

use rb_plugin_api::{PluginHost, ViewId};
use serde_json::{Map, Value};

/// The argument the host's state arrives under.
pub const STATE_KEY: &str = "$network";
/// The argument the host's capture arrives under.
pub const FEED_KEY: &str = "$console";
/// The default folder of the mock files.
pub const DEFAULT_MOCKS_DIR: &str = ".rbrowser/mocks";

/// One view as the host intercepts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    pub view: ViewId,
    /// Its engine is intercepted at all (Chromium for now).
    pub supported: bool,
    /// Requests of it pass the rules right now.
    pub intercepting: bool,
}

/// The host's `$network` state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    /// Empty when the host sent none (the app does not run the rules).
    pub session: String,
    pub trusted: bool,
    pub hits: BTreeMap<String, u64>,
    pub rejected: Vec<(String, String)>,
    pub applied: u64,
    pub cache_disabled: bool,
    pub views: Vec<ViewState>,
    /// The host sent a state at all.
    pub present: bool,
    /// The requests breakpoints hold, masked, oldest first (RB-080-37).
    pub held: Vec<Value>,
    /// The host's answers to the asks, by ask id.
    pub answers: Map<String, Value>,
    /// The newest composed and repeated results, masked.
    pub composed: Vec<Value>,
    /// Whether the host holds any breakpoint on, and how many apply.
    pub breakpoints_enabled: bool,
    pub breakpoints_active: u64,
    /// Response bodies a rewrite left alone (over the limit), per rule.
    pub skipped: BTreeMap<String, u64>,
}

impl Context {
    /// The state in `args`; an empty one (untrusted, nothing applied) when there is none.
    #[must_use]
    pub fn read(args: &Value) -> Self {
        let state = &args[STATE_KEY];
        if !state.is_object() {
            return Self::default();
        }
        Self {
            session: state["session"].as_str().unwrap_or_default().to_owned(),
            trusted: state["trusted"] == Value::Bool(true),
            hits: state["hits"]
                .as_object()
                .map(|hits| {
                    hits.iter()
                        .filter_map(|(id, n)| Some((id.clone(), n.as_u64()?)))
                        .collect()
                })
                .unwrap_or_default(),
            rejected: state["rejected"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|entry| {
                            Some((
                                entry["id"].as_str()?.to_owned(),
                                entry["reason"].as_str()?.to_owned(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            applied: state["applied"].as_u64().unwrap_or(0),
            cache_disabled: state["cacheDisabled"] == Value::Bool(true),
            views: state["views"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|entry| {
                            Some(ViewState {
                                view: ViewId::try_from(entry["view"].as_u64()?).ok()?,
                                supported: entry["supported"] == Value::Bool(true),
                                intercepting: entry["intercepting"] == Value::Bool(true),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            present: true,
            held: state["held"].as_array().cloned().unwrap_or_default(),
            answers: state["answers"].as_object().cloned().unwrap_or_default(),
            composed: state["composed"].as_array().cloned().unwrap_or_default(),
            breakpoints_enabled: state["breakpoints"]["enabled"] != Value::Bool(false),
            breakpoints_active: state["breakpoints"]["active"].as_u64().unwrap_or(0),
            skipped: state["skipped"]
                .as_object()
                .map(|skipped| {
                    skipped
                        .iter()
                        .filter_map(|(id, n)| Some((id.clone(), n.as_u64()?)))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// The plugin's settings (`settings.schema.json`, Settings > Network).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Mocks apply at the start of a session (the panel and `Ctrl+Shift+Space P M` switch them
    /// for the session).
    pub mocks_enabled: bool,
    /// Below `.rbrowser/`; the default when the setting is no such path.
    pub mocks_dir: String,
    /// HAR exports carry the response bodies the views still hold.
    pub har_bodies: bool,
    /// URL globs blocked in every view, trusted workspace or not (the person's own list).
    pub block: Vec<String>,
    /// The cache is off at the start of a session.
    pub cache_disabled: bool,
    /// A held request goes on unchanged after this long; 0 never (RB-080-37).
    pub breakpoint_timeout_ms: u64,
    /// `network_compose` and `network_repeat` over MCP send the person's cookies.
    pub allow_compose_from_mcp: bool,
    /// Folders map-local rules may serve from, beside the project.
    pub map_local_roots: Vec<String>,
    /// Response bodies larger than this are not rewritten.
    pub rewrite_max_body_bytes: u64,
}

/// The default of `breakpoints.timeoutMs`.
pub const DEFAULT_BREAKPOINT_TIMEOUT_MS: u64 = 30_000;
/// The default of `rewrite.maxBodyBytes` (4 MiB).
pub const DEFAULT_MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;

impl Default for Settings {
    fn default() -> Self {
        Self {
            mocks_enabled: true,
            mocks_dir: DEFAULT_MOCKS_DIR.to_owned(),
            har_bodies: true,
            block: Vec::new(),
            cache_disabled: false,
            breakpoint_timeout_ms: DEFAULT_BREAKPOINT_TIMEOUT_MS,
            allow_compose_from_mcp: false,
            map_local_roots: Vec::new(),
            rewrite_max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

impl Settings {
    /// The settings the host answers; the default for each key it has not or that does not read.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let mut settings = Self::default();
        if let Some(Value::Bool(on)) = value(host, "mocksEnabled") {
            settings.mocks_enabled = on;
        }
        if let Some(Value::String(dir)) = value(host, "mocksDir") {
            let dir = dir.trim_end_matches('/');
            if dir
                .strip_prefix(".rbrowser/")
                .is_some_and(crate::mocks::relative_path)
            {
                settings.mocks_dir = dir.to_owned();
            }
        }
        if let Some(Value::Bool(on)) = value(host, "harBodies") {
            settings.har_bodies = on;
        }
        if let Some(Value::Array(patterns)) = value(host, "block") {
            settings.block = patterns
                .iter()
                .filter_map(Value::as_str)
                .filter(|pattern| !pattern.is_empty() && pattern.len() <= 2048)
                .take(64)
                .map(str::to_owned)
                .collect();
        }
        if let Some(Value::Bool(off)) = value(host, "cacheDisabled") {
            settings.cache_disabled = off;
        }
        if let Some(timeout) = value(host, "breakpoints.timeoutMs").and_then(|v| v.as_u64()) {
            settings.breakpoint_timeout_ms = timeout.min(600_000);
        }
        if let Some(Value::Bool(on)) = value(host, "allowComposeFromMcp") {
            settings.allow_compose_from_mcp = on;
        }
        if let Some(Value::Array(roots)) = value(host, "mapLocalRoots") {
            settings.map_local_roots = roots
                .iter()
                .filter_map(Value::as_str)
                .filter(|root| !root.is_empty() && root.len() <= 1024)
                .take(32)
                .map(str::to_owned)
                .collect();
        }
        if let Some(bytes) = value(host, "rewrite.maxBodyBytes").and_then(|v| v.as_u64()) {
            settings.rewrite_max_body_bytes = bytes.min(64 * 1024 * 1024);
        }
        settings
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
