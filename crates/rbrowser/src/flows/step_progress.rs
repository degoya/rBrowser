//! The steps of a run going on, for the shell (RB-090-84): the sidecar's `$/progress` carries a
//! view's status (`{ viewId, status }`) or one of its steps (`{ viewId, step: { name, id?, state,
//! message? } }`, `sidecar/src/modules/flows/steps.ts`). A step becomes the pushed
//! `flowStep`, which the Recorder panel follows; a status stays the run's state
//! ([`super::app`]). A failed step's message is masked once more and cut.

use std::sync::Arc;

use rb_ipc::{CoreToShell, FlowStep};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::run::RunRequest;
use crate::control::Core;

/// The most characters of a step's message the shell gets.
pub const MAX_MESSAGE: usize = 300;
/// The most characters of a step's name the shell gets.
pub const MAX_NAME: usize = 500;

/// Follows the progress of the run `request` until the task is stopped: a step goes to the
/// shell, any other progress (a view's status) to `status`.
pub fn follow(
    core: &Arc<Core>,
    request: &RunRequest,
    mut progress: mpsc::UnboundedReceiver<Value>,
    status: fn(&Value),
) -> JoinHandle<()> {
    let (shell, run_id, flow) = (
        Arc::clone(core),
        request.run_id.clone(),
        request.name.clone(),
    );
    tokio::spawn(async move {
        while let Some(value) = progress.recv().await {
            match event(&run_id, &flow, &value) {
                Some(step) => shell.send(&CoreToShell::FlowStep(step)),
                None => status(&value),
            }
        }
    })
}

/// The `flowStep` of `progress` in the run `run_id` of `flow`; `None` for a status or anything
/// that is no step.
pub fn event(run_id: &str, flow: &str, progress: &Value) -> Option<FlowStep> {
    let step = progress.get("step")?;
    let view = u32::try_from(progress.get("viewId")?.as_u64()?).ok()?;
    let name = step.get("name")?.as_str()?;
    let state = step.get("state")?.as_str()?;
    if !matches!(state, "running" | "passed" | "failed") {
        return None;
    }
    Some(FlowStep {
        run_id: run_id.to_owned(),
        flow: flow.to_owned(),
        view,
        name: name.chars().take(MAX_NAME).collect(),
        step_id: step
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok()),
        state: state.to_owned(),
        message: step
            .get("message")
            .and_then(Value::as_str)
            .map(|message| rb_diag::redact(message).chars().take(MAX_MESSAGE).collect()),
    })
}

#[cfg(test)]
#[path = "step_progress_tests.rs"]
mod tests;
