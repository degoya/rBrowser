//! The plugins a [`PluginRuntime`] serves, swapped while it runs (RB-092-06), and the page
//! requests waiting per plugin (split from `runtime.rs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rb_plugin_host::{MAX_PENDING, PageBridge};

use super::{Plugin, PluginRuntime};
use crate::cdp::lock;

impl PluginRuntime {
    /// Serves `plugins` from now on: an install, update or removal took effect (RB-092-06). A
    /// plugin whose page scripts are the same keeps its bridge ([`keep_bridges`]), so its scripts
    /// in the views run on; a plugin no longer granted `network-rules` loses its rules. A call
    /// under way keeps the component it reached; its answer is refused when its plugin is gone.
    /// The page scripts follow with the next [`PluginRuntime::set_enabled`].
    pub fn replace_plugins(&self, plugins: Vec<Plugin>) {
        let old = self.plugins();
        let plugins = keep_bridges(&old, plugins);
        if let Some(rules) = self.network_rules() {
            for gone in old.iter().filter(|old| {
                old.network && !plugins.iter().any(|new| new.id == old.id && new.network)
            }) {
                rules.remove(&gone.id);
            }
        }
        *lock(&self.plugins) = Arc::new(plugins);
    }

    /// The plugin `plugin` as served now.
    pub(super) fn served(&self, plugin: &str) -> Option<Plugin> {
        self.plugins()
            .iter()
            .find(|served| served.id == plugin)
            .cloned()
    }

    /// Whether the window's interception rules run ([`PluginRuntime::set_network_rules`]).
    pub fn has_network_rules(&self) -> bool {
        lock(&self.network).is_some()
    }

    pub(super) fn take_slot(&self, plugin: &str) -> Option<PendingSlot<'_>> {
        let mut pending = lock(&self.pending);
        let count = pending.entry(plugin.to_owned()).or_default();
        if *count >= MAX_PENDING {
            return None;
        }
        *count += 1;
        Some(PendingSlot {
            pending: &self.pending,
            plugin: plugin.to_owned(),
        })
    }
}

/// `new` with the bridge of each plugin `old` serves with the same page scripts: a bridge's
/// binding and receiver are random per load, and the scripts in the views call the old ones.
/// A plugin whose scripts changed brings its new bridge; its old scripts are stopped and the new
/// ones injected.
pub fn keep_bridges(old: &[Plugin], new: Vec<Plugin>) -> Vec<Plugin> {
    new.into_iter()
        .map(|mut plugin| {
            let kept = old
                .iter()
                .find(|old| old.id == plugin.id)
                .and_then(|old| old.bridge.clone())
                .filter(|kept| {
                    plugin
                        .bridge
                        .as_deref()
                        .is_some_and(|fresh| same_scripts(fresh, kept))
                });
            if let Some(kept) = kept {
                plugin.bridge = Some(kept);
            }
            plugin
        })
        .collect()
}

/// Whether two bridges run the same scripts in the same world, whatever their names.
fn same_scripts(a: &PageBridge, b: &PageBridge) -> bool {
    a.plugin_id == b.plugin_id && a.world == b.world && a.scripts == b.scripts
}

/// Counts a page request of a plugin while it waits.
pub(super) struct PendingSlot<'a> {
    pending: &'a Mutex<HashMap<String, usize>>,
    plugin: String,
}

impl Drop for PendingSlot<'_> {
    fn drop(&mut self) {
        if let Some(count) = lock(self.pending).get_mut(&self.plugin) {
            *count = count.saturating_sub(1);
        }
    }
}

#[cfg(test)]
#[path = "runtime_swap_tests.rs"]
mod tests;
