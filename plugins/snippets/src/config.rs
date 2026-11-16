//! The plugin's settings (`settings.schema.json`, Settings > DevTools & Debugging > Snippets).
//!
//! `snippets.library` is the person's own snippets - a user-scope key (never the project's), so
//! no repository can plant one, and protected from MCP. Each entry is `{ "name", "code" }`; the
//! code may carry a header like a file.

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::header::{MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, World};

/// `snippets.defaultTimeoutMs` without a setting.
pub const DEFAULT_TIMEOUT_MS: u32 = 5_000;

/// The settings in effect.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub default_world: World,
    pub default_timeout_ms: u32,
    /// `snippets.projectEnabled`: the project's `.rbrowser/snippets/` is read (under trust).
    pub project_enabled: bool,
    /// `snippets.library`, as it reads.
    pub library: Vec<Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_world: World::Isolated,
            default_timeout_ms: DEFAULT_TIMEOUT_MS,
            project_enabled: true,
            library: Vec::new(),
        }
    }
}

fn value(host: &mut dyn PluginHost, key: &str) -> Option<Value> {
    host.setting(key)
        .and_then(|text| serde_json::from_str(&text).ok())
}

impl Settings {
    /// The settings the host answers; the default for each key it has not or that does not read.
    #[must_use]
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let mut settings = Self::default();
        if let Some(world) = value(host, "defaultWorld")
            .as_ref()
            .and_then(Value::as_str)
            .and_then(World::parse)
        {
            settings.default_world = world;
        }
        if let Some(ms) = value(host, "defaultTimeoutMs")
            .and_then(|ms| ms.as_u64())
            .and_then(|ms| u32::try_from(ms).ok())
            .filter(|ms| (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(ms))
        {
            settings.default_timeout_ms = ms;
        }
        if let Some(Value::Bool(enabled)) = value(host, "projectEnabled") {
            settings.project_enabled = enabled;
        }
        if let Some(Value::Array(entries)) = value(host, "library") {
            settings.library = entries;
        }
        settings
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
