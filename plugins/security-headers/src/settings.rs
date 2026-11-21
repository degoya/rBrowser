//! The plugin's settings (`settings.schema.json`, Settings > Checks > Security headers), read at
//! each call; a missing or malformed key keeps its default (a hand-edited config may hold
//! anything). The rules' on/off switch and severity are the host's
//! `checks.rules.security-headers.<rule>` ([`crate::levels`]), as for every check plugin.

use rb_plugin_api::PluginHost;
use serde_json::Value;

/// One year, the `max-age` the HSTS preload list and Lighthouse ask for.
pub const DEFAULT_HSTS_MIN_AGE: u64 = 31_536_000;
/// The largest `hstsMinAge` taken: two years.
pub const MAX_HSTS_MIN_AGE: u64 = 63_072_000;

/// The effective settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The smallest `Strict-Transport-Security` `max-age` not reported, in seconds
    /// (`hstsMinAge`).
    pub hsts_min_age: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hsts_min_age: DEFAULT_HSTS_MIN_AGE,
        }
    }
}

impl Settings {
    /// The settings as the host holds them.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let hsts_min_age = host
            .setting("hstsMinAge")
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| value.as_u64())
            .filter(|age| *age <= MAX_HSTS_MIN_AGE)
            .unwrap_or(DEFAULT_HSTS_MIN_AGE);
        Self { hsts_min_age }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
