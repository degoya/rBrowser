//! A check run's progress on its way to the findings panel (RB-080-89): the runner's job table
//! as `checksProgress` (throttled to one message per [`THROTTLE`], the last change always sent),
//! the findings so far as a partial `findings` whenever a plugin is through, the run's cancel
//! (`checks.cancel`), the status bar's start and end, and the plugins' timings of the last run
//! for the next run's estimate. Every message also stays for a panel that opens later
//! (`getChecksProgress`, `getFindings`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rb_ipc::{
    CheckJob, CheckJobState, ChecksProgress, CoreToShell, Notification, NotificationLevel,
    ProjectOff, ViewId,
};
use rb_plugin_host::{
    Cancel, JobProgress, JobState, PluginTiming, ProgressSink, RunOutcome, SkipReason, timings_of,
};
use rb_plugin_types::FindingsFile;

use super::{LAST, run};
use crate::cdp::lock;
use crate::control::Core;

/// The most often the panel hears of a run going on.
pub const THROTTLE: Duration = Duration::from_millis(250);
/// The i18n key of the status bar's notice when a run starts; `{plugins}`, `{views}`.
pub const STARTED_KEY: &str = "checks.notice.started";
/// The notice when a run ends; `{count}`, `{seconds}`.
pub const FINISHED_KEY: &str = "checks.notice.finished";
/// The notice when a run was cancelled; `{count}`, `{seconds}`.
pub const CANCELLED_KEY: &str = "checks.notice.cancelled";
/// The skip reason of a job the cancel stopped.
pub const SKIP_CANCELLED_KEY: &str = "checks.skip.cancelled";
/// The skip reason of a plugin whose rules are all off.
pub const SKIP_RULES_OFF_KEY: &str = "checks.skip.rules_off";
/// The skip reason of a view whose engine the plugin cannot serve; `{engine}` (RB-080-98).
pub const SKIP_ENGINE_KEY: &str = "checks.skip.engine";

/// The cancel of the run in progress.
static CURRENT: Mutex<Option<Arc<Cancel>>> = Mutex::new(None);
/// The progress of the run in progress or the last one.
static LAST_PROGRESS: Mutex<Option<ChecksProgress>> = Mutex::new(None);
/// What each plugin took in the last run it got through.
static TIMINGS: Mutex<BTreeMap<String, PluginTiming>> = Mutex::new(BTreeMap::new());

/// Cancels the run in progress; `false` when none is.
pub fn cancel_current() -> bool {
    lock(&CURRENT)
        .as_ref()
        .is_some_and(|cancel| cancel.cancel())
}

/// The progress the panel was told last; `None` before the first run.
pub fn last_progress() -> Option<ChecksProgress> {
    lock(&LAST_PROGRESS).clone()
}

/// The timings the next run's estimate is made of.
pub fn timings() -> BTreeMap<String, PluginTiming> {
    lock(&TIMINGS).clone()
}

/// Keeps the timings of the plugins `outcome` got through, beside the others'.
pub fn remember_timings(outcome: &RunOutcome) {
    lock(&TIMINGS).extend(timings_of(&outcome.jobs));
}

/// The runner's jobs as the panel reads them; `breakpoints` names the views.
pub fn ipc_jobs(jobs: &[JobProgress], breakpoints: &BTreeMap<u32, String>) -> Vec<CheckJob> {
    jobs.iter()
        .map(|job| {
            let (state, findings, reason, reason_params) = match &job.state {
                JobState::Queued => (CheckJobState::Queued, None, None, BTreeMap::new()),
                JobState::Running => (CheckJobState::Running, None, None, BTreeMap::new()),
                JobState::Done { findings } => (
                    CheckJobState::Done,
                    Some(u32::try_from(*findings).unwrap_or(u32::MAX)),
                    None,
                    BTreeMap::new(),
                ),
                JobState::Failed { code, params } => {
                    (CheckJobState::Failed, None, code.clone(), params.clone())
                }
                JobState::Skipped { reason, engine } => (
                    CheckJobState::Skipped,
                    None,
                    Some(skip_key(*reason).to_owned()),
                    engine
                        .iter()
                        .map(|engine| ("engine".to_owned(), engine_label(engine)))
                        .collect(),
                ),
            };
            CheckJob {
                plugin: job.plugin.clone(),
                view: job.view.map(ViewId),
                breakpoint: job.view.and_then(|view| breakpoints.get(&view).cloned()),
                state,
                findings,
                reason,
                reason_params,
                elapsed_ms: job.elapsed_ms,
            }
        })
        .collect()
}

fn skip_key(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::Cancelled => SKIP_CANCELLED_KEY,
        SkipReason::RulesOff => SKIP_RULES_OFF_KEY,
        SkipReason::Engine => SKIP_ENGINE_KEY,
    }
}

/// An engine's name for people (`webkit` -> `WebKit`); a name rBrowser does not know as given.
fn engine_label(engine: &str) -> String {
    serde_json::from_value::<rb_config::EngineKind>(serde_json::Value::from(engine)).map_or_else(
        |_| engine.to_owned(),
        |kind| rb_engine::engine_name(kind).to_owned(),
    )
}

/// A duration for the status bar: tenths below ten seconds, whole seconds above.
pub fn seconds_text(elapsed: Duration) -> String {
    let tenths = elapsed.as_millis().div_ceil(100);
    if tenths < 100 {
        format!("{}.{}", tenths / 10, tenths % 10)
    } else {
        tenths.div_ceil(10).to_string()
    }
}

/// What to do with a change of the table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Offer {
    /// Send it now.
    Now,
    /// Send what is newest after this wait (no send is waiting yet).
    Later(Duration),
    /// A send is waiting already; it takes this change along.
    Skip,
}

/// One message per [`THROTTLE`] at most, the last change never lost.
#[derive(Debug, Default)]
pub struct Throttle {
    sent: Option<Instant>,
    waiting: bool,
}

impl Throttle {
    /// A change at `now`.
    pub fn offer(&mut self, now: Instant) -> Offer {
        if self.waiting {
            return Offer::Skip;
        }
        match self.sent {
            Some(sent) if now.duration_since(sent) < THROTTLE => {
                self.waiting = true;
                Offer::Later(THROTTLE - now.duration_since(sent))
            }
            _ => {
                self.sent = Some(now);
                Offer::Now
            }
        }
    }

    /// The waiting send went out at `now`.
    pub fn sent(&mut self, now: Instant) {
        self.sent = Some(now);
        self.waiting = false;
    }
}

/// The panel's side of a run: what it is told, and how often.
pub struct PanelSink {
    core: Arc<Core>,
    run_id: String,
    started: Instant,
    estimate_ms: Option<u64>,
    breakpoints: BTreeMap<u32, String>,
    project_off: ProjectOff,
    state: Mutex<SinkState>,
}

#[derive(Default)]
struct SinkState {
    throttle: Throttle,
    latest: Vec<CheckJob>,
    /// The run is over: a send still waiting must not undo its last word.
    closed: bool,
}

impl PanelSink {
    /// A sink for run `run_id` started at `started`, its views named by `breakpoints`; the
    /// cancel of the run becomes the one `checks.cancel` reaches.
    pub fn start(
        core: Arc<Core>,
        run_id: String,
        breakpoints: BTreeMap<u32, String>,
        estimate_ms: Option<u64>,
        project_off: ProjectOff,
        cancel: Arc<Cancel>,
    ) -> Arc<Self> {
        *lock(&CURRENT) = Some(cancel);
        Arc::new(Self {
            core,
            run_id,
            started: Instant::now(),
            estimate_ms,
            breakpoints,
            project_off,
            state: Mutex::new(SinkState::default()),
        })
    }

    fn progress(&self, jobs: Vec<CheckJob>, finished: bool, cancelled: bool) -> ChecksProgress {
        ChecksProgress {
            run_id: self.run_id.clone(),
            elapsed_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
            estimate_ms: self.estimate_ms,
            jobs,
            finished,
            cancelled,
        }
    }

    /// Sends `progress` and keeps it - unless the run is over already: a send that passed its
    /// check before the end (the throttle's late one) never undoes the finished progress. The
    /// check, the send and the keeping are one step under the lock.
    fn send(&self, progress: ChecksProgress) {
        let mut last = lock(&LAST_PROGRESS);
        let over = last
            .as_ref()
            .is_some_and(|last| last.run_id == progress.run_id && last.finished);
        if over && !progress.finished {
            return;
        }
        self.core
            .send(&CoreToShell::ChecksProgress(progress.clone()));
        *last = Some(progress);
    }

    /// Sends the newest table unless the run is over (the waiting send of the throttle).
    fn flush(&self) {
        let jobs = {
            let mut state = lock(&self.state);
            if state.closed {
                return;
            }
            state.throttle.sent(Instant::now());
            state.latest.clone()
        };
        self.send(self.progress(jobs, false, false));
    }

    /// The run ended without an outcome: nothing more is sent, nothing more is cancelled.
    pub fn close(&self) {
        lock(&self.state).closed = true;
        *lock(&CURRENT) = None;
    }

    /// The run is over: its last progress, at once.
    pub fn finish(&self, outcome: &RunOutcome) {
        self.close();
        let jobs = ipc_jobs(&outcome.jobs, &self.breakpoints);
        self.send(self.progress(jobs, true, outcome.cancelled));
    }

    fn partial(&self, file: &FindingsFile) {
        match serde_json::to_value(file) {
            Ok(value) => {
                let message = CoreToShell::Findings {
                    run: value,
                    messages: run::messages(file, rb_i18n::plugin_texts),
                    project_off: self.project_off.clone(),
                    partial: true,
                };
                self.core.send(&message);
                *lock(&LAST) = Some(message);
            }
            Err(error) => tracing::warn!(%error, "the findings so far cannot be shown"),
        }
    }
}

/// The sink as the runner holds it.
pub struct Shared(pub Arc<PanelSink>);

impl ProgressSink for Shared {
    fn update(&self, jobs: &[JobProgress], partial: Option<&FindingsFile>) {
        let sink = &self.0;
        let table = ipc_jobs(jobs, &sink.breakpoints);
        if let Some(file) = partial {
            sink.partial(file);
        }
        let offer = {
            let mut state = lock(&sink.state);
            if state.closed {
                return;
            }
            state.latest = table.clone();
            // A plugin through is news worth more than the throttle.
            if partial.is_some() {
                state.throttle.sent(Instant::now());
                Offer::Now
            } else {
                state.throttle.offer(Instant::now())
            }
        };
        match offer {
            Offer::Now => sink.send(sink.progress(table, false, false)),
            Offer::Later(wait) => {
                let later = Arc::clone(sink);
                sink.core.runtime().spawn(async move {
                    tokio::time::sleep(wait).await;
                    later.flush();
                });
            }
            Offer::Skip => {}
        }
    }
}

/// The status bar's notice of `key` with `args`.
pub fn notice(core: &Core, level: NotificationLevel, key: &str, args: &[(&str, String)]) {
    core.notify(CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: None,
    }));
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
