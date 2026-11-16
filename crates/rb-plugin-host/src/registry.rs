//! The plugins admitted in this process and what they contribute that must be unique: the id,
//! the MCP prefix and the MCP tools `<mcp_prefix>_<tool>` (prepared for rb-mcp, RB-030-01).
//!
//! New in rBrowser (`docs/plugins.md`, "MCP"): the prefix is declared in the manifest and must
//! be unique; **the host refuses a collision** - the second plugin is not admitted, and the
//! refusal names the prefix and the plugin holding it.

use std::collections::BTreeMap;

use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

use crate::manifest::PluginManifest;

/// One plugin tool as MCP lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginTool {
    /// `<mcp_prefix>_<tool>`, the MCP name.
    pub name: String,
    pub plugin_id: String,
    /// The name the plugin's `call-tool` gets, without the prefix.
    pub tool: String,
    /// An i18n key of the plugin's locales.
    pub description: String,
    /// The JSON schema of the arguments.
    pub input_schema: Value,
    /// It runs JavaScript its caller supplies (`evaluates_input`): gated by `mcp.allowEvaluate`.
    pub evaluates_input: bool,
}

/// The admitted plugins, their prefixes and tools.
#[derive(Debug, Default)]
pub struct Registry {
    plugins: BTreeMap<String, PluginManifest>,
    prefixes: BTreeMap<String, String>,
    tools: BTreeMap<String, PluginTool>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Admits `manifest`: its id and MCP prefix are taken by no admitted plugin.
    ///
    /// # Errors
    ///
    /// `plugins.error.duplicate_id` or `plugins.error.mcp_prefix_taken` (with `prefix` and
    /// `other`, the plugin holding it); nothing is admitted then.
    pub fn admit(&mut self, manifest: &PluginManifest) -> Result<(), Failure> {
        if self.plugins.contains_key(&manifest.id) {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::DUPLICATE_ID)
                    .with_param("id", &manifest.id),
            );
        }
        if let Some(prefix) = &manifest.mcp_prefix
            && let Some(other) = self.prefixes.get(prefix)
        {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::MCP_PREFIX_TAKEN)
                    .with_param("prefix", prefix)
                    .with_param("other", other)
                    .with_param("id", &manifest.id),
            );
        }
        if let Some(prefix) = &manifest.mcp_prefix {
            self.prefixes.insert(prefix.clone(), manifest.id.clone());
            for tool in &manifest.tools {
                let name = format!("{prefix}_{}", tool.name);
                let input_schema = tool
                    .input
                    .as_ref()
                    .and_then(|input| serde_json::to_value(input).ok())
                    .unwrap_or_else(|| serde_json::json!({ "type": "object" }));
                self.tools.insert(
                    name.clone(),
                    PluginTool {
                        name,
                        plugin_id: manifest.id.clone(),
                        tool: tool.name.clone(),
                        description: tool.description.clone(),
                        input_schema,
                        evaluates_input: tool.evaluates_input,
                    },
                );
            }
        }
        self.plugins.insert(manifest.id.clone(), manifest.clone());
        Ok(())
    }

    /// Removes plugin `id` with its prefix and tools; whether it was admitted.
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(manifest) = self.plugins.remove(id) else {
            return false;
        };
        if let Some(prefix) = &manifest.mcp_prefix {
            self.prefixes.remove(prefix);
        }
        self.tools.retain(|_, tool| tool.plugin_id != id);
        true
    }

    /// The manifest of admitted plugin `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&PluginManifest> {
        self.plugins.get(id)
    }

    /// Every admitted plugin, by id.
    pub fn plugins(&self) -> impl Iterator<Item = &PluginManifest> {
        self.plugins.values()
    }

    /// Every plugin tool, by MCP name.
    pub fn tools(&self) -> impl Iterator<Item = &PluginTool> {
        self.tools.values()
    }

    /// The plugin tool with MCP name `name`.
    #[must_use]
    pub fn tool(&self, name: &str) -> Option<&PluginTool> {
        self.tools.get(name)
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
