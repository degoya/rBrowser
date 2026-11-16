//! Replays a [`FormAction`] into one view (RB-020-05): one evaluation in its agent's world
//! ([`super::script`]). Why the agent and not CDP input: a select's popup, a date picker or a
//! file chooser cannot be driven by input events, a hidden checkbox behind a styled label has no
//! point to click, and frameworks react to the events the agent dispatches (they do not check
//! `isTrusted`; React's tracker is passed by the prototype's setter). Clicks, keys and typed
//! text stay real input (RB-020-03); the `change` that follows them finds the state reached
//! and does nothing, so nothing toggles twice.

use rb_engine::ViewId;
use rb_sync::ReplayOutcome;

use super::plan::FormAction;
use super::script::{expression, outcome_of};
use crate::replay::ReplayPort;

/// Applies `action` in `view`: `Ok(true)` when the agent set something, `Ok(false)` when
/// nothing was sent (a file input), else the miss or failure. `block_submit`: the view must not
/// submit a form because of it.
pub async fn replay(
    port: &dyn ReplayPort,
    view: ViewId,
    action: FormAction,
    block_submit: bool,
) -> Result<bool, ReplayOutcome> {
    let Some(command) = action.command() else {
        if let FormAction::FileSkipped { field } = &action {
            tracing::debug!(%view, field = %field, "file input skipped");
        }
        return Ok(false);
    };
    let source = expression(&command, block_submit)
        .map_err(|error| ReplayOutcome::Failed(error.to_string()))?;
    let value = port
        .world(view, source)
        .await
        .map_err(|error| ReplayOutcome::Failed(error.to_string()))?;
    match outcome_of(&value) {
        ReplayOutcome::Done => Ok(true),
        other => Err(other),
    }
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
