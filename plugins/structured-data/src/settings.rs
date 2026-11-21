//! The plugin's settings (`settings.schema.json`, Settings > Checks > Structured data), read at
//! each call; a missing or malformed key keeps its default (a hand-edited config may hold
//! anything). Rules on/off and their severity are the host's `checks.rules.structured-data.<rule>`,
//! as for every check plugin.

use rb_plugin_api::{PluginHost, Severity};
use serde_json::Value;

/// The effective settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Whether the rich-result needs and recommendations are checked (`richResults`).
    pub rich: bool,
    /// The mildest severity shown and reported (`minSeverity`).
    pub min_severity: Severity,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rich: true,
            min_severity: Severity::Info,
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

impl Settings {
    /// The settings as the host holds them.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            rich: read(host, "richResults")
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.rich),
            min_severity: read(host, "minSeverity")
                .and_then(|value| value.as_str().and_then(Severity::parse))
                .unwrap_or(defaults.min_severity),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
