//! The plugin's settings (`settings.schema.json`, Settings > Checks > PWA), read at each call and
//! held to the schema's bounds again: a hand-edited config may hold anything. Rules on/off and
//! their severity are the host's `checks.rules.pwa-check.<rule>`, as for every check plugin.

use rb_plugin_api::PluginHost;
use serde_json::Value;

/// The shortest time budget of one check, in milliseconds.
pub const MIN_TIMEOUT_MS: u64 = 5_000;
/// The longest: below the manifest's call timeout (60 s), so a check ends with its findings.
pub const MAX_TIMEOUT_MS: u64 = 55_000;

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Whether the offline test asks the Cache Storage for the start URL.
    pub offline: bool,
    /// No further request once one check has run this long, in milliseconds.
    pub timeout_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            offline: true,
            timeout_ms: 30_000,
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

impl Settings {
    /// The settings as the host holds them; a missing or malformed key keeps its default.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            offline: read(host, "offline.enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.offline),
            timeout_ms: read(host, "timeoutMs")
                .and_then(|value| value.as_u64())
                .map_or(defaults.timeout_ms, |value| {
                    value.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
                }),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
