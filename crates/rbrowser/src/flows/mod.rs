//! Flows (RB-040-07): short Playwright scripts of the project, `.rbrowser/flows/<name>.ts`, run
//! in every view at once - to put all breakpoints into one state (logged in, cart filled) or to
//! check one. `page`, `view`, `expect`, `env` and `step` are injected; no import is needed.
//!
//! | Part | Where |
//! | --- | --- |
//! | The flows of a project, their head comments, the folder setting | [`catalog`] |
//! | The trust gate (K9: `.rbrowser/flows` is protected) | [`gate`] |
//! | Which views run, which are skipped and why | [`plan`] |
//! | One run: gate, endpoints, sync paused, the sidecar, `result.json` | [`run`] |
//! | Sync paused for a run's views, given back on drop | [`sync_pause`] |
//! | One internal CDP proxy per view, showing only its page | [`endpoints`], [`scope`] |
//! | The app's state, the commands and the shell's notices | [`app`] |
//! | `rbrowser flow list` and `rbrowser flow run` | [`cli`] |
//! | The flows in the command palette (`listFlows`, RB-060-11) | [`palette`] |
//! | The run in Node: a worker process per run under Node's permission model | `sidecar/src/modules/flows/` |
//!
//! The commands: `flows.run` (`Leader F R`), `flows.run_all` (`Leader F A`), `flows.rerun_last`
//! (`Leader F L`) - all `covered(flow_run)` -, `flows.cancel` (`Leader F X`) and
//! `flows.open_results` (`Leader F O`), which are a person's. Results land in
//! `.rbrowser/out/flows/<runId>/` (`result.json`, screenshots of failed views).

pub mod app;
pub mod catalog;
pub mod cli;
pub mod endpoints;
mod error;
pub mod gate;
pub mod palette;
pub mod plan;
pub mod queue;
pub mod result;
pub mod run;
pub mod scope;
pub mod sync_pause;

use std::sync::Arc;

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Key, McpCoverage,
};

use crate::control::Core;
pub use error::{FlowError, TRUST_HINT};

/// `flows.run`: one flow (`name`), in the views asked for or the flow's own.
pub const RUN: &str = "flows.run";
/// `flows.run_all`: every flow of the project, one after the other.
pub const RUN_ALL: &str = "flows.run_all";
/// `flows.rerun_last`: the last run again, with its views.
pub const RERUN_LAST: &str = "flows.rerun_last";
/// `flows.cancel`: ends the running flow.
pub const CANCEL: &str = "flows.cancel";
/// `flows.open_results`: shows the last run's `result.json` in the file manager.
pub const OPEN_RESULTS: &str = "flows.open_results";
/// The MCP tool that lists the flows.
pub const MCP_LIST: &str = "flow_list";
/// The MCP tool that runs a flow.
pub const MCP_RUN: &str = "flow_run";
/// Why `flows.cancel` has no MCP tool (job file RB-040-07).
pub const CANCEL_OMITTED: &str =
    "cancelling a running flow is the person's; agents wait for the result";
/// Why `flows.open_results` has no MCP tool.
pub const OPEN_OMITTED: &str = "opens the file manager; flow_run and flow_list answer the results";

/// The command definitions: id, the key after `Leader F`, MCP coverage.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    [
        (RUN, 'R', McpCoverage::Covered(MCP_RUN.to_owned())),
        (RUN_ALL, 'A', McpCoverage::Covered(MCP_RUN.to_owned())),
        (RERUN_LAST, 'L', McpCoverage::Covered(MCP_RUN.to_owned())),
        (CANCEL, 'X', McpCoverage::Omitted(CANCEL_OMITTED.to_owned())),
        (
            OPEN_RESULTS,
            'O',
            McpCoverage::Omitted(OPEN_OMITTED.to_owned()),
        ),
    ]
    .into_iter()
    .map(|(id, key, mcp)| {
        Ok(CommandDef {
            id: CommandId::new(id)?,
            title_key: format!("commands.{id}"),
            group: Group::Flows,
            default_bindings: vec![Binding::Leader(Chord {
                keys: vec![Key::Char(Group::Flows.leader_key()), Key::Char(key)],
            })],
            mcp,
        })
    })
    .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = app::handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "e2e_tests.rs"]
mod e2e_tests;
