//! The plugin's settings (Settings > DevTools & Debugging > JSON/XML viewer,
//! `settings.schema.json`): read through `settings.get` by property name, each checked against
//! the schema's bounds - a missing, malformed or out-of-range value is the schema's default -
//! and handed to the page script, which has no settings access of its own.

use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

/// The smallest and largest `maxBytes`: 64 KiB to 256 MiB.
pub const MAX_BYTES_RANGE: (u64, u64) = (65_536, 268_435_456);
/// The fewest and most levels `expandDepth` opens.
pub const EXPAND_DEPTH_RANGE: (u64, u64) = (0, 16);
/// What a document opens in.
pub const VIEWS: [&str; 2] = ["tree", "raw"];
/// The colour themes; `auto` follows the view's `prefers-color-scheme`.
pub const THEMES: [&str; 3] = ["auto", "light", "dark"];

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `json-xml-viewer.enabled`: whether documents are shown as a tree at all.
    pub enabled: bool,
    /// `json-xml-viewer.maxBytes`: above this a document stays raw, with a notice.
    pub max_bytes: u64,
    /// `json-xml-viewer.defaultView`: one of [`VIEWS`].
    pub default_view: &'static str,
    /// `json-xml-viewer.expandDepth`: the levels open when a document opens.
    pub expand_depth: u64,
    /// `json-xml-viewer.theme`: one of [`THEMES`].
    pub theme: &'static str,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_bytes: 20 * 1024 * 1024,
            default_view: VIEWS[0],
            expand_depth: 2,
            theme: THEMES[0],
        }
    }
}

fn setting(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn flag(host: &mut dyn PluginHost, key: &str, default: bool) -> bool {
    setting(host, key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn integer(host: &mut dyn PluginHost, key: &str, (low, high): (u64, u64), default: u64) -> u64 {
    setting(host, key)
        .and_then(|value| value.as_u64())
        .filter(|value| (low..=high).contains(value))
        .unwrap_or(default)
}

fn choice(
    host: &mut dyn PluginHost,
    key: &str,
    allowed: &[&'static str],
    default: &'static str,
) -> &'static str {
    setting(host, key)
        .and_then(|value| {
            let text = value.as_str()?.to_owned();
            allowed.iter().copied().find(|known| *known == text)
        })
        .unwrap_or(default)
}

impl Config {
    /// The settings as the host answers them.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        Self {
            enabled: flag(host, "enabled", defaults.enabled),
            max_bytes: integer(host, "maxBytes", MAX_BYTES_RANGE, defaults.max_bytes),
            default_view: choice(host, "defaultView", &VIEWS, defaults.default_view),
            expand_depth: integer(
                host,
                "expandDepth",
                EXPAND_DEPTH_RANGE,
                defaults.expand_depth,
            ),
            theme: choice(host, "theme", &THEMES, defaults.theme),
        }
    }

    /// What the page script gets.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "enabled": self.enabled,
            "maxBytes": self.max_bytes,
            "defaultView": self.default_view,
            "expandDepth": self.expand_depth,
            "theme": self.theme,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
