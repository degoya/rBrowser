//! A plugin command's answer asks for its panel (RB-060-12): `{"$open": {"panel": "<id>"}}`
//! opens the panels window at that panel - only one of the same plugin's panels the shell
//! offers. Anything else (another plugin's panel, an unknown or hidden one, a wrong shape) opens
//! nothing and is logged at debug level; `$open` never reaches the caller.

use serde_json::{Value, json};

use super::panels::PanelInfo;

/// The answer key that asks for a panel.
pub const OPEN_KEY: &str = "$open";

/// Splits plugin `plugin`'s answer into the answer without [`OPEN_KEY`] and the panel it asks
/// for, when `offered` has that panel of `plugin`.
#[must_use]
pub fn split_open(plugin: &str, answer: Value, offered: &[PanelInfo]) -> (Value, Option<String>) {
    let mut answer = answer;
    let Some(open) = answer
        .as_object_mut()
        .and_then(|fields| fields.remove(OPEN_KEY))
    else {
        return (answer, None);
    };
    let Some(panel) = open.get("panel").and_then(Value::as_str) else {
        tracing::debug!(
            plugin_id = plugin,
            "a plugin answer's `$open` has no panel id; ignored"
        );
        return (answer, None);
    };
    if offered
        .iter()
        .any(|info| info.plugin == plugin && info.panel == panel)
    {
        (answer, Some(panel.to_owned()))
    } else {
        tracing::debug!(
            plugin_id = plugin,
            "a plugin answer asks for a panel it does not offer; ignored"
        );
        (answer, None)
    }
}

/// The `pluginPanel` event that selects panel `panel` of plugin `plugin` in an open panels
/// window (a window opened anew reads it from its route).
#[must_use]
pub fn select_message(plugin: &str, panel: &str) -> Value {
    json!({ "select": { "plugin": plugin, "panel": panel } })
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
