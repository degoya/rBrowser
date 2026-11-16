//! `flows.run_all`'s queue: every flow of the project, one after the other. `flows.cancel` ends
//! the flow running and the queue with it - no later flow starts.

use std::sync::atomic::{AtomicU64, Ordering};

use super::FlowError;
use super::app::Started;

/// How many times `flows.cancel` was asked for; a queue stops once it moved.
static CANCELS: AtomicU64 = AtomicU64::new(0);

/// Notes a cancel: every queue going on stops before its next flow.
pub fn note_cancel() {
    CANCELS.fetch_add(1, Ordering::SeqCst);
}

/// The cancels so far; a queue started now stops when [`cancelled_since`] says so.
pub fn cancels() -> u64 {
    CANCELS.load(Ordering::SeqCst)
}

/// Whether a cancel came after `since` ([`cancels`]).
pub fn cancelled_since(since: u64) -> bool {
    cancels() != since
}

/// Starts each flow of `queue` with `start` once the one before ended; stops at a cancel
/// (`cancelled`, asked before each start and after each run) or a flow that does not start.
pub async fn run_queue<C, S>(queue: Vec<String>, cancelled: C, mut start: S)
where
    C: Fn() -> bool,
    S: FnMut(&str) -> Result<Started, FlowError>,
{
    for name in queue {
        if cancelled() {
            tracing::info!(flow = %name, "flows.run_all cancelled");
            break;
        }
        match start(&name) {
            Ok(next) => {
                let _ = next.done.await;
            }
            Err(error) => {
                tracing::warn!(%error, flow = %name, "flows.run_all stopped");
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
