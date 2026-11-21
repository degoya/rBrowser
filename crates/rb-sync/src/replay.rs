//! The hub's interface to the engines: [`Replay`] replays one captured event into one view.
//! RB-020-03 implements it over CDP (`Input.*` after the agent's `resolve`), Firefox and WebKit
//! follow with RB-040-*; until then the app runs [`NoReplay`]. [`RecordingReplay`] is the test
//! double: it records every request and answers what a test set for the view.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_engine::ViewId;

use crate::event::SyncEvent;

/// One event for one view. The event and the frame path are shared between the requests of
/// one routing (RB-080-27): an event to eight views is cloned once, the requests, their queued
/// items and the replays hold the same `Arc`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayRequest {
    /// The view that captured the event (the leader).
    pub source: ViewId,
    /// The view to replay it into.
    pub target: ViewId,
    /// Whether the event came from the source's top frame.
    pub top: bool,
    /// The iframes' CSS paths from the top document down, when the agent knew them
    /// (`EventMessage::frame`).
    pub frame: Option<Arc<[String]>>,
    /// The event.
    pub event: Arc<SyncEvent>,
    /// Leader-only submit: the view must not submit a form because of this input (a replayed
    /// click on a submit button, Enter in a field) - the leader submits, the view follows by
    /// URL. The implementation blocks the submit in the view, not the input.
    pub block_submit: bool,
}

/// How a replay went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayOutcome {
    /// Replayed.
    Done,
    /// The target is missing in the view: it is desynced.
    NotFound,
    /// The target exists but is not rendered: desynced.
    NotVisible,
    /// No answer in time: desynced.
    Timeout,
    /// The engine cannot replay this event (yet): skipped, the view stays in sync.
    Unsupported,
    /// The replay failed; the message is logged, never shown.
    Failed(String),
}

/// Replays events into views. Called once per event and target view, in capture order per
/// view; a call may take until the hub's timeout (`sync.desyncTimeoutMs`) before it counts as
/// [`ReplayOutcome::Timeout`].
#[async_trait]
pub trait Replay: Send + Sync {
    /// Replays `request.event` into `request.target`.
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome;

    /// The timing counter of a replay into `view` (`rb_util::perf::names`, RB-090-32): the
    /// engine's when the replay knows it.
    fn counter(&self, view: ViewId) -> &'static str {
        let _ = view;
        rb_util::perf::names::SYNC_REPLAY
    }
}

/// The replay until RB-020-03: every event is [`ReplayOutcome::Unsupported`], so no view is
/// desynced by a replay that does not exist.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoReplay;

#[async_trait]
impl Replay for NoReplay {
    async fn dispatch(&self, _request: ReplayRequest) -> ReplayOutcome {
        ReplayOutcome::Unsupported
    }
}

/// The test double: records every request; answers the outcome set for the view
/// ([`ReplayOutcome::Done`] by default), or never for a stalled view.
#[derive(Debug, Default)]
pub struct RecordingReplay {
    calls: Mutex<Vec<ReplayRequest>>,
    outcomes: Mutex<HashMap<ViewId, ReplayOutcome>>,
    stalled: Mutex<HashSet<ViewId>>,
}

impl RecordingReplay {
    /// `view` answers `outcome` from now on.
    pub fn answer(&self, view: ViewId, outcome: ReplayOutcome) {
        lock(&self.outcomes).insert(view, outcome);
    }

    /// `view` never answers (a hung renderer).
    pub fn stall(&self, view: ViewId) {
        lock(&self.stalled).insert(view);
    }

    /// Every request so far, in call order.
    pub fn calls(&self) -> Vec<ReplayRequest> {
        lock(&self.calls).clone()
    }

    /// The requests for `view`, in call order.
    pub fn calls_for(&self, view: ViewId) -> Vec<ReplayRequest> {
        lock(&self.calls)
            .iter()
            .filter(|request| request.target == view)
            .cloned()
            .collect()
    }
}

#[async_trait]
impl Replay for RecordingReplay {
    async fn dispatch(&self, request: ReplayRequest) -> ReplayOutcome {
        let view = request.target;
        lock(&self.calls).push(request);
        if lock(&self.stalled).contains(&view) {
            std::future::pending::<()>().await;
        }
        lock(&self.outcomes)
            .get(&view)
            .cloned()
            .unwrap_or(ReplayOutcome::Done)
    }
}

pub(crate) use rb_util::lock;

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
