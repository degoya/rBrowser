//! MCP's `recorder_start` and `recorder_stop` (RB-030-01's server registers them; RB-050-02
//! adds `recorder_export`): the arguments, the answer and the functions behind them. The
//! commands `recorder.start` (`covered(recorder_start)`), `recorder.stop` and `recorder.pause`
//! (`covered(recorder_stop)`, `pause: true`) run the same functions. An answer never carries a
//! recorded value - only the state, the name, the step count and the file.

use rb_commands::CommandError;
use rb_engine::Target;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::glue::{Outcome, RecorderFailure};
use super::recorder_of;
use crate::control::Core;

/// `recorder_start`'s name.
pub const RECORDER_START_TOOL: &str = "recorder_start";
/// `recorder_start`'s description for the MCP tool list.
pub const RECORDER_START_DESCRIPTION: &str = "Start recording the active view's interactions \
     (clicks, typing, keys, form changes, navigations) as a cleaned flow, or resume a paused \
     recording. `name` ([A-Za-z0-9_-], at most 64 characters) names the file; a timestamped \
     name when absent. `target: <viewId>` makes that view the active one first. The recorder \
     hears what the active view's sync agent captures: refused while sync is off or the view \
     is excluded, and while a flow runs (recorder.flow_running); `notCaptured` names the step \
     kinds whose sync switch is off. Password \
     fields and the configured secret fields are recorded as `{{secret:<name>}}`, never in \
     clear, and tokens in URLs masked. Answers the recorder's state, the name and the step \
     count.";
/// `recorder_stop`'s name.
pub const RECORDER_STOP_TOOL: &str = "recorder_stop";
/// `recorder_stop`'s description for the MCP tool list.
pub const RECORDER_STOP_DESCRIPTION: &str = "Stop the recording and write it to the project's \
     recordings folder (`automation.recorder.dir`, default `.rbrowser/out/recordings/<name>.json`); \
     an existing file is never overwritten - the next free `<name>-<n>` is taken. With \
     `pause: true` the recording only pauses (no steps until `recorder_start` resumes it). \
     Answers the state, the name, the step count and, after a stop, the file relative to the \
     project root.";

/// The arguments of `recorder_start`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStartArgs {
    /// `"all"` (absent) or the view to record from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The recording's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// The arguments of `recorder_stop`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderStopArgs {
    /// `"all"` (absent); the recorder belongs to the window, not a view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// Pause (or resume) instead of stopping.
    #[serde(default)]
    pub pause: bool,
}

/// The answer of both tools.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderAnswer {
    /// `idle`, `recording` or `paused`.
    pub state: String,
    /// The recording's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The steps so far, or in the file.
    pub steps: usize,
    /// After a stop: the file, relative to the project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// After a start: the kinds of steps not captured because their sync switch is off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_captured: Vec<String>,
}

impl From<Outcome> for RecorderAnswer {
    fn from(outcome: Outcome) -> Self {
        let state = serde_json::to_value(outcome.state)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        RecorderAnswer {
            state,
            name: outcome.name,
            steps: outcome.steps,
            path: outcome.path,
            not_captured: outcome
                .not_captured
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

/// Runs `recorder_start` with `args` (its JSON).
pub fn recorder_start(core: &Core, args: &Value) -> Result<RecorderAnswer, CommandError> {
    let args: RecorderStartArgs = parse(args)?;
    let target = match args.target {
        Some(Target::View(view)) => Some(view),
        _ => None,
    };
    // The view becomes the active one only once the name, the folder and sync are checked.
    let recorder = recorder_of(core);
    recorder
        .start(args.name.as_deref(), target)
        .map(RecorderAnswer::from)
        .map_err(failed)
}

/// Runs `recorder_stop` with `args` (its JSON).
pub fn recorder_stop(core: &Core, args: &Value) -> Result<RecorderAnswer, CommandError> {
    let args: RecorderStopArgs = parse(args)?;
    if let Some(Target::View(_)) = args.target {
        return Err(CommandError::InvalidArgs(
            "the recorder belongs to the window: use `target: \"all\"`".to_owned(),
        ));
    }
    let recorder = recorder_of(core);
    if args.pause {
        return recorder.pause().map(RecorderAnswer::from).map_err(failed);
    }
    let outcome = recorder.stop().map_err(failed)?;
    // An open Recorder panel lists the new recording (RB-090-60).
    core.send(&rb_ipc::CoreToShell::RecordingsChanged);
    Ok(RecorderAnswer::from(outcome))
}

fn parse<T: Default + serde::de::DeserializeOwned>(args: &Value) -> Result<T, CommandError> {
    if args.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

fn failed(failure: RecorderFailure) -> CommandError {
    match failure {
        RecorderFailure::Store(rb_sync::recorder::StoreError::BadName(_))
        | RecorderFailure::View(_) => CommandError::InvalidArgs(failure.to_string()),
        other => CommandError::Failed(other.to_string()),
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
