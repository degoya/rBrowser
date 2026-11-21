//! The sinks of a plugin load (split from `plugins.rs`, RB-091-08): the plugins' locales go to
//! `rb_i18n`, their settings schemas are checked against the settings registry and kept.

use std::collections::BTreeMap;
use std::sync::Mutex;

use rb_plugin_host::{LocaleSink, SettingsSink};
use rb_settings::SettingsRegistry;
use serde_json::Value;

use crate::cdp::lock;

/// Locales go to the one catalogue of `rb_i18n`.
pub struct I18nLocales;

impl LocaleSink for I18nLocales {
    fn register(&self, plugin_id: &str, language: &str, bytes: &[u8]) -> Result<(), String> {
        rb_i18n::register_plugin_locale(plugin_id, language, bytes)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn unregister(&self, plugin_id: &str) {
        rb_i18n::unregister_plugin_locales(plugin_id);
    }
}

/// A plugin's settings schema, checked against a registry holding the core keys (an unknown
/// category, a key that is taken or one without its annotations is refused) and kept for the
/// settings window.
pub struct SchemaSink {
    registry: Mutex<SettingsRegistry>,
    taken: Mutex<Vec<(String, Value)>>,
    counts: Mutex<BTreeMap<String, usize>>,
}

impl SchemaSink {
    /// A sink over the core schemas.
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(crate::control::settings::settings_registry().0),
            taken: Mutex::new(Vec::new()),
            counts: Mutex::new(BTreeMap::new()),
        }
    }

    /// How many keys each plugin registered, by plugin id.
    pub fn counts(&self) -> BTreeMap<String, usize> {
        lock(&self.counts).clone()
    }

    /// The schemas taken, by plugin id.
    pub fn into_schemas(self) -> Vec<(String, Value)> {
        self.taken
            .into_inner()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

impl Default for SchemaSink {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsSink for SchemaSink {
    fn register_plugin_schema(&self, plugin_id: &str, schema: &Value) -> Result<usize, String> {
        let count = lock(&self.registry)
            .register_plugin_schema(plugin_id, schema)
            .map_err(|issues| {
                issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.key, issue.error))
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
        lock(&self.taken).push((plugin_id.to_owned(), schema.clone()));
        lock(&self.counts).insert(plugin_id.to_owned(), count);
        Ok(count)
    }
}
