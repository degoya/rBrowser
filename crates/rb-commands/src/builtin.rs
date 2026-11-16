//! The core command definitions this crate knows: the commands the shell dispatches (RB-010-07)
//! and the palette and shortcut editor of RB-010-17. Their handlers live with the host
//! (`crates/rbrowser`), which registers each definition with
//! [`crate::CommandRegistry::register`]. Further core commands are defined by the jobs that
//! own them; every definition here passes the lint ([`crate::lint`]). The feature commands of
//! wave 0.1-w1b are [`crate::builtin_features`], chained into [`definitions`].

use crate::chord::PALETTE_COMMAND;
use crate::{Binding, CommandDef, CommandId, Group, McpCoverage};

/// The editor of Settings > Keyboard Shortcuts.
pub const SHORTCUTS_EDIT_COMMAND: &str = "shortcuts.edit";

/// One row: id, group, default bindings, MCP tool (`Ok`) or the reason it has none (`Err`).
type Row = (
    &'static str,
    Group,
    &'static [&'static str],
    Result<&'static str, &'static str>,
);

const ROWS: &[Row] = &[
    (
        "view.back",
        Group::View,
        // The arrows of RB-020-06 beside the mnemonic (`Alt+Left` stays the browser's).
        &["Ctrl+Shift+Space V B", "Ctrl+Shift+Space V Left"],
        Ok("back"),
    ),
    (
        "view.forward",
        Group::View,
        &["Ctrl+Shift+Space V F", "Ctrl+Shift+Space V Right"],
        Ok("forward"),
    ),
    (
        "view.navigate",
        Group::View,
        &["Ctrl+Shift+Space V N"],
        Ok("navigate"),
    ),
    (
        "view.focus_address_bar",
        Group::View,
        &["Ctrl+Shift+Space V A"],
        Err("ui-only"),
    ),
    (
        "reload.all",
        Group::Reload,
        &["Ctrl+Shift+Space R R"],
        Ok("reload"),
    ),
    (
        "sync.toggle",
        Group::Sync,
        &["Ctrl+Shift+Space S S"],
        Ok("sync_set"),
    ),
    (
        "layout.select_preset",
        Group::Layout,
        &["Ctrl+Shift+Space L L"],
        Ok("layout_set"),
    ),
    (
        "environments.select",
        Group::Environments,
        &["Ctrl+Shift+Space E E"],
        Ok("environment_set"),
    ),
    (
        "breakpoints.analyze",
        Group::Breakpoints,
        &["Ctrl+Shift+Space B A"],
        // The wizard of RB-010-13; MCP reaches the analysis as the plugin's `breakpoints_detect`.
        Ok("breakpoints_detect"),
    ),
    (
        "settings.open",
        Group::Settings,
        &["Ctrl+Shift+Space , ,"],
        Err("ui-only"),
    ),
    (
        SHORTCUTS_EDIT_COMMAND,
        Group::Settings,
        &["Ctrl+Shift+Space , K"],
        // The bindings are readable as the setting `shortcuts.bindings` (RB-060-11).
        Ok("settings_get"),
    ),
    (
        PALETTE_COMMAND,
        Group::View,
        &["Ctrl+Shift+Space V P"],
        Err("ui-only: the palette is a shell surface; commands_list lists the commands"),
    ),
];

/// The definitions, in table order, then the feature commands of wave 0.1-w1b
/// ([`crate::builtin_features`]). The title key is `commands.<id>`.
pub fn definitions() -> Vec<CommandDef> {
    let mut defs = table_definitions();
    defs.extend(crate::builtin_features::definitions());
    defs
}

/// The definitions of [`ROWS`], in table order.
fn table_definitions() -> Vec<CommandDef> {
    ROWS.iter()
        .map(|(id, group, bindings, mcp)| CommandDef {
            id: CommandId::new(id).expect("the table's ids are checked by builtin_tests"),
            title_key: format!("commands.{id}"),
            group: *group,
            default_bindings: bindings
                .iter()
                .map(|text| {
                    text.parse::<Binding>()
                        .expect("the table's bindings are checked by builtin_tests")
                })
                .collect(),
            mcp: match mcp {
                Ok(tool) => McpCoverage::Covered((*tool).to_owned()),
                Err(reason) => McpCoverage::Omitted((*reason).to_owned()),
            },
        })
        .collect()
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod tests;
