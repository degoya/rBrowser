//! "Continue recording from here" (RB-092-01, `recorder.continue_from`, `Leader F U`): the
//! stored recording's steps up to the one chosen play first, at the pace `fast`, in the
//! recording's views - so the pages are where the step left them -, then a recording starts
//! without a first `navigate`; its stop puts the new steps after the chosen one and writes the
//! recording over its file ([`Continuing::save`], `rb_sync::recorder::edit::splice`). When a
//! step before fails, no recording starts and the notice names why (the owner's decision,
//! 2026-11-20).
//!
//! `{ recordingId, stepId, views? }`; without `recordingId` the command goes to the Recorder
//! panel, which runs it on the step in focus ([`super::steps::forward`]).

use std::path::PathBuf;
use std::sync::Arc;

use rb_commands::CommandError;
use rb_config::PlaybackPace;
use rb_engine::ViewHost as _;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_sync::recorder::{Recording, RecordingStore, edit};
use serde::Deserialize;
use serde_json::{Value, json};

use super::glue::RecorderFailure;
use super::play::{Part, prepare};
use crate::control::Core;
use crate::flows::app::{resolve_views, start_inline};
use crate::flows::result::RunResult;

/// `Leader F U`: continue the recording from a step.
pub const CONTINUE_FROM: &str = "recorder.continue_from";
/// The notice when the steps before did not play through; `{name}`, `{error}`.
pub const FAILED_NOTICE: &str = "recorder.continue.notice.failed";

/// The recording a running one continues, and the step after which the new steps land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continuing {
    /// The stored recording.
    pub name: String,
    /// The step the new ones follow.
    pub after: u32,
}

impl Continuing {
    /// Puts `segment` - the recording that ran from the step - into the stored recording after
    /// the step and writes it over its file; the whole recording and its file.
    ///
    /// # Errors
    ///
    /// The stored recording is gone or unreadable, the step is gone, writing failed.
    pub fn save(
        &self,
        store: &RecordingStore,
        segment: &Recording,
    ) -> Result<(Recording, PathBuf), RecorderFailure> {
        // No other write of the file comes between the read and the write.
        let _written = rb_sync::recorder::write_lock();
        let base = store.load(&self.name)?;
        let whole = edit::splice(&base, self.after, segment)?;
        let path = store.replace(&whole)?;
        Ok((whole, path))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Args {
    recording_id: String,
    step_id: u32,
    #[serde(default)]
    views: Option<Value>,
}

/// `recorder.continue_from`: checks the arguments, then plays the steps up to the one chosen
/// and starts the recording on a blocking thread of the core's runtime. Answers at once; a
/// failure after that is [`FAILED_NOTICE`].
///
/// # Errors
///
/// Bad arguments, an unknown view, a recording that runs already, a closed window.
pub fn run(core: &Core, raw: &Value) -> Result<Value, CommandError> {
    if super::steps::forward(core, CONTINUE_FROM, raw) {
        return Ok(json!({ "forwarded": true }));
    }
    let args: Args = serde_json::from_value(raw.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let core = core
        .arc()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    if super::recorder_of(&core).outcome().name.is_some() {
        return Err(CommandError::Failed(
            "a recording runs: stop it before continuing another".to_owned(),
        ));
    }
    let views = resolve_views(
        &core.host().views(),
        args.views.as_ref().unwrap_or(&Value::Null),
    )
    .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let answer = json!({ "recordingId": args.recording_id, "stepId": args.step_id });
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        if let Err(error) = continue_now(&core, &args, views) {
            tracing::warn!(
                error = crate::flows::report::reason(&error).as_str(),
                "a recording could not continue"
            );
            core.notify(failed_notice(&args.recording_id, &error));
        }
    });
    Ok(answer)
}

/// Plays the steps up to `args.step_id` and, when they passed, starts the recording.
fn continue_now(
    core: &Arc<Core>,
    args: &Args,
    views: Option<Vec<rb_engine::ViewId>>,
) -> Result<(), String> {
    let part = Part {
        up_to: Some(args.step_id),
        ..Part::default()
    };
    let (name, flow) = prepare(
        core,
        Some(args.recording_id.clone()),
        Some(PlaybackPace::Fast),
        &part,
    )
    .map_err(|error| error.to_string())?;
    let started = start_inline(core, &name, flow, views).map_err(|error| error.to_string())?;
    let result = core
        .runtime()
        .block_on(started.done)
        .map_err(|_| "the run ended without a result".to_owned())?
        .map_err(|error| error.to_string())?;
    if let Some(reason) = failure(&result) {
        return Err(reason);
    }
    let recorder = super::recorder_of(core);
    *crate::cdp::lock(&recorder.continuing) = Some(Continuing {
        name: name.clone(),
        after: args.step_id,
    });
    match recorder.start(Some(&name), None) {
        Ok(_) => {
            tracing::info!("recording continued from a step");
            Ok(())
        }
        Err(error) => {
            *crate::cdp::lock(&recorder.continuing) = None;
            Err(error.to_string())
        }
    }
}

/// Why the steps before did not play through: the run's error, else the first failed view's
/// label and message; `None` when they passed.
#[must_use]
pub fn failure(result: &RunResult) -> Option<String> {
    if result.passed {
        return None;
    }
    if let Some(error) = &result.error {
        return Some(error.clone());
    }
    let failed = result.views.iter().find(|view| view.error.is_some());
    Some(failed.map_or_else(
        || "a step before it failed".to_owned(),
        |view| {
            let message = view
                .error
                .as_ref()
                .map_or("", |error| error.message.as_str());
            format!("{}: {message}", view.label)
        },
    ))
}

/// The notice that the recording `name` did not continue.
#[must_use]
pub fn failed_notice(name: &str, error: &str) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Error,
        message_key: FAILED_NOTICE.to_owned(),
        args: [
            ("name".to_owned(), name.to_owned()),
            ("error".to_owned(), rb_diag::redact(error)),
        ]
        .into_iter()
        .collect(),
        settings_key: None,
    })
}

#[cfg(test)]
#[path = "continue_from_tests.rs"]
mod tests;
