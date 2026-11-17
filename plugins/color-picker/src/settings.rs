//! The plugin's settings (`settings.schema.json`, Settings > DevTools & Debugging > Colour picker
//! & measuring), read at the call that needs them; a missing or malformed value falls back to its
//! default.

use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

use crate::convert::Format;
use crate::pixels::loupe_size;

/// The longest selector taken (setting, panel or MCP).
pub const MAX_SELECTOR: usize = 1024;

/// How the pipette reads a view (`color-picker.capture`, RB-080-77).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    /// The view as its window shows it: no flicker; on a display with a colour profile in the
    /// display's colours.
    Window,
    /// A clip of the page: its own colours; the view flickers at every read.
    Clip,
}

impl Capture {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Clip => "clip",
        }
    }
}

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `color-picker.defaultFormat`: the format shown first and copied by Enter.
    pub format: Format,
    /// `color-picker.loupeSize`: device pixels per side of the loupe (odd, 5-31).
    pub loupe: u32,
    /// `color-picker.capture`: how the pipette reads a view.
    pub capture: Capture,
    /// `color-picker.textMeasure.selector`: what the measurement takes without a choice.
    pub selector: String,
    /// `color-picker.textMeasure.optimalMin` / `optimalMax`: the comfortable characters per line.
    pub optimal_min: u32,
    pub optimal_max: u32,
    /// `color-picker.textMeasure.warnAbove`: lines with more characters are marked (WCAG 1.4.8).
    pub warn_above: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: Format::Hex,
            loupe: 11,
            capture: Capture::Window,
            selector: "p, li".to_owned(),
            optimal_min: 45,
            optimal_max: 75,
            warn_above: 80,
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn count(host: &mut dyn PluginHost, key: &str) -> Option<u32> {
    value(host, key)
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| (1..=1000).contains(value))
}

impl Settings {
    /// The settings as the host holds them now.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        let format = value(host, "defaultFormat")
            .and_then(|value| value.as_str().map(Format::from_name))
            .unwrap_or(defaults.format);
        let loupe = value(host, "loupeSize")
            .and_then(|value| value.as_u64())
            .and_then(|value| loupe_size(value).ok())
            .unwrap_or(defaults.loupe);
        let capture = match value(host, "capture").as_ref().and_then(Value::as_str) {
            Some("clip") => Capture::Clip,
            _ => defaults.capture,
        };
        let selector = value(host, "textMeasure.selector")
            .and_then(|value| value.as_str().map(str::trim).map(str::to_owned))
            .filter(|text| !text.is_empty() && text.chars().count() <= MAX_SELECTOR)
            .unwrap_or_else(|| defaults.selector.clone());
        let (optimal_min, optimal_max) = match (
            count(host, "textMeasure.optimalMin"),
            count(host, "textMeasure.optimalMax"),
        ) {
            (Some(min), Some(max)) if min <= max => (min, max),
            (Some(min), None) if min <= defaults.optimal_max => (min, defaults.optimal_max),
            (None, Some(max)) if max >= defaults.optimal_min => (defaults.optimal_min, max),
            _ => (defaults.optimal_min, defaults.optimal_max),
        };
        let warn_above = count(host, "textMeasure.warnAbove").unwrap_or(defaults.warn_above);
        Self {
            format,
            loupe,
            capture,
            selector,
            optimal_min,
            optimal_max,
            warn_above,
        }
    }

    /// The settings as the page script and the panel take them.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "format": self.format.name(),
            "loupeSize": self.loupe,
            "capture": self.capture.name(),
            "textMeasure": {
                "selector": self.selector,
                "optimalMin": self.optimal_min,
                "optimalMax": self.optimal_max,
                "warnAbove": self.warn_above,
            },
        })
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
