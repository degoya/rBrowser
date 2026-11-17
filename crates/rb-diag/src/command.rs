//! The command `diagnostics.create`: its definition for the command registry (leader `, D`,
//! MCP tool `diagnostics`), the CLI arguments of `rbrowser diagnostics [--out <zip>]` and the
//! handler that writes the ZIP into the diagnostics folder. The shell's "Save diagnostics..."
//! asks where with the Save dialog instead (`rbrowser::control::diag_save`, RB-080-73).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use rb_commands::{Binding, Chord, CommandDef, CommandError, CommandId, Group, Handler, Key};
use serde_json::{Value, json};

use crate::bundle::{BundleInput, BundleReport, free_bundle_path, write_bundle};
use crate::error::DiagError;

/// The command id.
pub const COMMAND_ID: &str = "diagnostics.create";
/// The MCP tool that covers it (registered by RB-030-01).
pub const MCP_TOOL: &str = "diagnostics";

/// The command's definition: group Settings, default binding leader `, D`.
pub fn command_def() -> Result<CommandDef, CommandError> {
    Ok(CommandDef {
        id: CommandId::new(COMMAND_ID)?,
        title_key: format!("commands.{COMMAND_ID}"),
        group: Group::Settings,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Settings.leader_key()), Key::Char('D')],
        })],
        mcp: rb_commands::McpCoverage::Covered(MCP_TOOL.to_owned()),
    })
}

/// `rbrowser diagnostics [--out <zip>]`: add as `Diagnostics(rb_diag::DiagnosticsArgs)` to the
/// binary's subcommands.
#[derive(Clone, Debug, Default, PartialEq, Eq, clap::Args)]
pub struct DiagnosticsArgs {
    /// Where to write the ZIP; default `%LOCALAPPDATA%\rBrowser\diagnostics\
    /// rbrowser-diagnostics-<time>.zip`. An existing file is never overwritten.
    #[arg(long, value_name = "ZIP")]
    pub out: Option<PathBuf>,
}

/// Runs the CLI command: writes the ZIP and returns what was written (the binary prints the
/// path). `--out` may name any path, but never an existing file.
pub fn run_cli(args: &DiagnosticsArgs, input: &BundleInput) -> Result<BundleReport, DiagError> {
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| free_bundle_path(&input.paths, SystemTime::now()));
    write_bundle(input, &out)
}

/// The handler of `diagnostics.create` with named arguments (MCP). Arguments
/// `{ "out"?: string }`; `out` is a file name ending in `.zip` inside the diagnostics folder (a
/// command writes nowhere else, and never over an existing file). Answers
/// `{ "path": string, "entries": [string] }`. `input` gathers the current versions, config and
/// plugins each time. Writing a ZIP takes a moment: dispatch it off the UI thread.
pub fn handler(input: Arc<dyn Fn() -> BundleInput + Send + Sync>) -> Handler {
    Arc::new(move |args: Value| {
        let input = input();
        let out = match args.get("out") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) if is_bundle_name(name) => {
                Some(input.paths.bundles.join(name))
            }
            Some(_) => {
                return Err(CommandError::InvalidArgs(
                    "`out` must be a file name ending in .zip; the ZIP goes to the diagnostics \
                     folder"
                        .to_owned(),
                ));
            }
        };
        let report = run_cli(&DiagnosticsArgs { out }, &input)
            .map_err(|error| CommandError::Failed(error.i18n_key().to_owned()))?;
        Ok(json!({ "path": report.path, "entries": report.entries }))
    })
}

/// Whether `name` is a bare file name ending in `.zip`: no folder, no drive, no `..`.
fn is_bundle_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !name.contains(['/', '\\', ':'])
        && name.len() > ".zip".len()
        && name.to_ascii_lowercase().ends_with(".zip")
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
