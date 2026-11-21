//! Pausing a video recording (RB-090-68): `capture.video.pause` (`Leader C Z`, the activity
//! bar's Pause/Resume, the page menu's Video submenu) and `record_video` `pause`/`resume`.
//!
//! A [`Pauser`] belongs to one [`super::recorder::Recording`]: it logs the pauses on the
//! recording's clock ([`Pauses`]) and hands them to every writer, whose pacer leaves the paused
//! time out of the file ([`super::pacing`]), and to the dispatcher, which holds the maximum
//! duration while paused, so it counts recorded time. The screencasts go on while paused: the
//! page's latest frame is the one the file goes on with. The running recording's pause state is
//! the activity bar's entry and the toolbar's indicator (`VideoActivity::paused_at_ms`).

use std::sync::Arc;

use rb_ipc::NotificationLevel;
use tokio::sync::watch;

use super::VideoError;
use super::ffmpeg::notice;
use super::pacing::Pauses;
use crate::control::Core;

/// The recording paused.
pub const PAUSED_KEY: &str = "video.notice.paused";
/// The recording goes on.
pub const RESUMED_KEY: &str = "video.notice.resumed";

/// Pauses and resumes one recording; clones share the one log.
#[derive(Clone)]
pub struct Pauser {
    pauses: Arc<watch::Sender<Pauses>>,
    clock: Arc<dyn Fn() -> f64 + Send + Sync>,
}

impl Pauser {
    /// A pauser on `clock` (seconds, the frames' clock), not paused.
    pub fn new(clock: Arc<dyn Fn() -> f64 + Send + Sync>) -> Self {
        let (pauses, _) = watch::channel(Pauses::default());
        Self {
            pauses: Arc::new(pauses),
            clock,
        }
    }

    /// Pauses now; `false` when paused already.
    pub fn pause(&self) -> bool {
        let now = (self.clock)();
        self.pauses.send_if_modified(|pauses| pauses.pause(now))
    }

    /// Resumes now; `false` when not paused.
    pub fn resume(&self) -> bool {
        let now = (self.clock)();
        self.pauses.send_if_modified(|pauses| pauses.resume(now))
    }

    /// Whether the recording is paused.
    pub fn is_paused(&self) -> bool {
        self.pauses.borrow().is_paused()
    }

    /// The pauses so far.
    pub fn pauses(&self) -> Pauses {
        self.pauses.borrow().clone()
    }

    /// Follows the pauses (the writers, the dispatcher).
    pub fn subscribe(&self) -> watch::Receiver<Pauses> {
        self.pauses.subscribe()
    }
}

/// The state a change left: paused or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseState {
    /// The recording is paused now.
    pub paused: bool,
    /// The call changed it (`false`: it was so already).
    pub changed: bool,
}

/// What a pause call asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseRequest {
    /// Pause.
    Pause,
    /// Resume.
    Resume,
    /// Pause a running recording, resume a paused one.
    Toggle,
}

/// Applies `request` to `pauser`.
pub fn apply(pauser: &Pauser, request: PauseRequest) -> PauseState {
    let pause = match request {
        PauseRequest::Pause => true,
        PauseRequest::Resume => false,
        PauseRequest::Toggle => !pauser.is_paused(),
    };
    let changed = if pause {
        pauser.pause()
    } else {
        pauser.resume()
    };
    PauseState {
        paused: pauser.is_paused(),
        changed,
    }
}

/// Milliseconds of `secs` seconds, never below 0.
pub fn millis(secs: f64) -> u64 {
    if secs.is_finite() && secs > 0.0 {
        // At most a few centuries of milliseconds: inside u64.
        (secs * 1000.0).round() as u64
    } else {
        0
    }
}

/// Pauses, resumes or toggles the running recording; the activity bar, the toolbar's indicator
/// and the status bar follow.
///
/// # Errors
///
/// [`VideoError::NotRecording`].
pub fn set(core: &Core, request: PauseRequest) -> Result<PauseState, VideoError> {
    let pauser = super::app::running_pauser().ok_or(VideoError::NotRecording)?;
    let state = apply(&pauser, request);
    if state.changed {
        let pauses = pauser.pauses();
        crate::activity_bar::publish_video_pause(
            core.host(),
            pauses.paused_since().map(millis),
            millis(pauses.ended_secs()),
        );
        let key = if state.paused {
            PAUSED_KEY
        } else {
            RESUMED_KEY
        };
        core.notify(notice(NotificationLevel::Info, key, &[]));
    }
    Ok(state)
}

#[cfg(test)]
#[path = "pause_tests.rs"]
mod tests;
