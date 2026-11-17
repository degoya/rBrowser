//! The hook points of the soft dependencies: workspace trust (RB-010-09), plugin locales
//! (RB-010-14) and plugin settings (RB-010-02). Each is a trait the application implements over
//! the owning crate's API once it lands; the defaults here are the safe ones - no trust, nothing
//! registered.

use std::path::Path;

use serde_json::Value;

/// Whether the person trusts a project, so its `.rbrowser/plugins/` may load (RB-010-09,
/// `rb_config::trust_state`). A trust decision is a human's: no MCP caller reaches it.
pub trait WorkspaceTrust: Send + Sync {
    /// Whether the project at `project_root` is trusted.
    fn is_trusted(&self, project_root: &Path) -> bool;
}

/// Trusts no project: project plugins never load. The default until RB-010-09 is wired.
#[derive(Clone, Copy, Debug, Default)]
pub struct DenyAllTrust;

impl WorkspaceTrust for DenyAllTrust {
    fn is_trusted(&self, _project_root: &Path) -> bool {
        false
    }
}

/// Where a loaded plugin's locale files go: `rb_i18n::register_plugin_locale(id, lang, bytes)`
/// and `unregister_plugin_locales(id)` (`docs/i18n.md`, "Plugin locales").
pub trait LocaleSink: Send + Sync {
    /// Registers one locale file of plugin `plugin_id`.
    ///
    /// # Errors
    ///
    /// The catalogue refuses the file; the message is English, for the log.
    fn register(&self, plugin_id: &str, language: &str, bytes: &[u8]) -> Result<(), String>;

    /// Removes every locale of plugin `plugin_id` (it was disabled or removed).
    fn unregister(&self, plugin_id: &str);
}

/// Registers nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoLocales;

impl LocaleSink for NoLocales {
    fn register(&self, _plugin_id: &str, _language: &str, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn unregister(&self, _plugin_id: &str) {}
}

/// Where a loaded plugin's settings schema goes: RB-010-02's
/// `SettingsRegistry::register_plugin_schema(plugin_id, schema)` (keys `<plugin-id>.<property>`,
/// labels `plugin.<plugin-id>.settings.<property>.label`).
pub trait SettingsSink: Send + Sync {
    /// Registers the schema; answers how many keys it holds.
    ///
    /// # Errors
    ///
    /// The registry refuses the fragment; the message is English, for the log.
    fn register_plugin_schema(&self, plugin_id: &str, schema: &Value) -> Result<usize, String>;

    /// The effective values of plugin `plugin_id`'s own keys, JSON text by property name: what
    /// its `settings.get` answers. Never another plugin's keys, never rBrowser's.
    fn values(&self, plugin_id: &str) -> Vec<(String, String)> {
        let _ = plugin_id;
        Vec::new()
    }
}

/// Registers nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoSettings;

impl SettingsSink for NoSettings {
    fn register_plugin_schema(&self, _plugin_id: &str, _schema: &Value) -> Result<usize, String> {
        Ok(0)
    }
}
