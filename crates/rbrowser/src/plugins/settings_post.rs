//! The page scripts told that their plugin's settings changed (RB-090-58): after a change of the
//! user settings or the project config, every plugin with page scripts whose own effective values
//! ([`super::services::AppServices::plugin_values`]) differ from the ones seen last gets the post
//! `{"$settings": true}` in every view (`rbPlugin.on`), so an overlay drawn from its settings -
//! the rulers' colours - follows at once, without a reload. The values themselves stay with the
//! component; the script asks it again.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::{Map, Value, json};

use super::runtime::PluginRuntime;
use super::services;
use crate::cdp::lock;
use crate::control::Core;

/// The post a page script gets: its plugin's settings changed.
#[must_use]
pub fn message() -> Value {
    json!({ "$settings": true })
}

/// The values each plugin had when they were looked at last.
#[derive(Debug, Default)]
pub struct Seen {
    values: HashMap<String, Map<String, Value>>,
}

impl Seen {
    /// The plugins of `now` whose values differ from the ones seen last - a plugin not seen
    /// before among them -, in the order of `now`; `now` is what is seen from here on.
    pub fn changed(&mut self, now: Vec<(String, Map<String, Value>)>) -> Vec<String> {
        let mut changed = Vec::new();
        let mut next = HashMap::with_capacity(now.len());
        for (plugin, values) in now {
            if self.values.get(&plugin) != Some(&values) {
                changed.push(plugin.clone());
            }
            next.insert(plugin, values);
        }
        self.values = next;
        changed
    }
}

fn seen() -> &'static Mutex<Seen> {
    static SEEN: LazyLock<Mutex<Seen>> = LazyLock::new(Mutex::default);
    &SEEN
}

/// Tells the page scripts of every plugin whose settings changed, off the calling thread (the
/// values are resolved there, never on the UI thread).
pub fn post_changes(core: &Core, runtime: Arc<PluginRuntime>) {
    if runtime
        .plugins()
        .iter()
        .all(|plugin| plugin.bridge.is_none())
    {
        return;
    }
    core.runtime().spawn(async move {
        let now: Vec<(String, Map<String, Value>)> = runtime
            .plugins()
            .iter()
            .filter(|plugin| plugin.bridge.is_some())
            .map(|plugin| {
                let values = services::shared().plugin_values(&plugin.id);
                (plugin.id.clone(), values)
            })
            .collect();
        let changed = lock(seen()).changed(now);
        let message = message();
        for plugin in changed {
            runtime.pages().post(&plugin, None, &message).await;
        }
    });
}

#[cfg(test)]
#[path = "settings_post_tests.rs"]
mod tests;
