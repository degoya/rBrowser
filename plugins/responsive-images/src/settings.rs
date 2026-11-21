//! The plugin's settings (`settings.schema.json`, Settings > Checks > Responsive images), read at
//! each call; a missing, malformed or out-of-range key keeps its default (a hand-edited config may
//! hold anything). Rules on/off and their severity are the host's
//! `checks.rules.responsive-images.<rule>`, as for every check plugin.

use rb_plugin_api::PluginHost;
use serde_json::Value;

/// The range of `oversizeFactor` (the schema's).
pub const OVERSIZE_RANGE: (f64, f64) = (1.1, 10.0);
/// The range of `undersizeFactor` (the schema's).
pub const UNDERSIZE_RANGE: (f64, f64) = (0.1, 1.0);

/// The effective settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// A file with more than this many times the pixels needed is too large.
    pub oversize_factor: f64,
    /// A file with less than this many times the pixels needed is too small.
    pub undersize_factor: f64,
    /// Whether the LCP image is checked for `loading` and `fetchpriority`.
    pub check_lcp: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            oversize_factor: 1.5,
            undersize_factor: 1.0,
            check_lcp: true,
        }
    }
}

fn read(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn factor(value: Option<Value>, (low, high): (f64, f64)) -> Option<f64> {
    value
        .and_then(|value| value.as_f64())
        .filter(|factor| (low..=high).contains(factor))
}

impl Settings {
    /// The settings as the host holds them.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            oversize_factor: factor(read(host, "oversizeFactor"), OVERSIZE_RANGE)
                .unwrap_or(defaults.oversize_factor),
            undersize_factor: factor(read(host, "undersizeFactor"), UNDERSIZE_RANGE)
                .unwrap_or(defaults.undersize_factor),
            check_lcp: read(host, "checkLcp")
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.check_lcp),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
