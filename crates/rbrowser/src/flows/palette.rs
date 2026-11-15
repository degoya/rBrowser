//! The project's flows for the command palette (RB-060-11): the shell asks `listFlows` when
//! the palette opens and lists "Run flow: <name>", which runs `flows.run { name }`. Listing
//! is not running: the trust gate stays on `flows.run`.

use std::path::Path;

use rb_ipc::{CoreToShell, FlowList, FlowSummary, IpcErrorCode, ShellToCore};

use super::{FlowError, catalog};
use crate::control::Core;
use crate::ipc::Failure;

/// The flows in `dir` of the project at `root`, by name; none without a project.
///
/// # Errors
///
/// [`FlowError::BadDir`] for a folder outside `.rbrowser/flows`, [`FlowError::Io`].
pub fn flow_list(root: Option<&Path>, dir: &str) -> Result<FlowList, FlowError> {
    let Some(root) = root else {
        return Ok(FlowList::default());
    };
    let items = catalog::list(root, dir)?
        .into_iter()
        .map(|flow| FlowSummary {
            name: flow.name,
            description: flow.description,
        })
        .collect();
    Ok(FlowList { items })
}

/// Whether `message` is the one this module answers.
fn answers(message: &ShellToCore) -> bool {
    matches!(message, ShellToCore::ListFlows)
}

/// Answers the shell's `listFlows`; `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    if !answers(message) {
        return None;
    }
    let root = core.host().project_root();
    let dir = core.user_settings().automation.flows.dir;
    Some(
        flow_list(root.as_deref(), &dir)
            .map(CoreToShell::Flows)
            .map_err(|error| (IpcErrorCode::Internal, error.to_string())),
    )
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
