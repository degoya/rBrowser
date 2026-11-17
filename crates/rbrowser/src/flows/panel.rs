//! The Flows panel's run state (RB-090-60): `getFlowRun` answers the run going on - its views'
//! statuses as the runner reports them - and the last run's `result.json`; `flowRunChanged`
//! (sent when a run starts and ends) has the panel ask again, and while a run goes on it asks
//! every second. The flows come from `listFlows` ([`super::palette`]); running, cancelling and
//! showing the results are the commands `flows.*`, as everywhere else. The page menu's "Run
//! flow" lists the flows last read ([`menu_flows`]).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rb_ipc::{CoreToShell, FlowRunState, FlowRunning, FlowViewStatus, ShellToCore};

use super::{catalog, result};
use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;

/// Answers `getFlowRun`; `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    if !matches!(message, ShellToCore::GetFlowRun) {
        return None;
    }
    let project = core.host().project_root().is_some();
    let (running, last_dir) = run_state();
    Some(Ok(CoreToShell::FlowRun(state_of(
        project,
        running,
        last_dir.as_deref(),
    ))))
}

/// The run going on - its id, flow and the views' statuses so far - and the last run's folder.
fn run_state() -> (Option<FlowRunning>, Option<PathBuf>) {
    let state = lock(&super::app::STATE);
    let running = state.running.as_ref().map(|running| FlowRunning {
        run_id: running.run_id.clone(),
        flow: running.flow.clone(),
        views: running
            .statuses
            .iter()
            .map(|(view, status)| FlowViewStatus {
                view: *view,
                status: status.clone(),
            })
            .collect(),
        started_at_ms: running.started_at_ms,
        queued: super::queue::waiting(),
    });
    (running, state.last.as_ref().map(|last| last.dir.clone()))
}

/// The panel's state from the app's: the run going on and the last run's result, read from its
/// folder.
#[must_use]
pub fn state_of(
    project: bool,
    running: Option<FlowRunning>,
    last_dir: Option<&Path>,
) -> FlowRunState {
    FlowRunState {
        project,
        running,
        last: last_dir
            .and_then(result::read)
            .and_then(|last| serde_json::to_value(last).ok()),
    }
}

/// The flows the page menu last listed, with their project's root.
static KNOWN: Mutex<Option<(PathBuf, Vec<String>)>> = Mutex::new(None);

/// The project's flows for the page menu's "Run flow" (RB-090-60): the last listing of the open
/// project - `None` before the first - while a new one is read on a blocking thread for the next
/// menu. The menu is built on the UI thread, which never reads the disk (a WSL project's folder
/// is a network share).
pub fn menu_flows(core: &Core) -> Option<Vec<String>> {
    let root = core.host().project_root()?;
    let dir = core.user_settings().automation.flows.dir;
    let known = known_for(&root);
    core.runtime().spawn_blocking(move || {
        if let Ok(flows) = catalog::list(&root, &dir) {
            let names = flows.into_iter().map(|flow| flow.name).collect();
            remember(root, names);
        }
    });
    known
}

/// The flows last listed for `root`.
pub fn known_for(root: &Path) -> Option<Vec<String>> {
    lock(&KNOWN)
        .as_ref()
        .filter(|(at, _)| at == root)
        .map(|(_, names)| names.clone())
}

/// Keeps `names` as `root`'s flows.
pub fn remember(root: PathBuf, names: Vec<String>) {
    *lock(&KNOWN) = Some((root, names));
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
