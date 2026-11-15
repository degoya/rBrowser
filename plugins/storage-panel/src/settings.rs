//! The plugin's settings (`settings.schema.json`, Settings > DevTools & Debugging > Storage),
//! each read at the call that needs it; a missing or malformed value falls back to its default.

use rb_plugin_api::PluginHost;
use serde_json::Value;

/// The panel's refresh interval by default, in milliseconds (`0`: no polling).
pub const DEFAULT_POLL_MS: u64 = 2000;
/// The longest refresh interval.
pub const MAX_POLL_MS: u64 = 60_000;

/// The effective settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `storage-panel.pollMs`: how often the open panel reads the views again.
    pub poll_ms: u64,
    /// `storage-panel.showHttpOnly`: HttpOnly cookies in the panel and in `storage_get`.
    pub show_http_only: bool,
    /// `storage-panel.pinsEnabled`: pinned values are set again after each navigation.
    pub pins_enabled: bool,
    /// `storage-panel.mcpValues` (protected): MCP may read values and write them.
    pub mcp_values: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_ms: DEFAULT_POLL_MS,
            show_http_only: true,
            pins_enabled: true,
            mcp_values: false,
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn flag(host: &mut dyn PluginHost, key: &str, default: bool) -> bool {
    value(host, key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

impl Settings {
    /// The settings as the host holds them now.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            poll_ms: value(host, "pollMs")
                .and_then(|value| value.as_u64())
                .filter(|ms| *ms <= MAX_POLL_MS)
                .unwrap_or(defaults.poll_ms),
            show_http_only: flag(host, "showHttpOnly", defaults.show_http_only),
            pins_enabled: flag(host, "pinsEnabled", defaults.pins_enabled),
            mcp_values: flag(host, "mcpValues", defaults.mcp_values),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
