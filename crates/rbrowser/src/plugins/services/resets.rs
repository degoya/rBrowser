//! CDP state a plugin set on a view that outlives the plugin (Charge 10): a colour-vision
//! simulation (`Emulation.setEmulatedVisionDeficiency`, simulators) or scripts switched off
//! (`Emulation.setScriptExecutionDisabled`, overlays' js-off) stays on the view's page session
//! after the plugin stops. The services note every such call per plugin and view
//! ([`CdpResets::note`]); when the plugin is disabled or stops for want of workspace trust the
//! host sends the method's neutral value to the views it touched ([`CdpResets::take`],
//! `super::AppServices::reset_cdp`). Nothing else is tracked: the other methods of the allowlist
//! read, mark or act once.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, PoisonError};

use rb_engine::ViewId;
use serde_json::{Value, json};

/// The methods whose effect lasts.
pub const RESETS: &[&str] = &[
    "Emulation.setEmulatedVisionDeficiency",
    "Emulation.setScriptExecutionDisabled",
];

/// The parameters that undo `method`, one of [`RESETS`].
fn neutral(method: &str) -> Option<Value> {
    match method {
        "Emulation.setEmulatedVisionDeficiency" => Some(json!({ "type": "none" })),
        "Emulation.setScriptExecutionDisabled" => Some(json!({ "value": false })),
        _ => None,
    }
}

/// Per plugin, the views and reset-able methods it left set.
#[derive(Debug, Default)]
pub struct CdpResets {
    set: Mutex<BTreeMap<String, BTreeSet<(u32, &'static str)>>>,
}

impl CdpResets {
    /// Plugin `plugin` sent `method` with `params` to `view` (and it succeeded).
    pub fn note(&self, plugin: &str, view: u32, method: &str, params: &Value) {
        let Some((method, undo)) = RESETS
            .iter()
            .find(|known| **known == method)
            .and_then(|known| Some((*known, neutral(known)?)))
        else {
            return;
        };
        let mut set = self.set.lock().unwrap_or_else(PoisonError::into_inner);
        let views = set.entry(plugin.to_owned()).or_default();
        if *params == undo {
            views.remove(&(view, method));
        } else {
            views.insert((view, method));
        }
    }

    /// What undoes plugin `plugin`'s lasting calls, `(view, method, params)`; forgotten here.
    pub fn take(&self, plugin: &str) -> Vec<(ViewId, &'static str, Value)> {
        let mut set = self.set.lock().unwrap_or_else(PoisonError::into_inner);
        set.remove(plugin)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(view, method)| Some((ViewId(view), method, neutral(method)?)))
            .collect()
    }
}

#[cfg(test)]
#[path = "resets_tests.rs"]
mod tests;
