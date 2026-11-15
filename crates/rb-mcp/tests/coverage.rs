//! The MCP coverage over the whole product (RB-030-08): every command of the registry - as
//! `schema/commands.v1.json` holds it, which a test of crates/rbrowser keeps current - and every
//! bundled plugin's `[[commands]]` and `[[tools]]` (`plugins/*/manifest.toml`) against the tools
//! this server registers. `crates/rbrowser/src/mcp/mod_tests.rs` runs the same per-command rule
//! over the live registry.

use std::fs;
use std::path::{Path, PathBuf};

use rb_commands::McpCoverage;
use rb_mcp::RbMcpServer;
use rb_mcp::coverage::plugins::{PluginTools, plugin_problems};
use rb_mcp::coverage::tools::tool_problems;
use rb_mcp::coverage::{NO_PREFIX, PENDING, UNDECLARED, orphans, pending_job_problems, problems};
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every command of `schema/commands.v1.json`: id and MCP entry.
fn core_commands() -> Vec<(String, McpCoverage)> {
    let text = fs::read_to_string(root().join("schema/commands.v1.json")).expect("commands file");
    let file: Value = serde_json::from_str(&text).expect("commands JSON");
    file["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .map(|command| {
            let id = command["id"].as_str().expect("id").to_owned();
            let mcp = serde_json::from_value(command["mcp"].clone()).expect("an MCP entry");
            (id, mcp)
        })
        .collect()
}

/// A bundled plugin: its MCP names and its commands as the plugin host registers them
/// (`rb-plugin-host`, `commands.rs`).
fn bundled_plugins() -> Vec<(PluginTools, Vec<(String, McpCoverage)>)> {
    let mut manifests: Vec<PathBuf> = fs::read_dir(root().join("plugins"))
        .expect("plugins")
        .flatten()
        .map(|entry| entry.path().join("manifest.toml"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();
    manifests
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).expect("manifest");
            let manifest: toml::Table = toml::from_str(&text).expect("manifest TOML");
            let id = manifest["id"].as_str().expect("id").to_owned();
            let prefix = manifest
                .get("mcp_prefix")
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
            let entries = |key: &str| {
                manifest
                    .get(key)
                    .and_then(toml::Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            };
            let tools = entries("tools")
                .iter()
                .map(|tool| tool["name"].as_str().expect("tool name").to_owned())
                .collect();
            let commands = entries("commands")
                .iter()
                .map(|command| {
                    let name = command["id"].as_str().expect("command id");
                    let mcp = match (
                        command.get("tool").and_then(toml::Value::as_str),
                        command.get("mcp_omitted").and_then(toml::Value::as_str),
                    ) {
                        (Some(tool), _) => prefix.as_ref().map_or_else(
                            || McpCoverage::Omitted(NO_PREFIX.to_owned()),
                            |prefix| McpCoverage::Covered(format!("{prefix}_{tool}")),
                        ),
                        (None, Some(reason)) => McpCoverage::Omitted(reason.to_owned()),
                        (None, None) => McpCoverage::Omitted(UNDECLARED.to_owned()),
                    };
                    (format!("plugin.{id}.{name}"), mcp)
                })
                .collect();
            let plugin = PluginTools {
                plugin: id,
                prefix,
                tools,
            };
            (plugin, commands)
        })
        .collect()
}

#[test]
fn every_command_core_and_plugin_resolves_to_a_tool_or_a_reason() {
    let core = core_commands();
    assert!(core.len() > 40, "{}", core.len());
    let plugins = bundled_plugins();
    assert!(!plugins.is_empty(), "the bundled plugins");
    let mut known = RbMcpServer::tool_names();
    known.extend(plugins.iter().flat_map(|(plugin, _)| plugin.mcp_names()));
    let commands: Vec<(String, McpCoverage)> = core
        .into_iter()
        .chain(plugins.into_iter().flat_map(|(_, commands)| commands))
        .collect();
    let entries: Vec<(&str, &McpCoverage)> = commands
        .iter()
        .map(|(id, mcp)| (id.as_str(), mcp))
        .collect();
    assert_eq!(problems(&entries, &known), Vec::<String>::new());
    assert_eq!(
        orphans(&entries, &RbMcpServer::tool_names()),
        Vec::<String>::new()
    );
}

#[test]
fn every_tool_is_unique_described_typed_and_targeted() {
    assert_eq!(tool_problems(), Vec::<String>::new());
}

#[test]
fn the_bundled_plugins_follow_the_naming_rule() {
    let plugins: Vec<PluginTools> = bundled_plugins()
        .into_iter()
        .map(|(plugin, _)| plugin)
        .collect();
    assert_eq!(
        plugin_problems(&plugins, &RbMcpServer::tool_names()),
        Vec::<String>::new()
    );
}

#[test]
fn every_pending_tool_waits_on_an_open_job() {
    // The job files are planning and stay in the private repository: the public export
    // (scripts/public-exclude.txt, RB-090-12) has no docs/, so there is nothing to hold here.
    if !root().join("docs/roadmap/jobs").is_dir() {
        eprintln!("skipped: no docs/roadmap/jobs in this tree (the public export)");
        return;
    }
    assert_eq!(pending_job_problems(&root(), PENDING), Vec::<String>::new());
}
