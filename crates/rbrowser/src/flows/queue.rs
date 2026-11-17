//! `flows.run_all`'s queue: every flow of the project, one after the other. `flows.cancel` ends
//! the flow running and the queue with it - no later flow starts. The flows still waiting are
//! part of the run state ([`waiting`]): without them several flows - exports of one recording,
//! say - one after the other read as one flow running in a loop (RB-090-71).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rb_ipc::{CoreToShell, NotificationLevel};
use serde_json::{Value, json};

use super::app::{Started, notice, notification, setup, start};
use super::{FlowError, catalog};
use crate::cdp::lock;
use crate::control::Core;

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

/// `flows.run_all`: every flow, one after the other, in its own views. A flow that does not
/// start ends the queue with a notice naming why and the flows that did not run.
pub(super) fn run_all(core: &Arc<Core>) -> Result<Value, FlowError> {
    let setup = setup(core)?;
    let names: Vec<String> = catalog::list(&setup.root, &setup.settings.dir)?
        .into_iter()
        .map(|flow| flow.name)
        .collect();
    let Some((first, rest)) = names.split_first() else {
        return Err(FlowError::NoName(Vec::new()));
    };
    let since = cancels();
    let started = start(core, first, None, None)?;
    let rest = rest.to_vec();
    set_waiting(rest.clone());
    // The shell reads the run again: it names the flows waiting now.
    core.send(&CoreToShell::FlowRunChanged);
    let core = Arc::clone(core);
    core.runtime().clone().spawn(async move {
        let _ = started.done.await;
        let cancelled = move || cancelled_since(since);
        if let Some(refused) =
            run_queue(rest, cancelled, |name| start(&core, name, None, None)).await
        {
            let args = refused.notice_args();
            core.notify(notification(
                NotificationLevel::Error,
                notice::QUEUE_STOPPED,
                &args,
            ));
            core.send(&CoreToShell::FlowRunChanged);
        }
    });
    Ok(json!({ "flows": names }))
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
