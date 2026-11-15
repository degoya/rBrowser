//! The progress of one check run (RB-080-89): the state of every plugin x view job, told to a
//! [`ProgressSink`] as it changes, the findings so far each time a plugin is through, a
//! [`Cancel`] that stops the run with what it found, and the estimate of a run's length from the
//! last run's timings.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rb_plugin_types::{Failure, FindingsFile, MessageParams, ViewId, codes};
use serde::Serialize;
use tokio::sync::Notify;

/// Why a job did not run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// The run was cancelled before or while it ran.
    Cancelled,
    /// Every rule of the plugin is switched off.
    RulesOff,
    /// The view's engine cannot serve the plugin: the host answered its page service with
    /// `plugins.error.not_available` naming the engine (RB-080-98).
    Engine,
}

/// Where one plugin x view job is.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum JobState {
    /// Waits for a free slot (`checks.parallel`).
    Queued,
    /// Runs now.
    Running,
    /// Answered; `findings` kept after the rule overrides and ignore patterns.
    Done { findings: usize },
    /// Failed; `code` is the failure's i18n key when it has one, `params` its parameters.
    Failed {
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        #[serde(skip_serializing_if = "MessageParams::is_empty")]
        params: MessageParams,
    },
    /// Did not run; `engine` names the view's engine of a [`SkipReason::Engine`].
    Skipped {
        reason: SkipReason,
        #[serde(skip_serializing_if = "Option::is_none")]
        engine: Option<String>,
    },
}

impl JobState {
    /// `failure` as the end of the job in `view`: skipped when it says `view`'s own engine
    /// cannot serve the plugin ([`Failure::engine_unserved`] naming `view`), else failed with its
    /// code and parameters. A refusal of another view, or in a job without a view, is a failure.
    #[must_use]
    pub fn ended_by(failure: &Failure, view: Option<ViewId>) -> Self {
        match view.and_then(|view| failure.unserved_engine_in(view)) {
            Some(engine) => Self::skipped_for(engine),
            None => Self::Failed {
                code: failure.code().map(str::to_owned),
                params: failure.params.clone(),
            },
        }
    }

    /// Failed in the host (a panic): `plugins.error.host_failed`.
    #[must_use]
    pub fn host_failed() -> Self {
        Self::Failed {
            code: Some(codes::HOST_FAILED.to_owned()),
            params: MessageParams::new(),
        }
    }

    /// Skipped for `reason`, which names no engine.
    #[must_use]
    pub fn skipped(reason: SkipReason) -> Self {
        Self::Skipped {
            reason,
            engine: None,
        }
    }

    fn skipped_for(engine: &str) -> Self {
        Self::Skipped {
            reason: SkipReason::Engine,
            engine: Some(engine.to_owned()),
        }
    }

    /// Whether the job is through (done, failed or skipped).
    #[must_use]
    pub fn ended(&self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

/// One plugin over one view (`view` absent: the plugin as a whole, or a run without views).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub plugin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    #[serde(flatten)]
    pub state: JobState,
    /// How long it ran, once it answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

impl JobProgress {
    pub(crate) fn new(plugin: &str, view: Option<ViewId>, state: JobState) -> Self {
        Self {
            plugin: plugin.to_owned(),
            view,
            state,
            elapsed_ms: None,
        }
    }
}

/// Told about a run while it goes. Called on the runner's task: it must not block.
pub trait ProgressSink: Send + Sync {
    /// The jobs changed; `partial` is the run so far when a plugin just got through all its
    /// views (its findings are final from now on).
    fn update(&self, jobs: &[JobProgress], partial: Option<&FindingsFile>);
}

/// Stops a run: jobs not yet started are skipped, running ones are dropped; what the finished
/// ones found stays.
#[derive(Debug, Default)]
pub struct Cancel {
    flag: AtomicBool,
    notify: Notify,
}

impl Cancel {
    /// Cancels the run; a second call changes nothing. `true` when this call cancelled it.
    pub fn cancel(&self) -> bool {
        let first = !self.flag.swap(true, Ordering::AcqRel);
        self.notify.notify_waiters();
        first
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    /// Resolves once the run is cancelled.
    pub async fn cancelled(&self) {
        loop {
            let notified = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// What a run is told and asked while it goes.
#[derive(Clone, Default)]
pub struct RunControl {
    pub cancel: Arc<Cancel>,
    pub sink: Option<Arc<dyn ProgressSink>>,
}

/// A run with how each job ended.
#[derive(Clone, Debug)]
pub struct RunOutcome {
    pub file: FindingsFile,
    pub jobs: Vec<JobProgress>,
    pub cancelled: bool,
}

/// What a plugin took in a run: its jobs' time together and its longest job, in ms.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PluginTiming {
    pub work_ms: u64,
    pub longest_ms: u64,
}

/// The timings of the plugins every job of which answered (done or failed) in `jobs` - a plugin
/// that ran nothing (all rules off, its rules not described) at no time; a plugin with a
/// cancelled or unfinished job is left out, so a cancelled run teaches nothing wrong.
#[must_use]
pub fn timings_of(jobs: &[JobProgress]) -> BTreeMap<String, PluginTiming> {
    let mut timings: BTreeMap<String, Option<PluginTiming>> = BTreeMap::new();
    for job in jobs {
        let entry = timings
            .entry(job.plugin.clone())
            .or_insert(Some(PluginTiming::default()));
        // A view skipped for its engine answered too: the plugin ran and asked (RB-080-98).
        let answered = matches!(
            job.state,
            JobState::Done { .. }
                | JobState::Failed { .. }
                | JobState::Skipped {
                    reason: SkipReason::Engine,
                    ..
                }
        );
        // A plugin that ran nothing - all rules off, or it did not describe them - took no time.
        let ran_nothing = job.elapsed_ms.is_none()
            && matches!(
                job.state,
                JobState::Skipped {
                    reason: SkipReason::RulesOff,
                    ..
                } | JobState::Failed { .. }
            );
        match (entry.as_mut(), job.elapsed_ms) {
            (Some(timing), Some(ms)) if answered => {
                timing.work_ms = timing.work_ms.saturating_add(ms);
                timing.longest_ms = timing.longest_ms.max(ms);
            }
            (Some(_), None) if ran_nothing => {}
            _ => *entry = None,
        }
    }
    timings
        .into_iter()
        .filter_map(|(plugin, timing)| timing.map(|timing| (plugin, timing)))
        .collect()
}

/// How long a run of `plugins` takes with `parallel` jobs at once, from `last` - their timings
/// in the last run: their work spread over the slots, never less than the longest job. `None`
/// when a plugin has no timing yet (no estimate rather than a wrong one) or nothing runs.
#[must_use]
pub fn estimate_ms(
    last: &BTreeMap<String, PluginTiming>,
    plugins: &[String],
    parallel: usize,
) -> Option<u64> {
    if plugins.is_empty() {
        return None;
    }
    let mut work: u64 = 0;
    let mut longest: u64 = 0;
    for plugin in plugins {
        let timing = last.get(plugin)?;
        work = work.saturating_add(timing.work_ms);
        longest = longest.max(timing.longest_ms);
    }
    let slots = u64::try_from(parallel.max(1)).unwrap_or(u64::MAX);
    Some(work.div_ceil(slots).max(longest))
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
