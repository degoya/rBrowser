//! The progress of a check run as the findings panel shows it (RB-080-89): every plugin x view
//! job's state, how long the run has gone, the estimate from the last run. Pushed as
//! `checksProgress` while the run goes (throttled) and once when it ends; `getChecksProgress`
//! answers the last one.

use std::collections::BTreeMap;

use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

/// Where one job is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckJobState {
    /// Waits for a free slot.
    Queued,
    /// Runs now.
    Running,
    /// Answered.
    Done,
    /// Failed (`code`).
    Failed,
    /// Did not run (`reason`).
    Skipped,
}

/// One plugin over one view; no `view`: the plugin as a whole (it failed to describe its rules
/// or runs none) or a run without views.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckJob {
    /// The plugin id.
    pub plugin: String,
    /// The view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    /// The view's breakpoint, for its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoint: Option<String>,
    /// Where it is.
    pub state: CheckJobState,
    /// The findings it kept (`done`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub findings: Option<u32>,
    /// The failure's i18n key (`failed`), or the skip reason's (`skipped`,
    /// `checks.skip.cancelled`, `checks.skip.rules_off`, `checks.skip.engine`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The reason key's parameters (`{service}`, `{engine}`, RB-080-98).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reason_params: BTreeMap<String, String>,
    /// How long it ran, once it answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

/// A check run's progress.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksProgress {
    /// The run's id.
    pub run_id: String,
    /// How long the run has gone, as of this message.
    pub elapsed_ms: u64,
    /// How long the whole run takes, from the last run's timings; absent while a plugin has
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_ms: Option<u64>,
    /// Every job, in plan order.
    pub jobs: Vec<CheckJob>,
    /// The run is over.
    #[serde(default)]
    pub finished: bool,
    /// The run was cancelled.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cancelled: bool,
}

impl ChecksProgress {
    /// The jobs through (done, failed or skipped) and all of them.
    pub fn counts(&self) -> (usize, usize) {
        let ended = self
            .jobs
            .iter()
            .filter(|job| !matches!(job.state, CheckJobState::Queued | CheckJobState::Running))
            .count();
        (ended, self.jobs.len())
    }
}

#[cfg(test)]
#[path = "checks_tests.rs"]
mod tests;
