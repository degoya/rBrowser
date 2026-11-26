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
//! | Deleting a flow, its trust kept (`flows.delete`, RB-090-91) | [`delete`] |
//! | `rbrowser flow list` and `rbrowser flow run` | [`cli`] |
//! | The flows in the command palette (`listFlows`, RB-060-11) | [`palette`] |
//! | The Flows panel's run state (`getFlowRun`, `panels.flows`, RB-090-60) | [`panel`] |
//! | What a run leaves in the log: its end, each failed view's step and reason (RB-090-84) | [`report`] |
//! | Each step of a run going on pushed to the shell (`flowStep`, RB-090-84) | [`step_progress`] |
//! | The step each view is at, "step n/m" in the activity bar (RB-090-68) | [`run_steps`] |
//! | The layout a run shows when the one shown hides views (`hiddenViews: layout`) | [`run_layout`] |
//! | The views of a run's `views` or `scope` (all, visible, chosen breakpoints; RB-099-10) | [`scoped`] |
//! | A played recording's view brought into sight when every one is hidden (RB-090-89) | [`reveal`] |
//! | The run in Node: a worker process per run under Node's permission model | `sidecar/src/modules/flows/` |
//!
//! The commands: `flows.run` (`Leader F R`), `flows.run_all` (`Leader F A`), `flows.rerun_last`
//! (`Leader F L`) - all `covered(flow_run)` -, `flows.cancel` (`Leader F X`) and
//! `flows.open_results` (`Leader F O`) and `flows.delete` (`Leader F D`), which are a person's;
//! `flows.run_all_visible` (`Leader F G`, `covered(flow_run)`) runs every flow in the views the
//! grid shows (RB-099-10).
//! Results land in `.rbrowser/out/flows/<runId>/` (`result.json`, screenshots of failed views).

pub mod app;
pub mod catalog;
pub mod cli;
pub mod delete;
pub mod endpoints;
mod error;
pub mod exclusive;
pub mod gate;
pub mod palette;
pub mod panel;
pub mod plan;
pub mod queue;
pub mod report;
pub mod result;
pub mod reveal;
pub mod run;
pub mod run_layout;
pub mod run_steps;
pub mod scope;
pub mod scoped;
pub mod step_progress;
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
/// `flows.run_all_visible`: every flow in the views the grid shows (RB-099-10).
pub const RUN_ALL_VISIBLE: &str = "flows.run_all_visible";
/// `flows.rerun_last`: the last run again, with its views.
pub const RERUN_LAST: &str = "flows.rerun_last";
/// `flows.cancel`: ends the running flow.
pub const CANCEL: &str = "flows.cancel";
/// `flows.open_results`: shows the last run's `result.json` in the file manager.
pub const OPEN_RESULTS: &str = "flows.open_results";
/// `flows.delete`: deletes the flow `name`; without one the Flows panel opens (RB-090-91).
pub const DELETE: &str = "flows.delete";
/// `flows.panel`'s id is `panels.flows` (`Leader F F`): the Flows panel (RB-090-60).
pub const PANEL: &str = "panels.flows";
/// The MCP tool that lists the flows.
pub const MCP_LIST: &str = "flow_list";
/// The MCP tool that runs a flow.
pub const MCP_RUN: &str = "flow_run";
/// Why `flows.cancel` has no MCP tool (job file RB-040-07).
pub const CANCEL_OMITTED: &str =
    "cancelling a running flow is the person's; agents wait for the result";
/// Why `flows.open_results` has no MCP tool.
pub const OPEN_OMITTED: &str = "opens the file manager; flow_run and flow_list answer the results";
/// Why `flows.delete` has no MCP tool (RB-090-91).
pub const DELETE_OMITTED: &str =
    "protected: deleting project files is the person's; an agent never deletes a flow";

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
        (PANEL, 'F', McpCoverage::Covered(MCP_LIST.to_owned())),
        (DELETE, 'D', McpCoverage::Omitted(DELETE_OMITTED.to_owned())),
        // `G` for "the grid shows": every other letter after `F` is taken.
        (
            RUN_ALL_VISIBLE,
            'G',
            McpCoverage::Covered(MCP_RUN.to_owned()),
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
