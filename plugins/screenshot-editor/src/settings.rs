//! The plugin's settings (`settings.schema.json`, Capture > Screenshot editor). The component
//! reads the export format and the JPEG quality; the drawing defaults (colour, stroke, text size,
//! redaction block) are the panel's, from its `settings` message. A value that does not read
//! falls back to the schema's default.

use rb_plugin_api::PluginHost;

use crate::paths::Format;

/// The schema's default JPEG quality.
pub const DEFAULT_QUALITY: u8 = 85;

/// What the component needs of the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub format: Format,
    pub jpeg_quality: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: Format::Png,
            jpeg_quality: DEFAULT_QUALITY,
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<serde_json::Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// The effective settings.
pub fn read(host: &mut dyn PluginHost) -> Settings {
    let defaults = Settings::default();
    let format = value(host, "format")
        .and_then(|v| v.as_str().and_then(Format::parse))
        .unwrap_or(defaults.format);
    let jpeg_quality = value(host, "jpegQuality")
        .and_then(|v| v.as_u64())
        .filter(|q| (10..=100).contains(q))
        .and_then(|q| u8::try_from(q).ok())
        .unwrap_or(defaults.jpeg_quality);
    Settings {
        format,
        jpeg_quality,
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
