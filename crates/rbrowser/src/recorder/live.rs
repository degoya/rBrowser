//! The steps of the recording that runs, live in the Recorder panel (RB-090-89,
//! `automation.recorder.showLiveSteps`, on by default). Each time the window's recorder publishes
//! its state ([`super::glue`]), the steps from the first one added, changed or taken out since the
//! last push - a new step, a fill that grew, a hover that gained its pointer path - go to the shell
//! as `recordingSteps` ([`next`], `Recorder::take_changed`): never the whole recording again. The
//! end says why: `ended` once it is saved, `hidden` when the setting went off. A panel opened
//! during a recording asks for every step once (`resendRecordingSteps`, [`resend`]).
//!
//! Each window's state lives in its [`HostRecorder`] behind one lock, taken before the recorder's
//! and held through the send: the pushes leave in the order the steps changed.

use std::sync::{Arc, Weak};

use rb_ipc::{CoreToShell, LiveState, RecordingSteps};
use rb_sync::recorder::{Recorder, Step};
use serde_json::Value;

use super::glue::HostRecorder;
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

/// What one window's shell lists of the recording that runs.
#[derive(Debug, Default)]
pub struct Live {
    /// Where the steps go; none until the window's core made the recorder.
    core: Weak<Core>,
    /// The recording the shell lists; none while it lists none.
    shown: Option<String>,
}

impl Live {
    /// The steps go to `core`'s shell.
    pub fn attach(&mut self, core: &Core) {
        if let Some(core) = core.arc() {
            self.core = Arc::downgrade(&core);
        }
    }
}

/// The push that brings the shell from listing `shown` to `recorder`'s state, and `shown`
/// updated; `None` when the shell's list stays as it is. `on`: `showLiveSteps`; `all`: every
/// step again (a panel catching up).
pub fn next(
    shown: &mut Option<String>,
    recorder: &mut Recorder,
    on: bool,
    all: bool,
) -> Option<RecordingSteps> {
    let running = recorder.name().map(str::to_owned);
    let Some(name) = running.clone().filter(|_| on) else {
        // Saved, or the setting went off: the shell lets its list go, once; nobody shows the
        // changes, so they would only make every event publish.
        recorder.forget_changes();
        let name = shown.take()?;
        let state = match running {
            Some(_) => LiveState::Hidden,
            None => LiveState::Ended,
        };
        return Some(RecordingSteps {
            name,
            state,
            ..RecordingSteps::default()
        });
    };
    if all || shown.as_deref() != Some(name.as_str()) {
        // A new recording, the setting on again, or a catch-up: every step.
        recorder.take_changed();
        *shown = Some(name.clone());
        return Some(RecordingSteps {
            name,
            from: 0,
            steps: values(&recorder.steps_from(0)),
            state: LiveState::Recording,
            continue_after: None,
        });
    }
    let (from, steps) = recorder.take_changed()?;
    Some(RecordingSteps {
        name,
        from: u32::try_from(from).unwrap_or(u32::MAX),
        steps: values(&steps),
        state: LiveState::Recording,
        continue_after: None,
    })
}

/// `recorder` published its state (`all`: a panel catches up): what changed goes to the shell.
pub fn publish(recorder: &HostRecorder, host: &Host, all: bool) {
    let on = host.config().automation.recorder.show_live_steps;
    let mut live = lock(&recorder.live);
    let mut message = next(&mut live.shown, &mut lock(&recorder.recorder), on, all);
    // A recording continued from a step says where its new steps land (RB-092-01).
    if let Some(message) = message
        .as_mut()
        .filter(|message| message.state == LiveState::Recording)
    {
        message.continue_after = lock(&recorder.continuing)
            .as_ref()
            .map(|continuing| continuing.after);
    }
    if let (Some(message), Some(core)) = (message, live.core.upgrade()) {
        core.send(&CoreToShell::RecordingSteps(message));
    }
}

/// `resendRecordingSteps`: every step of the recording that runs goes to `core`'s shell again.
pub fn resend(core: &Core) {
    if let Some(recorder) = super::for_host(core.host()) {
        publish(&recorder, core.host(), true);
    }
}

fn values(steps: &[Step]) -> Vec<Value> {
    steps
        .iter()
        .filter_map(|step| serde_json::to_value(step).ok())
        .collect()
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
