//! The naming rule of plugin tools (`docs/plugins.md`, "MCP"): core tools carry no prefix,
//! plugin tools are `<mcp_prefix>_<tool>`, one prefix per plugin, never another plugin's and
//! never an area of the core tools. The plugin host enforces the same when it admits a plugin
//! (`rb-plugin-host`: `manifest/checks.rs`, `registry.rs`); this holds the bundled plugins and
//! the core tools to it in the coverage test.

use std::collections::BTreeMap;

/// What a plugin's manifest says about MCP: its prefix and its `[[tools]]`, by their bare names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginTools {
    /// The plugin id.
    pub plugin: String,
    /// `mcp_prefix`.
    pub prefix: Option<String>,
    /// The `name` of every `[[tools]]` entry, without the prefix.
    pub tools: Vec<String>,
}

impl PluginTools {
    /// The MCP names of its tools, `<mcp_prefix>_<tool>`; none without a prefix.
    #[must_use]
    pub fn mcp_names(&self) -> Vec<String> {
        self.prefix.as_ref().map_or_else(Vec::new, |prefix| {
            self.tools
                .iter()
                .map(|tool| format!("{prefix}_{tool}"))
                .collect()
        })
    }
}

/// Whether `prefix` is an MCP prefix: a lower-case letter, then letters and digits, 2 to 24
/// characters - no `_`, so `<prefix>_<tool>` splits one way (as `rb-plugin-host` holds it).
#[must_use]
pub fn valid_prefix(prefix: &str) -> bool {
    (2..=24).contains(&prefix.len())
        && prefix.starts_with(|c: char| c.is_ascii_lowercase())
        && prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// A tool's bare name: `[a-z][a-z0-9_]*`, at most 40 characters.
fn valid_tool(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Every break of the naming rule among `plugins` and against the core tools (`core`), each as
/// `<plugin>: <why>`: a tool without a prefix, a malformed prefix or tool name, a prefix two
/// plugins hold, a tool twice, a prefix that is an area of core tools the plugin does not
/// declare itself (a bundled plugin's own tool the core serves, such as `breakpoints_detect`,
/// is its own).
#[must_use]
pub fn plugin_problems(plugins: &[PluginTools], core: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut holders: BTreeMap<&str, &str> = BTreeMap::new();
    for plugin in plugins {
        let id = plugin.plugin.as_str();
        for tool in &plugin.tools {
            if !valid_tool(tool) {
                problems.push(format!("{id}: tool `{tool}` is not [a-z][a-z0-9_]*"));
            }
            if plugin.tools.iter().filter(|other| *other == tool).count() > 1 {
                problems.push(format!("{id}: tool `{tool}` is declared twice"));
            }
        }
        let Some(prefix) = plugin.prefix.as_deref() else {
            problems.extend(plugin.tools.iter().map(|tool| {
                format!(
                    "{id}: tool `{tool}` has no mcp_prefix; plugin tools are <mcp_prefix>_<tool>"
                )
            }));
            continue;
        };
        if !valid_prefix(prefix) {
            problems.push(format!(
                "{id}: mcp_prefix `{prefix}` is not a lower-case letter, then letters and digits \
                 (2 to 24)"
            ));
        }
        if let Some(other) = holders.insert(prefix, id) {
            problems.push(format!("{id}: mcp_prefix `{prefix}` is taken by {other}"));
        }
        let own = plugin.mcp_names();
        let foreign: Vec<&str> = core
            .iter()
            .filter(|name| {
                name.strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('_'))
                    && !own.contains(name)
            })
            .map(String::as_str)
            .collect();
        if !foreign.is_empty() {
            problems.push(format!(
                "{id}: mcp_prefix `{prefix}` is an area of the core tools ({})",
                foreign.join(", ")
            ));
        }
    }
    problems.dedup();
    problems
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
