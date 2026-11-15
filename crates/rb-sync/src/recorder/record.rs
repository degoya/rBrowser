//! [`Recorder`]: records the leader's captured events as a [`Recording`] (RB-050-01), without a
//! runtime. The app feeds it what the SyncHub sees ([`crate::Observer`]) and the leader's
//! committed navigations; `stop` hands over the finished recording.
//!
//! Every event goes through [`crate::redact`] before the recorder looks at it, so a password
//! field's value never reaches the model; its `fill` holds `{{secret:<name>}}`, marked `secret`
//! ([`super::secrets`]), and of its keys only the named ones are steps. A URL's tokens (an
//! OAuth `code`, an `access_token`, a signature) are recorded masked
//! ([`super::secrets::mask_url`]). The clean-up rules are [`super::cleanup`].
//!
//! `onlyIn` comes from the replays: a step runs in the leader and in every view its events were
//! replayed into; a view whose replay of one of them failed (not found, not visible, a timeout,
//! an error - the same outcomes that desync it) drops out, and a view the hub did not replay
//! into (desynced, excluded, loading) never was in. A step that ran in every breakpoint of the
//! recording has no `onlyIn`.

use std::collections::{BTreeMap, BTreeSet};

use rb_engine::ViewId;
use serde::Serialize;

use super::cleanup::{Builder, Capture, Trail, shown};
use super::model::{Action, RECORDING_VERSION, RecordedBreakpoint, Recording};
use super::secrets::mask_url;
use crate::event::SyncEvent;
use crate::message::EventMessage;
use crate::replay::ReplayOutcome;

/// A navigation this soon after a click, Enter or a submit is its result: `waitForUrl`.
pub const NAVIGATION_WINDOW_MS: u64 = 3_000;
/// The most steps a recording takes; later events are dropped.
pub const MAX_STEPS: usize = 10_000;

/// Whether the recorder runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecorderState {
    /// No recording.
    #[default]
    Idle,
    /// Recording.
    Recording,
    /// Recording, steps suppressed until it resumes.
    Paused,
}

/// The settings `automation.recorder.*` the recorder follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecorderOptions {
    /// `recordScroll`: scrolls are steps.
    pub record_scroll: bool,
    /// `recordHover`: a hover that reveals what the next click hits is a step.
    pub record_hover: bool,
    /// `secretFields`: further fields whose values are never recorded.
    pub secret_fields: Vec<String>,
}

impl Default for RecorderOptions {
    fn default() -> Self {
        Self {
            record_scroll: false,
            record_hover: true,
            secret_fields: Vec::new(),
        }
    }
}

/// A view taking part, with its breakpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingView {
    /// The view.
    pub view: ViewId,
    /// Its breakpoint and emulation.
    pub breakpoint: RecordedBreakpoint,
}

/// What a recording starts with.
#[derive(Clone, Debug, PartialEq)]
pub struct StartInfo {
    /// The recording's name.
    pub name: String,
    /// The leader's URL.
    pub start_url: Option<String>,
    /// The environment in effect.
    pub environment: Option<String>,
    /// The app's version.
    pub app_version: String,
    /// The views.
    pub views: Vec<RecordingView>,
    /// The active view.
    pub leader: Option<ViewId>,
    /// Now, ms since the epoch.
    pub now_ms: u64,
}

/// Why a recorder call did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RecorderError {
    /// `start` while recording.
    #[error("a recording is already running")]
    AlreadyRecording,
    /// `stop` or `pause` without a recording.
    #[error("no recording is running")]
    NotRecording,
}

/// The recorder of one window.
#[derive(Debug, Default)]
pub struct Recorder {
    options: RecorderOptions,
    state: RecorderState,
    recording: Option<Recording>,
    started_ms: u64,
    names: BTreeMap<ViewId, String>,
    leader: Option<ViewId>,
    last_url: Option<String>,
    steps: Builder,
}

impl Recorder {
    /// An idle recorder.
    pub fn new(options: RecorderOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// The settings from now on.
    pub fn set_options(&mut self, options: RecorderOptions) {
        self.options = options;
    }

    /// Idle, recording or paused.
    pub fn state(&self) -> RecorderState {
        self.state
    }

    /// The steps so far.
    pub fn step_count(&self) -> usize {
        self.steps.entries.len()
    }

    /// The running recording's name.
    pub fn name(&self) -> Option<&str> {
        self.recording
            .as_ref()
            .map(|recording| recording.name.as_str())
    }

    /// Starts a recording; its first step loads the start URL.
    pub fn start(&mut self, info: StartInfo) -> Result<(), RecorderError> {
        if self.state != RecorderState::Idle {
            return Err(RecorderError::AlreadyRecording);
        }
        self.steps = Builder::default();
        self.names.clear();
        self.started_ms = info.now_ms;
        self.leader = info.leader;
        self.recording = Some(Recording {
            schema: None,
            version: RECORDING_VERSION,
            name: info.name,
            start_url: info
                .start_url
                .as_deref()
                .filter(|url| !url.is_empty())
                .map(|url| mask_url(url, &self.options.secret_fields)),
            environment: info.environment,
            breakpoints: Vec::new(),
            started_at_ms: info.now_ms,
            app_version: info.app_version,
            steps: Vec::new(),
        });
        self.set_views(&info.views);
        self.state = RecorderState::Recording;
        self.last_url = None;
        if let Some(url) = info.start_url.filter(|url| !url.is_empty()) {
            self.navigation(url, info.now_ms, false);
        }
        Ok(())
    }

    /// Pauses: events are no steps until [`Recorder::resume`].
    pub fn pause(&mut self) -> Result<(), RecorderError> {
        match self.state {
            RecorderState::Idle => Err(RecorderError::NotRecording),
            _ => {
                self.state = RecorderState::Paused;
                self.steps.drop_hovers();
                Ok(())
            }
        }
    }

    /// Resumes; when the leader is now at another URL than recorded, a `navigate` step takes
    /// the recording there.
    pub fn resume(&mut self, current_url: Option<&str>, now_ms: u64) -> Result<(), RecorderError> {
        match self.state {
            RecorderState::Idle => Err(RecorderError::NotRecording),
            RecorderState::Recording => Ok(()),
            RecorderState::Paused => {
                self.state = RecorderState::Recording;
                self.steps.last_trigger_ms = None;
                if let Some(url) = current_url.filter(|url| !url.is_empty())
                    && self.last_url.as_deref() != Some(url)
                {
                    self.navigation(url.to_owned(), now_ms, false);
                }
                Ok(())
            }
        }
    }

    /// Stops and hands over the recording, `onlyIn` filled in.
    pub fn stop(&mut self) -> Result<Recording, RecorderError> {
        let recording = self.snapshot()?;
        self.recording = None;
        self.state = RecorderState::Idle;
        self.steps = Builder::default();
        Ok(recording)
    }

    /// The recording as [`Recorder::stop`] would hand it over, while it goes on: the caller
    /// stops only once it is saved, so a failed save loses nothing.
    pub fn snapshot(&self) -> Result<Recording, RecorderError> {
        let mut recording = self.recording.clone().ok_or(RecorderError::NotRecording)?;
        let all: BTreeSet<&str> = self.names.values().map(String::as_str).collect();
        recording.steps = self
            .steps
            .entries
            .iter()
            .map(|entry| {
                let mut step = entry.step.clone();
                let ran: BTreeSet<&str> = entry
                    .trail
                    .views
                    .iter()
                    .filter_map(|view| self.names.get(view).map(String::as_str))
                    .collect();
                if ran != all {
                    step.only_in = ran.into_iter().map(str::to_owned).collect();
                }
                step
            })
            .collect();
        Ok(recording)
    }

    /// The views from now on: a new view's breakpoint joins the recording.
    pub fn set_views(&mut self, views: &[RecordingView]) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        for view in views {
            self.names.insert(view.view, view.breakpoint.id.clone());
            if !recording
                .breakpoints
                .iter()
                .any(|known| known.id == view.breakpoint.id)
            {
                recording.breakpoints.push(view.breakpoint.clone());
            }
        }
    }

    /// The active view changed.
    pub fn set_leader(&mut self, leader: Option<ViewId>) {
        self.leader = leader;
    }

    /// The leader `source` captured `message`; the hub replayed it into `targets`.
    pub fn on_event(
        &mut self,
        source: ViewId,
        message: &EventMessage,
        targets: &[ViewId],
        now_ms: u64,
    ) {
        if self.state != RecorderState::Recording || self.steps.entries.len() >= MAX_STEPS {
            return;
        }
        self.leader = Some(source);
        let Some(event) = shown(&message.event) else {
            return;
        };
        let view = self.name_of(source);
        let mut views: BTreeSet<ViewId> = targets.iter().copied().collect();
        views.insert(source);
        let frame = if message.top {
            None
        } else {
            message.frame.as_deref()
        };
        let capture = Capture {
            view: &view,
            frame,
            views,
            at_ms: self.offset(now_ms),
            record_hover: self.options.record_hover,
            record_scroll: self.options.record_scroll,
            secret_fields: &self.options.secret_fields,
        };
        self.steps.event(&capture, &event);
    }

    /// The replay of `event` into `view` ended with `outcome`.
    pub fn on_replayed(&mut self, view: ViewId, event: &SyncEvent, outcome: &ReplayOutcome) {
        if self.recording.is_none()
            || matches!(outcome, ReplayOutcome::Done | ReplayOutcome::Unsupported)
        {
            return;
        }
        let path = crate::switches::selector_of(event).unwrap_or_default();
        self.steps
            .failed(view, crate::switches::event_type(event), &path);
    }

    /// `view` committed a navigation to `url`; only the leader's are steps.
    pub fn on_navigated(&mut self, view: ViewId, url: &str, now_ms: u64) {
        if self.state != RecorderState::Recording
            || self.leader != Some(view)
            || url.is_empty()
            || self.last_url.as_deref() == Some(url)
            || self.steps.entries.len() >= MAX_STEPS
        {
            return;
        }
        let triggered = self
            .steps
            .last_trigger_ms
            .take()
            .is_some_and(|at| self.offset(now_ms).saturating_sub(at) <= NAVIGATION_WINDOW_MS);
        self.navigation(url.to_owned(), now_ms, triggered);
    }

    fn navigation(&mut self, url: String, now_ms: u64, triggered: bool) {
        let shown = mask_url(&url, &self.options.secret_fields);
        self.last_url = Some(url);
        let url = shown;
        let action = if triggered {
            Action::WaitForUrl { url }
        } else {
            Action::Navigate { url }
        };
        let view = self
            .leader
            .map(|leader| self.name_of(leader))
            .unwrap_or_default();
        let trail = Trail {
            views: self.names.keys().copied().collect(),
            origins: Vec::new(),
        };
        let at = self.offset(now_ms);
        self.steps.push(action, &view, at, trail);
    }

    fn name_of(&self, view: ViewId) -> String {
        self.names
            .get(&view)
            .cloned()
            .unwrap_or_else(|| format!("view-{view}"))
    }

    fn offset(&self, now_ms: u64) -> u64 {
        now_ms.saturating_sub(self.started_ms)
    }
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
