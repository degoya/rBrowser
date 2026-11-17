//! The plugin's settings (Settings > DevTools & Debugging > Peek, `settings.schema.json`):
//! read through `settings.get` by property name, with the schema's defaults when a key is not
//! answered, and handed to the page script, which has no settings access of its own.

use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

/// The modifiers the inspector may be held with - each a single key held alone, so none is a
/// reserved combination: `Ctrl+Alt` is AltGr on German, French and Spanish layouts and
/// `Alt+Shift` switches the input language on Windows; neither is offered. Holding Alt alone,
/// without a letter, is no `accesskey`.
pub const MODIFIERS: [&str; 3] = ["Alt", "Ctrl", "Shift"];

/// The effective settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `peek.enabled`: whether the inspector reacts at all.
    pub enabled: bool,
    /// `peek.modifier`: one of [`MODIFIERS`].
    pub modifier: &'static str,
    /// `peek.syncHighlight`: the same element is outlined in the other views.
    pub sync_highlight: bool,
    /// `peek.showBoxModel`: margin, border, padding and content are drawn.
    pub show_box_model: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            modifier: MODIFIERS[0],
            sync_highlight: true,
            show_box_model: true,
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

impl Config {
    /// The settings as the host answers them; a missing or malformed value is the default.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let defaults = Self::default();
        let modifier = setting(host, "modifier")
            .and_then(|value| {
                let text = value.as_str()?.to_owned();
                MODIFIERS.into_iter().find(|known| *known == text)
            })
            .unwrap_or(defaults.modifier);
        Self {
            enabled: flag(host, "enabled", defaults.enabled),
            modifier,
            sync_highlight: flag(host, "syncHighlight", defaults.sync_highlight),
            show_box_model: flag(host, "showBoxModel", defaults.show_box_model),
        }
    }

    /// What the page script gets.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "enabled": self.enabled,
            "modifier": self.modifier,
            "syncHighlight": self.sync_highlight,
            "showBoxModel": self.show_box_model,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
