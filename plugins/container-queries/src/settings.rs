//! The plugin's settings (`settings.schema.json`, DevTools & Debugging > Container queries):
//! `container-queries.overlay.labels` - the labels beside each container - and
//! `container-queries.overlay.color` - the colour of its frame, label and handle.

use rb_plugin_api::PluginHost;
use serde::Serialize;
use serde_json::Value;

/// The default frame colour (a teal that shows on light and dark pages).
pub const DEFAULT_COLOR: &str = "#0d9488";

/// What the page script draws with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PageSettings {
    pub labels: bool,
    pub color: String,
}

impl Default for PageSettings {
    fn default() -> Self {
        Self {
            labels: true,
            color: DEFAULT_COLOR.to_owned(),
        }
    }
}

/// The value of the plugin's key `key` (`overlay.labels`); the host may name it with the
/// plugin's id in front (`container-queries.overlay.labels`), so both are asked.
fn get(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .or_else(|| host.setting(&format!("{}.{key}", crate::PLUGIN_ID)))
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// Whether `text` is a colour `#rrggbb`.
#[must_use]
pub fn is_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text.chars().skip(1).all(|c| c.is_ascii_hexdigit())
}

/// The settings, every value that is missing or out of its range its default.
pub fn page_settings(host: &mut dyn PluginHost) -> PageSettings {
    let defaults = PageSettings::default();
    PageSettings {
        labels: get(host, "overlay.labels")
            .and_then(|value| value.as_bool())
            .unwrap_or(defaults.labels),
        color: get(host, "overlay.color")
            .and_then(|value| value.as_str().map(str::to_owned))
            .filter(|color| is_color(color))
            .unwrap_or(defaults.color),
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
