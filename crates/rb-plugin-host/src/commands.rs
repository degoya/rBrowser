//! The commands of the plugin system: the core's `plugins.*` (group `X`) and the ones a plugin's
//! manifest declares, as `rb_commands::CommandDef`s for the one command registry (RB-010-17).
//!
//! **Protected from MCP** (`docs/plugins.md`, "Settings"): install, enable, disable, remove,
//! trusting a key and resuming are `omitted("protected: plugin install/trust")` - an agent can
//! neither install a plugin nor trust one. Only the reads have MCP tools: `plugins.list` its
//! `plugins_list` (the app builds its answer, `rbrowser::plugins::list`; rb-mcp serves it), and
//! the catalogue's `plugins.catalog` and `plugins.check_updates` (RB-091-01) `plugins_catalog_list`,
//! which lists what the configured repositories offer and installs nothing - installing from the
//! catalogue and adding a repository are a person's, in Settings > Plugins > Catalog.

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, Group, Key, McpCoverage, Platform,
    reserved_list,
};
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

use crate::manifest::PluginManifest;

/// The reason of every protected `plugins.*` command in the MCP coverage table.
pub const PROTECTED_REASON: &str = "protected: plugin install/trust";

/// The MCP tool of `plugins.list`.
pub const PLUGINS_LIST_TOOL: &str = "plugins_list";

/// The MCP tool of the catalogue's reads, `plugins.catalog` and `plugins.check_updates`
/// (RB-091-01).
pub const PLUGINS_CATALOG_TOOL: &str = "plugins_catalog_list";

/// `plugins.panels`: opens the plugin panels window (RB-060-12), at a panel when the arguments
/// name one (`{"plugin", "panel"}`).
pub const PANELS_COMMAND: &str = "plugins.panels";
/// Its MCP coverage reason: a window for a person.
pub const PANELS_REASON: &str = "ui-only: opens the plugin panels window";

/// The core commands: id, the key after `Ctrl+Shift+Space X`, and the MCP tool covering it -
/// `None` for the protected ones. `X C` and `X P` were taken (`extensions.check_updates`,
/// `plugins.panels`), so the catalogue is `X B` (browse) and its update check `X Q` (query).
pub const CORE_COMMANDS: &[(&str, char, Option<&str>)] = &[
    ("plugins.list", 'L', Some(PLUGINS_LIST_TOOL)),
    ("plugins.install", 'I', None),
    ("plugins.enable", 'E', None),
    ("plugins.disable", 'D', None),
    ("plugins.remove", 'R', None),
    ("plugins.trust_key", 'K', None),
    ("plugins.resume", 'U', None),
    ("plugins.catalog", 'B', Some(PLUGINS_CATALOG_TOOL)),
    ("plugins.check_updates", 'Q', Some(PLUGINS_CATALOG_TOOL)),
];

/// The core `plugins.*` commands, each with its leader chord `X <key>`, and `plugins.panels`
/// (`X P`).
///
/// # Errors
///
/// An id breaks the command id format, which a test holds.
pub fn core_command_defs() -> Result<Vec<CommandDef>, CommandError> {
    let core = CORE_COMMANDS.iter().map(|(id, key, tool)| {
        let mcp = match tool {
            Some(tool) => McpCoverage::Covered((*tool).to_owned()),
            None => McpCoverage::Omitted(PROTECTED_REASON.to_owned()),
        };
        (*id, *key, mcp)
    });
    let panels = (
        PANELS_COMMAND,
        'P',
        McpCoverage::Omitted(PANELS_REASON.to_owned()),
    );
    core.chain(std::iter::once(panels))
        .map(|(id, key, mcp)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Plugins,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Plugins.leader_key()), Key::Char(key)],
                })],
                mcp,
            })
        })
        .collect()
}

/// The commands `manifest` declares, `plugin.<id>.<command>`, and the notices for default
/// shortcuts that will stay unbound: one on the reservation list, one that does not parse. Such a
/// command keeps its place in the palette and under the leader group (`docs/plugins.md`,
/// "Shortcuts"). A reserved default is still handed to the registry, which drops it with the
/// notice the person sees (status line, shortcut editor: "reachable through the leader"); the
/// conflict check against other commands' bindings is the registry's too.
#[must_use]
pub fn plugin_command_defs(manifest: &PluginManifest) -> (Vec<CommandDef>, Vec<Failure>) {
    let reserved = reserved_list().ok();
    let mut defs = Vec::new();
    let mut notices = Vec::new();
    for command in &manifest.commands {
        let id = manifest.command_id(&command.id);
        let Ok(command_id) = CommandId::new(&id) else {
            notices.push(unbound(&id, "", "invalid_id"));
            continue;
        };
        let group = command
            .group
            .as_ref()
            .and_then(|group| serde_json::from_value::<Group>(Value::String(group.clone())).ok())
            .unwrap_or(Group::Plugins);
        let mut default_bindings = Vec::new();
        if let Some(text) = &command.shortcut {
            match text.parse::<Binding>() {
                Ok(Binding::Direct(shortcut))
                    if reserved
                        .as_ref()
                        .and_then(|list| list.find(&shortcut, Platform::current()))
                        .is_some() =>
                {
                    notices.push(unbound(&id, text, "reserved"));
                    default_bindings.push(Binding::Direct(shortcut));
                }
                Ok(binding) => default_bindings.push(binding),
                Err(_) => notices.push(unbound(&id, text, "unparsed")),
            }
        }
        let mcp = match (&command.tool, &command.mcp_omitted) {
            (Some(tool), _) => manifest.tool_name(tool).map_or_else(
                || McpCoverage::Omitted("no mcp_prefix".to_owned()),
                McpCoverage::Covered,
            ),
            (None, Some(reason)) => McpCoverage::Omitted(reason.clone()),
            (None, None) => McpCoverage::Omitted("undeclared".to_owned()),
        };
        defs.push(CommandDef {
            id: command_id,
            title_key: command.title.clone(),
            group,
            default_bindings,
            mcp,
        });
    }
    (defs, notices)
}

fn unbound(command: &str, binding: &str, why: &str) -> Failure {
    Failure::coded(FailureKind::Other, codes::SHORTCUT_UNBOUND)
        .with_param("command", command)
        .with_param("binding", binding)
        .with_param("why", why)
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
