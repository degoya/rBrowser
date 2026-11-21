//! Deleting a stored recording (RB-090-91): `recorder.delete { recordingId }` (as `recorder.play` names it) (`Leader F M`, reMove)
//! removes `<automation.recorder.dir>/<id>.json` - a person's, from the Recorder panel's
//! "Delete" or a key; MCP has no tool (an agent never deletes project files). Without an id the
//! command opens the Recorder panel.
//!
//! The id is checked as `recorder.play` checks it (`rb_sync::recorder::check_name`: no path, no
//! `..`, no device name), the store keeps to its folder. The recording being recorded and the one
//! playing (a played recording's run carries its id as the flow's name) are not deleted; the
//! check and the delete run under the flows' gate, so no play or recording starts in between. The
//! file goes on a blocking thread (a WSL project's disk is a network share); a notice says how it
//! went, and `recordingsChanged` has an open Recorder panel list the recordings again.

use rb_commands::CommandError;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_sync::recorder::check_name;
use serde_json::{Value, json};

use super::glue;
use crate::control::Core;
use crate::flows::exclusive::{exclusively, running_flow};

/// `Leader F M`: delete the recording `id`; without one the Recorder panel opens.
pub const DELETE: &str = "recorder.delete";
/// The notice of a deleted recording; `{name}`.
pub const DELETED_NOTICE: &str = "recorder.delete.notice.deleted";
/// The notice of a recording not deleted; `{name}` and `{error}`.
pub const FAILED_NOTICE: &str = "recorder.delete.notice.failed";

/// Why the recording `id` may not go now: it is being recorded (`recording_now`) or plays
/// (`playing`, the flow running now).
///
/// # Errors
///
/// The reason, safe to show.
pub fn refusal(id: &str, recording_now: Option<&str>, playing: Option<&str>) -> Result<(), String> {
    if recording_now == Some(id) {
        return Err(format!(
            "the recording {id:?} is being recorded: stop it (recorder.stop) before deleting it"
        ));
    }
    if playing == Some(id) {
        return Err(format!(
            "the recording {id:?} is playing: delete it once its run has ended, or cancel it (flows.cancel)"
        ));
    }
    Ok(())
}

/// `recorder.delete`: without `recordingId` the Recorder panel opens; with one the id is checked, then
/// the recording is deleted on a blocking thread of the core's runtime. Answers at once; the
/// outcome is a notice.
///
/// # Errors
///
/// A `recordingId` that is no recording name, the recording running or playing, no project open, or a
/// closed window.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let id = match args.get("recordingId") {
        None | Some(Value::Null) => {
            let recorder = crate::panel_dock::core_panels::RECORDER;
            crate::panel_dock::core_panels::open_unfolded(core, recorder);
            return Ok(Value::Null);
        }
        Some(Value::String(id)) => id.clone(),
        Some(_) => {
            return Err(CommandError::InvalidArgs(
                "`recordingId` is the recording's name, a string".to_owned(),
            ));
        }
    };
    check_name(&id).map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    refusal(
        &id,
        recording_now(core).as_deref(),
        running_flow().as_deref(),
    )
    .map_err(CommandError::Failed)?;
    if core.host().project_root().is_none() {
        return Err(CommandError::Failed("no project is open".to_owned()));
    }
    let core = core
        .arc()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    let answer = json!({ "recordingId": id });
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        // No recording or play of it starts between the check and the delete.
        let outcome = exclusively(|| {
            refusal(
                &id,
                recording_now(&core).as_deref(),
                running_flow().as_deref(),
            )?;
            let store = glue::store_of(core.host()).map_err(|error| error.to_string())?;
            store.delete(&id).map_err(|error| error.to_string())
        });
        if let Err(error) = &outcome {
            tracing::warn!(error = error.as_str(), "a recording was not deleted");
        } else {
            tracing::info!("recording deleted");
        }
        core.notify(outcome_notice(&id, &outcome));
        core.send(&CoreToShell::RecordingsChanged);
    });
    Ok(answer)
}

/// The name of the recording running (or paused) in `core`'s window.
fn recording_now(core: &Core) -> Option<String> {
    super::for_host(core.host()).and_then(|recorder| recorder.outcome().name)
}

/// The notice of a delete: done, or why not.
#[must_use]
pub fn outcome_notice(id: &str, outcome: &Result<(), String>) -> CoreToShell {
    let mut args = vec![("name".to_owned(), id.to_owned())];
    let (level, key) = match outcome {
        Ok(()) => (NotificationLevel::Info, DELETED_NOTICE),
        Err(error) => {
            args.push(("error".to_owned(), rb_diag::redact(error)));
            (NotificationLevel::Error, FAILED_NOTICE)
        }
    };
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args.into_iter().collect(),
        settings_key: None,
    })
}

#[cfg(test)]
#[path = "delete_tests.rs"]
mod tests;
