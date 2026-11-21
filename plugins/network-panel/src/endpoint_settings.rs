//! The endpoint analysis' settings (RB-096-06, Settings > Network > Endpoints): which resource
//! types count, the person's path patterns and the limits of the check rules. Read anew at each
//! call; a value out of its bounds is held to them, one that does not read keeps the default.
//! Whether a rule runs at all is the person's level of it (`checks.rules.network-panel.*`).

use std::collections::BTreeSet;

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::template::{MAX_PATTERN, MAX_PATTERNS, Template, templates};

/// The resource types the setting `endpoints.resourceTypes` offers (CDP's names).
pub const TYPES: &[&str] = &[
    "Document",
    "XHR",
    "Fetch",
    "EventSource",
    "WebSocket",
    "Script",
    "Stylesheet",
    "Image",
    "Font",
    "Media",
    "Other",
];
/// The types counted by default: the page's API calls.
pub const DEFAULT_TYPES: &[&str] = &["XHR", "Fetch"];

/// The limits of the check rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// `endpoint.slow`: a p95 duration at or above this many milliseconds.
    pub slow_ms: f64,
    /// `endpoint.uncompressed`: a body of at least this many bytes sent uncompressed.
    pub large_bytes: f64,
    /// `endpoint.n-plus-one`: this many different calls of one endpoint in one page load.
    pub n_plus_one: usize,
    /// `endpoint.breakpoint-calls`: one breakpoint calls an endpoint this many times more often.
    pub breakpoint_difference: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            slow_ms: 1000.0,
            large_bytes: 10.0 * 1024.0,
            n_plus_one: 5,
            breakpoint_difference: 2,
        }
    }
}

/// The settings in effect.
#[derive(Clone, Debug, PartialEq)]
pub struct EndpointSettings {
    /// Empty: every type.
    pub types: BTreeSet<String>,
    pub templates: Vec<Template>,
    pub limits: Limits,
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn number(host: &mut dyn PluginHost, key: &str, low: u64, high: u64) -> Option<u64> {
    value(host, key)
        .and_then(|value| value.as_u64())
        .map(|n| n.clamp(low, high))
}

/// The known types of `list`.
#[must_use]
pub fn known_types(list: &[Value]) -> BTreeSet<String> {
    list.iter()
        .filter_map(Value::as_str)
        .filter(|kind| TYPES.contains(kind))
        .map(str::to_owned)
        .collect()
}

impl EndpointSettings {
    /// The settings the host answers.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let types = match value(host, "endpoints.resourceTypes") {
            Some(Value::Array(list)) => known_types(&list),
            _ => DEFAULT_TYPES
                .iter()
                .map(|kind| (*kind).to_owned())
                .collect(),
        };
        let patterns: Vec<String> = match value(host, "endpoints.patterns") {
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(Value::as_str)
                .filter(|text| text.len() <= MAX_PATTERN)
                .take(MAX_PATTERNS)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        let mut limits = Limits::default();
        if let Some(ms) = number(host, "endpoints.slowMs", 1, 600_000) {
            limits.slow_ms = ms as f64;
        }
        if let Some(bytes) = number(host, "endpoints.largeBytes", 1024, 64 * 1024 * 1024) {
            limits.large_bytes = bytes as f64;
        }
        if let Some(n) = number(host, "endpoints.nPlusOne", 2, 100) {
            limits.n_plus_one = usize::try_from(n).unwrap_or(limits.n_plus_one);
        }
        if let Some(n) = number(host, "endpoints.breakpointDifference", 1, 100) {
            limits.breakpoint_difference =
                usize::try_from(n).unwrap_or(limits.breakpoint_difference);
        }
        Self {
            types,
            templates: templates(&patterns),
            limits,
        }
    }
}

#[cfg(test)]
#[path = "endpoint_settings_tests.rs"]
mod tests;
