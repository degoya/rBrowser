//! A plugin command's answer asks for its panel (RB-060-12): `{"$open": {"panel": "<id>"}}`
//! opens the panels window at that panel - only one of the same plugin's panels the shell
//! offers. Anything else (another plugin's panel, an unknown or hidden one, a wrong shape) opens
//! nothing and is logged at debug level; `$open` never reaches the caller.
//!
//! A link can ask for a panel's tab too (RB-094-27: a link to `api-client.secrets` opens the API
//! client at Variables, [`open_at`]): the tab waits for the panel's next read of its state
//! ([`take_tab`]) - a panel that is not open yet reads it when it starts - and an open one is told
//! to read it by the event `{"tab": true}`.

use std::collections::BTreeMap;
use std::sync::Mutex;

use rb_util::lock;
use serde_json::{Value, json};

use super::panels::PanelInfo;
use crate::control::Core;

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

/// The tab each plugin's panel was asked to show, until the panel reads it.
static ASKED_TABS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// Asks plugin `plugin`'s panel to show `tab`; it waits for [`take_tab`], a later ask replaces it.
pub fn ask_tab(plugin: &str, tab: &str) {
    lock(&ASKED_TABS).insert(plugin.to_owned(), tab.to_owned());
}

/// The tab plugin `plugin`'s panel was asked to show, taken: the panel reads it once.
#[must_use]
pub fn take_tab(plugin: &str) -> Option<String> {
    lock(&ASKED_TABS).remove(plugin)
}

/// The `pluginPanel` event that tells plugin `plugin`'s open panel to read its asked tab. It names
/// no tab: the panel takes it with its state, so an asked tab is shown once.
#[must_use]
pub fn tab_event(plugin: &str) -> Value {
    super::panels::event_message(plugin, json!({ "message": { "tab": true } }))
}

/// Opens panel `panel` of plugin `plugin` - a person's link, so a dock folded to its rail opens -
/// and, with `tab`, asks it to show that tab ([`ask_tab`], [`tab_event`]).
pub fn open_at(core: &Core, plugin: &str, panel: &str, tab: Option<&str>) {
    if let Some(tab) = tab {
        ask_tab(plugin, tab);
    }
    super::commands::open_panels_unfolded(core, &json!({ "plugin": plugin, "panel": panel }));
    if tab.is_some() {
        core.send(&rb_ipc::CoreToShell::PluginPanel {
            message: tab_event(plugin),
        });
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
