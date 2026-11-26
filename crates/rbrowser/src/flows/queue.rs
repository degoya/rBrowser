//! `flows.run_all`'s queue: every flow of the project, one after the other. `flows.cancel` ends
//! the flow running and the queue with it - no later flow starts. The flows still waiting are
//! part of the run state ([`waiting`]): without them several flows - exports of one recording,
//! say - one after the other read as one flow running in a loop (RB-090-71). When the queue ends
//! it says so once: how many flows ran, passed and failed, and which failed ([`Tally`],
//! RB-090-84).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rb_engine::ViewId;
use rb_ipc::{CoreToShell, NotificationLevel};
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::app::{Started, notice, notification, setup, start};
use super::result::RunResult;
use super::{FlowError, catalog};
use crate::cdp::lock;
use crate::control::Core;

/// The notice of a `flows.run_all` that ended with every flow passed; `{ran}`, `{count}`,
/// `{passed}`, `{failed}`.
pub const ALL_DONE: &str = "flows.notice.all_done";
/// The same with flows that failed, named in `{flows}`.
pub const ALL_FAILED: &str = "flows.notice.all_failed";

/// How many times `flows.cancel` was asked for; a queue stops once it moved.
static CANCELS: AtomicU64 = AtomicU64::new(0);

/// The flows of the queue going on that have not started yet, in order.
static WAITING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Notes a cancel: every queue going on stops before its next flow, and none waits any more.
pub fn note_cancel() {
    CANCELS.fetch_add(1, Ordering::SeqCst);
    lock(&WAITING).clear();
}

/// The flows of a queue that wait for the one running now.
pub fn set_waiting(names: Vec<String>) {
    *lock(&WAITING) = names;
}

/// The flows still waiting, in order.
pub fn waiting() -> Vec<String> {
    lock(&WAITING).clone()
}

/// `name` starts: it waits no more.
fn started(name: &str) {
    let mut waiting = lock(&WAITING);
    if let Some(at) = waiting.iter().position(|waiting| waiting == name) {
        waiting.remove(at);
    }
}

/// The cancels so far; a queue started now stops when [`cancelled_since`] says so.
pub fn cancels() -> u64 {
    CANCELS.load(Ordering::SeqCst)
}

/// Whether a cancel came after `since` ([`cancels`]).
pub fn cancelled_since(since: u64) -> bool {
    cancels() != since
}

/// Why a queue ended before its last flow: the flow that did not start, why, and every flow
/// that did not run - that one and the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// The flow that did not start.
    pub flow: String,
    /// Why.
    pub error: FlowError,
    /// The flows that did not run, in order.
    pub not_run: Vec<String>,
}

impl Refused {
    /// The arguments of [`notice::QUEUE_STOPPED`]: `flow`, `error`, `flows`.
    pub fn notice_args(&self) -> [(&'static str, String); 3] {
        [
            ("flow", self.flow.clone()),
            ("error", rb_diag::redact(&self.error.to_string())),
            ("flows", self.not_run.join(", ")),
        ]
    }
}

/// How the flows of a `flows.run_all` ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// The flows that ran.
    pub ran: usize,
    /// Of them, the ones that passed.
    pub passed: usize,
    /// The ones that failed or broke off, in order.
    pub failed: Vec<String>,
}

impl Tally {
    /// Counts the flow `name` with its `outcome` (none: the run ended without one).
    pub fn count(&mut self, name: &str, outcome: Option<&Result<RunResult, FlowError>>) {
        self.ran += 1;
        match outcome {
            Some(Ok(result)) if result.passed => self.passed += 1,
            _ => self.failed.push(name.to_owned()),
        }
    }

    /// The notice of the queue's end, of `count` flows.
    pub fn notice(&self, count: usize) -> CoreToShell {
        let (level, key) = if self.failed.is_empty() {
            (NotificationLevel::Info, ALL_DONE)
        } else {
            (NotificationLevel::Error, ALL_FAILED)
        };
        notification(
            level,
            key,
            &[
                ("ran", self.ran.to_string()),
                ("count", count.to_string()),
                ("passed", self.passed.to_string()),
                ("failed", self.failed.len().to_string()),
                ("flows", self.failed.join(", ")),
            ],
        )
    }
}

/// `started`, its outcome counted into `tally` as it arrives and passed on unchanged.
fn tallied(core: &Core, name: &str, started: Started, tally: &Arc<Mutex<Tally>>) -> Started {
    let (sender, done) = oneshot::channel();
    let (tally, name) = (Arc::clone(tally), name.to_owned());
    core.runtime().spawn(async move {
        let outcome = started.done.await.ok();
        lock(&tally).count(&name, outcome.as_ref());
        if let Some(outcome) = outcome {
            let _ = sender.send(outcome);
        }
    });
    Started {
        run_id: started.run_id,
        done,
    }
}

/// Starts each flow of `queue` with `start` once the one before ended, taking it off the
/// [`waiting`] flows first; stops at a cancel (`cancelled`, asked before each start and after
/// each run) or at a flow that does not start, which it answers ([`Refused`]). None waits once
/// it returns.
pub async fn run_queue<C, S>(queue: Vec<String>, cancelled: C, mut start: S) -> Option<Refused>
where
    C: Fn() -> bool,
    S: FnMut(&str) -> Result<Started, FlowError>,
{
    let mut refused = None;
    for (at, name) in queue.iter().enumerate() {
        if cancelled() {
            tracing::info!(flow = %name, "flows.run_all cancelled");
            break;
        }
        started(name);
        match start(name) {
            Ok(next) => {
                let _ = next.done.await;
            }
            Err(error) => {
                tracing::warn!(%error, flow = %name, "flows.run_all stopped");
                refused = Some(Refused {
                    flow: name.clone(),
                    error,
                    not_run: queue[at..].to_vec(),
                });
                break;
            }
        }
    }
    lock(&WAITING).clear();
    refused
}

/// `flows.run_all`: every flow, one after the other, in its own views - in `views` when the run
/// names them (a scope, RB-099-10). A flow that does not start ends the queue with a notice
/// naming why and the flows that did not run.
pub(super) fn run_all(core: &Arc<Core>, views: Option<Vec<ViewId>>) -> Result<Value, FlowError> {
    let setup = setup(core)?;
    let names: Vec<String> = catalog::list(&setup.root, &setup.settings.dir)?
        .into_iter()
        .map(|flow| flow.name)
        .collect();
    let Some((first, rest)) = names.split_first() else {
        return Err(FlowError::NoName(Vec::new()));
    };
    let since = cancels();
    let tally = Arc::new(Mutex::new(Tally::default()));
    // One layout for the whole batch (`hiddenViews: layout`), the one before at its end.
    super::run_layout::batch_begin();
    let first_run = start(core, first, views.clone(), None).inspect_err(|_| {
        super::run_layout::batch_end(core);
    })?;
    let started = tallied(core, first, first_run, &tally);
    let rest = rest.to_vec();
    set_waiting(rest.clone());
    // The shell reads the run again: it names the flows waiting now.
    core.send(&CoreToShell::FlowRunChanged);
    let core = Arc::clone(core);
    let count = names.len();
    core.runtime().clone().spawn(async move {
        let _ = started.done.await;
        let cancelled = move || cancelled_since(since);
        let next = |name: &str| {
            start(&core, name, views.clone(), None)
                .map(|started| tallied(&core, name, started, &tally))
        };
        if let Some(refused) = run_queue(rest, cancelled, next).await {
            let args = refused.notice_args();
            core.notify(notification(
                NotificationLevel::Error,
                notice::QUEUE_STOPPED,
                &args,
            ));
            core.send(&CoreToShell::FlowRunChanged);
        } else {
            // Once, when the queue ended or was cancelled (RB-090-84).
            let end = lock(&tally).notice(count);
            core.notify(end);
        }
        super::run_layout::batch_end(&core);
    });
    Ok(json!({ "flows": names }))
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
