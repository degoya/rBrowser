//! [`HostRecorder`]: the recorder of one window. It hears the SyncHub through
//! [`rb_sync::Observer`] while a recording runs, follows the host's views, active view and
//! navigations, keeps the toolbar's recording indicator in the shell's state and writes the
//! finished recording through [`RecordingStore`].

use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use rb_config::RecordHover;
use rb_engine::{ViewEvent, ViewHost, ViewId};
use rb_ipc::{RecorderPhase, RecorderStatus};
use rb_sync::recorder::{
    HoverCapture, RecordedBreakpoint, Recorder, RecorderOptions, RecorderState, RecordingStore,
    RecordingView, StartInfo, StoreError, check_name,
};
use rb_sync::{EventMessage, Observer, ReplayOutcome, Route, SyncEvent, SyncSwitches};
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

use crate::cdp::lock;
use crate::flows::exclusive::{exclusively, running_flow};
use crate::host::Host;

/// The code in the refusal of a recording while a flow runs (RB-090-71).
pub const FLOW_RUNNING: &str = "recorder.flow_running";

/// What a recorder command did: the state and, after a stop, where the file is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Idle, recording or paused.
    pub state: RecorderState,
    /// The recording's name.
    pub name: Option<String>,
    /// The steps so far (after a stop: in the file).
    pub steps: usize,
    /// After a stop: the file, relative to the project root with `/`.
    pub path: Option<String>,
    /// After a start: the kinds of steps the active view's agent does not capture, because
    /// their sync switch is off (`keyboard`, `input`) - the recording will lack them.
    pub not_captured: Vec<&'static str>,
}

/// Why a recorder command failed; its text is safe to show (no value of a field).
#[derive(Debug, thiserror::Error)]
pub enum RecorderFailure {
    /// No project is open, so there is nowhere to store a recording.
    #[error("no project is open")]
    NoProject,
    /// The SyncHub does not run, so no events reach the recorder.
    #[error("sync is not started")]
    NoSync,
    /// The active view's agent captures nothing (sync off, the view excluded, no active view):
    /// the recording would stay empty.
    #[error("{0}")]
    NotCaptured(&'static str),
    /// The view to record from cannot become the active one.
    #[error("{0}")]
    View(String),
    /// Start, stop or pause in the wrong state.
    #[error(transparent)]
    State(#[from] rb_sync::recorder::RecorderError),
    /// The folder or the name is not allowed, or writing failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A flow runs: the recording would take its navigations as steps (RB-090-71).
    #[error(
        "the flow {0:?} is running: stop it (flows.cancel) or wait for its end before a recording takes steps (recorder.flow_running)"
    )]
    FlowRunning(String),
}

/// Refuses a recording's start or resume while the flow `flow` runs (RB-090-71).
///
/// # Errors
///
/// [`RecorderFailure::FlowRunning`].
pub fn not_while_flow(flow: Option<&str>) -> Result<(), RecorderFailure> {
    match flow {
        Some(flow) => Err(RecorderFailure::FlowRunning(flow.to_owned())),
        None => Ok(()),
    }
}

/// The recorder of one window.
pub struct HostRecorder {
    host: Weak<Host>,
    recorder: Mutex<Recorder>,
}

impl HostRecorder {
    /// The recorder of `host`'s window, following its views on `runtime`.
    pub fn new(host: &Arc<Host>, runtime: &Handle) -> Arc<Self> {
        let recorder = Arc::new(Self {
            host: Arc::downgrade(host),
            recorder: Mutex::new(Recorder::new(RecorderOptions::default())),
        });
        recorder.watch_views(host, runtime);
        recorder
    }

    /// The name of the recording that takes steps now; none while paused or idle.
    pub fn taking_steps(&self) -> Option<String> {
        let recorder = lock(&self.recorder);
        (recorder.state() == RecorderState::Recording)
            .then(|| recorder.name().unwrap_or_default().to_owned())
    }

    /// Starts a recording named `name` (a default name when absent), or resumes a paused one;
    /// with `target`, that view becomes the active one first - only once every check passed.
    /// Refused while a flow runs, checked and claimed under the flows' gate.
    pub fn start(
        self: &Arc<Self>,
        name: Option<&str>,
        target: Option<ViewId>,
    ) -> Result<Outcome, RecorderFailure> {
        exclusively(|| {
            not_while_flow(running_flow().as_deref())?;
            self.start_now(name, target)
        })
    }

    fn start_now(
        self: &Arc<Self>,
        name: Option<&str>,
        target: Option<ViewId>,
    ) -> Result<Outcome, RecorderFailure> {
        let host = self.host.upgrade().ok_or(RecorderFailure::NoProject)?;
        let paused = lock(&self.recorder).state() == RecorderState::Paused;
        let name = match name {
            _ if paused => String::new(),
            Some(name) => {
                check_name(name)?;
                name.to_owned()
            }
            None => default_name(now_ms()),
        };
        if !paused {
            // The folder is checked before anything is recorded.
            store_of(&host)?;
        }
        let sync = crate::sync_hub::for_host(&host).ok_or(RecorderFailure::NoSync)?;
        let config = host.config().automation.recorder;
        let (switches, excluded) = sync.hub().with(|hub| {
            let leader = target.or_else(|| host.active_view());
            (
                hub.switches(),
                leader.is_some_and(|view| hub.is_excluded(view)),
            )
        });
        let has_leader = target.is_some() || host.active_view().is_some();
        if let Some(reason) = uncaptured(switches, excluded, has_leader) {
            return Err(RecorderFailure::NotCaptured(reason));
        }
        let records_hover = config.record_hover != RecordHover::Off;
        let not_captured = missing_kinds(switches, records_hover, config.record_scroll);
        if let Some(view) = target {
            host.set_active_view(view)
                .map_err(|error| RecorderFailure::View(error.to_string()))?;
        }
        if !not_captured.is_empty() {
            tracing::warn!(?not_captured, "the recording lacks steps whose sync is off");
        }
        if paused {
            return self.resume(&host).map(|outcome| Outcome {
                not_captured,
                ..outcome
            });
        }
        let leader = host.active_view();
        let info = StartInfo {
            name,
            start_url: leader.and_then(|view| host.view(view)).map(|info| info.url),
            environment: host.shell_state().environment,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            views: recording_views(&host),
            leader,
            now_ms: now_ms(),
        };
        {
            let mut recorder = lock(&self.recorder);
            recorder.set_options(RecorderOptions {
                record_scroll: config.record_scroll,
                record_hover: hover_capture(config.record_hover),
                secret_fields: config.secret_fields,
                record_autofill: config.record_autofill,
            });
            recorder.start(info)?;
        }
        let observer: Arc<dyn Observer> = self.clone();
        sync.hub().set_observer(Some(observer));
        tracing::info!("recording started");
        Ok(Outcome {
            not_captured,
            ..self.publish()
        })
    }

    /// Pauses a running recording, or resumes a paused one - not while a flow runs.
    pub fn pause(self: &Arc<Self>) -> Result<Outcome, RecorderFailure> {
        let host = self.host.upgrade().ok_or(RecorderFailure::NoProject)?;
        let resumed = exclusively(|| {
            if lock(&self.recorder).state() != RecorderState::Paused {
                return None;
            }
            Some(not_while_flow(running_flow().as_deref()).and_then(|()| self.resume(&host)))
        });
        if let Some(resumed) = resumed {
            return resumed;
        }
        lock(&self.recorder).pause()?;
        tracing::info!("recording paused");
        Ok(self.publish())
    }

    /// Stops the recording and writes it under its name or the next free one; a file is
    /// never overwritten. The recording ends only once it is saved: when the save fails it
    /// goes on, and the next stop tries again.
    pub fn stop(&self) -> Result<Outcome, RecorderFailure> {
        let host = self.host.upgrade().ok_or(RecorderFailure::NoProject)?;
        let (recording, path) = {
            // Held through the save, so no step comes in between the save and the end.
            let mut recorder = lock(&self.recorder);
            let mut recording = recorder.snapshot()?;
            let path = store_of(&host)?.save_new(&mut recording)?;
            recorder.stop()?;
            (recording, path)
        };
        if let Some(sync) = crate::sync_hub::for_host(&host) {
            sync.hub().set_observer(None);
        }
        self.publish();
        tracing::info!(steps = recording.steps.len(), "recording saved");
        Ok(Outcome {
            state: RecorderState::Idle,
            name: Some(recording.name),
            steps: recording.steps.len(),
            path: Some(relative(&host, &path)),
            not_captured: Vec::new(),
        })
    }

    /// A check step from the page menu (RB-090-60) for `views` (every view when empty).
    pub fn add_check(
        &self,
        source: ViewId,
        action: rb_sync::recorder::Action,
        views: &[ViewId],
    ) -> Result<Outcome, RecorderFailure> {
        lock(&self.recorder).add_check(source, action, views, now_ms())?;
        tracing::info!("check step recorded");
        Ok(self.publish())
    }

    /// The form autofill set `fill` (RB-090-78, [`super::autofill`]): its fields are steps while
    /// a recording runs with `automation.recorder.recordAutofill`.
    pub fn on_autofill(&self, fill: &rb_sync::recorder::AutofillView) {
        if lock(&self.recorder).on_autofill(fill, now_ms()) {
            self.publish();
        }
    }

    /// The state now.
    pub fn outcome(&self) -> Outcome {
        let recorder = lock(&self.recorder);
        Outcome {
            state: recorder.state(),
            name: recorder.name().map(str::to_owned),
            steps: recorder.step_count(),
            path: None,
            not_captured: Vec::new(),
        }
    }

    fn resume(&self, host: &Host) -> Result<Outcome, RecorderFailure> {
        let url = host
            .active_view()
            .and_then(|view| host.view(view))
            .map(|info| info.url);
        lock(&self.recorder).resume(url.as_deref(), now_ms())?;
        tracing::info!("recording resumed");
        Ok(self.publish())
    }

    /// Puts the indicator's state into the shell's state; answers it.
    fn publish(&self) -> Outcome {
        let outcome = self.outcome();
        let status = status_of(&outcome);
        if let Some(host) = self.host.upgrade() {
            host.update_shell_state(|shell| {
                // A recording that stops is offered for "Play last" (RB-090-67).
                crate::activity_bar::on_recorder(shell, status.as_ref());
                shell.recorder = status;
            });
        }
        outcome
    }

    fn watch_views(self: &Arc<Self>, host: &Host, runtime: &Handle) {
        let mut events = host.subscribe();
        let weak = Arc::downgrade(self);
        runtime.spawn(async move {
            loop {
                let event = events.recv().await;
                let Some(recorder) = weak.upgrade() else {
                    return;
                };
                let Some(host) = recorder.host.upgrade() else {
                    return;
                };
                match event {
                    Ok(ViewEvent::Navigated { view, url }) => {
                        lock(&recorder.recorder).on_navigated(view, &url, now_ms());
                        recorder.publish();
                    }
                    Ok(ViewEvent::ActiveChanged(active)) => {
                        lock(&recorder.recorder).set_leader(active);
                    }
                    Ok(ViewEvent::Opened(_) | ViewEvent::LayoutChanged)
                    | Err(RecvError::Lagged(_)) => {
                        lock(&recorder.recorder).set_views(&recording_views(&host));
                    }
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
        });
    }
}

impl Observer for HostRecorder {
    fn routed(&self, source: ViewId, message: &EventMessage, route: &Route) {
        let targets: Vec<ViewId> = match route {
            Route::Replay(requests) => requests.iter().map(|request| request.target).collect(),
            _ => Vec::new(),
        };
        let changed = {
            let mut recorder = lock(&self.recorder);
            let before = recorder.step_count();
            recorder.on_event(source, message, &targets, now_ms());
            recorder.step_count() != before
        };
        if changed {
            self.publish();
        }
    }

    fn replayed(&self, view: ViewId, event: &SyncEvent, outcome: &ReplayOutcome) {
        lock(&self.recorder).on_replayed(view, event, outcome);
    }
}

/// The indicator's state for `outcome`; `None` while idle.
pub fn status_of(outcome: &Outcome) -> Option<RecorderStatus> {
    let phase = match outcome.state {
        RecorderState::Idle => return None,
        RecorderState::Recording => RecorderPhase::Recording,
        RecorderState::Paused => RecorderPhase::Paused,
    };
    Some(RecorderStatus {
        phase,
        name: outcome.name.clone().unwrap_or_default(),
        steps: u32::try_from(outcome.steps).unwrap_or(u32::MAX),
    })
}

/// Why the active view's agent captures nothing - so a recording would stay empty: no active
/// view, sync off, or the active view excluded. The recorder hears only what that agent
/// captures, and the agent captures only while sync routes its events.
pub fn uncaptured(
    switches: SyncSwitches,
    excluded: bool,
    has_leader: bool,
) -> Option<&'static str> {
    if !has_leader {
        Some("no view is active to record from")
    } else if !switches.enabled {
        Some(
            "sync is off: the recorder records what the active view's sync agent captures - turn sync on first",
        )
    } else if excluded {
        Some(
            "the active view is excluded from sync: the recorder records what its sync agent captures - include it first",
        )
    } else {
        None
    }
}

/// The recorder's hover capture for `automation.recorder.recordHover` (RB-090-81). A function,
/// not a `From`: neither type is this crate's, and neither crate depends on the other.
pub fn hover_capture(choice: RecordHover) -> HoverCapture {
    match choice {
        RecordHover::Menu => HoverCapture::Menu,
        RecordHover::All => HoverCapture::All,
        RecordHover::Off => HoverCapture::Off,
    }
}

/// The kinds of steps whose capture is off by their sync switch: clicks, input, keys and form
/// controls always, hover and scroll when the recorder records them.
pub fn missing_kinds(
    switches: SyncSwitches,
    record_hover: bool,
    record_scroll: bool,
) -> Vec<&'static str> {
    [
        ("click", switches.click),
        ("input", switches.input),
        ("keyboard", switches.keyboard),
        ("forms", switches.forms),
        ("hover", switches.hover || !record_hover),
        ("scroll", switches.scroll || !record_scroll),
    ]
    .into_iter()
    .filter(|(_, on)| !on)
    .map(|(kind, _)| kind)
    .collect()
}

/// The project's recordings folder (`automation.recorder.dir`).
pub(super) fn store_of(host: &Host) -> Result<RecordingStore, RecorderFailure> {
    let root = host.project_root().ok_or(RecorderFailure::NoProject)?;
    let protected: Vec<&str> = rb_config::trust::PROTECTED_DIRS
        .iter()
        .map(|(_, name)| *name)
        .collect();
    let dir = host.config().automation.recorder.dir;
    Ok(RecordingStore::open(&root, &dir, &protected)?)
}

/// The views with their breakpoints (a free-size view by its size).
fn recording_views(host: &Host) -> Vec<RecordingView> {
    host.views()
        .into_iter()
        .map(|info| RecordingView {
            view: info.id,
            breakpoint: RecordedBreakpoint {
                id: info
                    .breakpoint
                    .clone()
                    .unwrap_or_else(|| format!("{}x{}", info.width, info.height)),
                width: info.width,
                height: info.height,
                device_scale_factor: info.dpr,
            },
        })
        .collect()
}

/// `path` relative to the project root, with `/`.
fn relative(host: &Host, path: &Path) -> String {
    let shown = host
        .project_root()
        .and_then(|root| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf());
    shown.to_string_lossy().replace('\\', "/")
}

/// Now, ms since the epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// `recording-YYYYMMDD-HHMMSS` in local time (RB-090-69).
pub fn default_name(now_ms: u64) -> String {
    name_at(now_ms, crate::local_time::offset_at(now_ms / 1000))
}

/// `recording-YYYYMMDD-HHMMSS` of `now_ms` in the zone `offset_secs` east of UTC.
fn name_at(now_ms: u64, offset_secs: i32) -> String {
    let stamp = rb_visual::Stamp::from_unix_with_offset(now_ms / 1000, offset_secs);
    format!(
        "recording-{}-{}",
        stamp.day().replace('-', ""),
        stamp.time().replace('-', "")
    )
}

#[cfg(test)]
#[path = "glue_tests.rs"]
mod tests;
