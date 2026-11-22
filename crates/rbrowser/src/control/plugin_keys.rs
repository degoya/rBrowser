//! The settings registry with the keys of the plugins loaded now (RB-094-26): the core's keys
//! from the schemas and every key of the plugins' settings schemas, as the plugin load took them
//! (`crate::analyzer::plugins`, handed to `crate::plugins::services`). The settings window's
//! schema and values, its writes, its links (`crate::settings_window::target`) and MCP's settings
//! tools read this one registry, so a plugin key is shown, written and linked to like a core key.
//!
//! A load puts a new list of schemas in place - at the start, after an install, an update, a
//! removal or a plugin switched on or off (RB-092-06, RB-094-23) - and the registry is built
//! again the next time it is asked for; between loads every caller shares one.

use std::sync::{Arc, PoisonError, RwLock};

use rb_settings::SettingsRegistry;
use serde_json::Value;

/// The plugins' settings schemas by plugin id, one list per load.
pub type PluginSchemas = Arc<Vec<(String, Value)>>;

/// The registry over the core's keys and the schemas of one load, built once per load.
pub struct LiveRegistry {
    held: RwLock<Held>,
}

struct Held {
    schemas: PluginSchemas,
    registry: Arc<SettingsRegistry>,
}

impl LiveRegistry {
    /// The core's keys alone, before any plugin loaded.
    pub fn new(core: SettingsRegistry) -> Self {
        Self {
            held: RwLock::new(Held {
                schemas: Arc::default(),
                registry: Arc::new(core),
            }),
        }
    }

    /// The registry with the keys of `schemas`: the one held when it was built over the same
    /// list, else built now and held.
    pub fn current(&self, schemas: PluginSchemas) -> Arc<SettingsRegistry> {
        {
            let held = self.held.read().unwrap_or_else(PoisonError::into_inner);
            if same(&held.schemas, &schemas) {
                return Arc::clone(&held.registry);
            }
        }
        let registry = Arc::new(with_plugins(&schemas));
        let mut held = self.held.write().unwrap_or_else(PoisonError::into_inner);
        *held = Held {
            schemas,
            registry: Arc::clone(&registry),
        };
        registry
    }
}

/// The same load: the same list, or no plugin in either.
fn same(held: &PluginSchemas, now: &PluginSchemas) -> bool {
    Arc::ptr_eq(held, now) || (held.is_empty() && now.is_empty())
}

/// The core's keys and those of every schema in `schemas`, in the order the load took them (so
/// a key two plugins declare goes to the one the load accepted). A schema the load took is
/// registered again whole; should one not be, its refused keys are logged and left out.
pub fn with_plugins(schemas: &[(String, Value)]) -> SettingsRegistry {
    let (mut registry, _) = super::settings::settings_registry();
    for (plugin, schema) in schemas {
        if let Err(issues) = registry.register_plugin_schema(plugin, schema) {
            for issue in issues {
                tracing::warn!(
                    %plugin,
                    key = %issue.key,
                    error = %issue.error,
                    "a plugin setting was not registered"
                );
            }
        }
    }
    registry
}

#[cfg(test)]
#[path = "plugin_keys_tests.rs"]
mod tests;
