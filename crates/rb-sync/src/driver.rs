//! [`SyncHub`]: the [`Hub`] on tokio. Each target view has a queue of its own and a task that
//! replays its events one after the other, so the order per source and view holds and a slow or
//! hung view holds up nobody else. A replay that does not answer within the timeout
//! (`sync.desyncTimeoutMs`) counts as [`ReplayOutcome::Timeout`]; a view more than
//! [`QUEUE_DEPTH`] events behind is desynced the same way. A request whose view has become the
//! active one by its turn is dropped (the leader's own input is the original), and an outcome
//! of a document the view has left since the routing counts for nothing ([`Hub::record_at`]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use rb_engine::ViewId;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::event::SyncEvent;
use crate::hub::{Hub, Route};
use crate::message::EventMessage;
use crate::replay::{Replay, ReplayOutcome, ReplayRequest, lock};

/// Events a view may fall behind before it is desynced.
pub const QUEUE_DEPTH: usize = 256;
/// The default of `sync.desyncTimeoutMs`.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(750);

/// Called after the status changed off the caller's path (a view got desynced).
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

/// A replay in a view's queue, with the view's document at the routing ([`Hub::epoch`]). The
/// request shares its event with the route's (`Arc`): queueing copies no payload.
struct Queued {
    request: ReplayRequest,
    epoch: u64,
}

/// Hears what the hub does - the recorder (RB-050-01). Called outside the hub's lock; never
/// call back into the hub from it.
pub trait Observer: Send + Sync {
    /// The active view `source` captured `message`, which went `route`.
    fn routed(&self, source: ViewId, message: &EventMessage, route: &Route);
    /// The replay of `event` into `view` ended with `outcome` - only an outcome the hub counts
    /// (of the document the event was routed in); a view too far behind is a timeout.
    fn replayed(&self, view: ViewId, event: &SyncEvent, outcome: &ReplayOutcome);
}

/// The hub with its replay queues.
pub struct SyncHub {
    me: Weak<SyncHub>,
    hub: Mutex<Hub>,
    replay: Mutex<Arc<dyn Replay>>,
    timeout: Mutex<Duration>,
    queues: Mutex<HashMap<ViewId, mpsc::Sender<Queued>>>,
    runtime: Handle,
    on_change: OnChange,
    observer: Mutex<Option<Arc<dyn Observer>>>,
}

impl SyncHub {
    /// A hub over `hub`, replaying with `replay` on `runtime`; `on_change` hears of desyncs.
    pub fn new(
        hub: Hub,
        replay: Arc<dyn Replay>,
        timeout: Duration,
        runtime: Handle,
        on_change: OnChange,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            hub: Mutex::new(hub),
            replay: Mutex::new(replay),
            timeout: Mutex::new(timeout),
            queues: Mutex::new(HashMap::new()),
            runtime,
            on_change,
            observer: Mutex::new(None),
        })
    }

    /// Runs `change` on the routing state. Never call back into the hub from it.
    pub fn with<R>(&self, change: impl FnOnce(&mut Hub) -> R) -> R {
        change(&mut lock(&self.hub))
    }

    /// The engine's replay from now on (RB-020-03 plugs in here).
    pub fn set_replay(&self, replay: Arc<dyn Replay>) {
        *lock(&self.replay) = replay;
    }

    /// Who hears of the active view's events and the replays' outcomes from now on.
    pub fn set_observer(&self, observer: Option<Arc<dyn Observer>>) {
        *lock(&self.observer) = observer;
    }

    /// The desync timeout from now on.
    pub fn set_timeout(&self, timeout: Duration) {
        *lock(&self.timeout) = timeout;
    }

    /// The views, in grid order; the queues of views that are gone stop.
    pub fn set_views(&self, views: &[ViewId]) {
        self.with(|hub| hub.set_views(views));
        lock(&self.queues).retain(|view, _| views.contains(view));
    }

    /// Routes the event `message` of `source` and queues its replays. The shell hears when a
    /// view fell behind or `Leader S N` was used up or taken back.
    pub fn on_event(&self, source: ViewId, message: &EventMessage) -> Route {
        let (route, skip_changed, queued, leader) = self.with(|hub| {
            let armed = hub.skip_next_armed();
            let leader = hub.active() == Some(source);
            let route = hub.route(source, message, Instant::now());
            let queued: Vec<Queued> = match &route {
                Route::Replay(requests) => requests
                    .iter()
                    .map(|request| Queued {
                        request: request.clone(),
                        epoch: hub.epoch(request.target),
                    })
                    .collect(),
                _ => Vec::new(),
            };
            (route, armed != hub.skip_next_armed(), queued, leader)
        });
        let observer = lock(&self.observer).clone();
        if leader && let Some(observer) = observer {
            observer.routed(source, message, &route);
        }
        let mut changed = skip_changed;
        for queued in queued {
            changed |= !self.enqueue(queued);
        }
        if changed {
            (self.on_change)();
        }
        route
    }

    /// Queues `request`; `false` when its view fell too far behind and was desynced.
    fn enqueue(&self, request: Queued) -> bool {
        let view = request.request.target;
        let mut queues = lock(&self.queues);
        let sender = queues.entry(view).or_insert_with(|| self.start_queue(view));
        let request = match sender.try_send(request) {
            Ok(()) => return true,
            Err(mpsc::error::TrySendError::Full(request)) => request,
            // The queue's task ended (the runtime is shutting down): start it again.
            Err(mpsc::error::TrySendError::Closed(request)) => {
                let fresh = self.start_queue(view);
                let event = request.request.event.clone();
                let sent = fresh.try_send(request).is_ok();
                queues.insert(view, fresh);
                if sent {
                    return true;
                }
                drop(queues);
                return !self.dropped(view, &event);
            }
        };
        drop(queues);
        !self.dropped(view, &request.request.event)
    }

    /// `event` could not be queued for `view`: a timeout, for the hub and the observer alike;
    /// `true` when the view became desynced.
    fn dropped(&self, view: ViewId, event: &SyncEvent) -> bool {
        let desynced = self.with(|hub| hub.record(view, event, &ReplayOutcome::Timeout));
        let observer = lock(&self.observer).clone();
        if let Some(observer) = observer {
            observer.replayed(view, event, &ReplayOutcome::Timeout);
        }
        desynced
    }

    fn start_queue(&self, view: ViewId) -> mpsc::Sender<Queued> {
        let (sender, receiver) = mpsc::channel(QUEUE_DEPTH);
        self.runtime
            .spawn(run_queue(self.me.clone(), view, receiver));
        sender
    }

    async fn replay_one(&self, queued: Queued) {
        let Queued { request, epoch } = queued;
        let (view, event) = (request.target, request.event.clone());
        if self.with(|hub| hub.active() == Some(view)) {
            // The view leads now: its own input is the original.
            return;
        }
        let (replay, timeout) = (lock(&self.replay).clone(), *lock(&self.timeout));
        let outcome = tokio::time::timeout(timeout, replay.dispatch(request))
            .await
            .unwrap_or(ReplayOutcome::Timeout);
        if let ReplayOutcome::Failed(error) = &outcome {
            tracing::debug!(%view, %error, "replay failed");
        }
        // Whether the outcome counts is decided with the hub's record, under one lock: an
        // outcome of a document the view has left is heard by nobody.
        let (current, desynced) = self.with(|hub| {
            (
                hub.epoch(view) == epoch,
                hub.record_at(view, epoch, &event, &outcome),
            )
        });
        let observer = lock(&self.observer).clone();
        if current && let Some(observer) = observer {
            observer.replayed(view, &event, &outcome);
        }
        if desynced {
            (self.on_change)();
        }
    }
}

/// Replays `view`'s queue in order while the hub lives.
async fn run_queue(hub: Weak<SyncHub>, view: ViewId, mut queue: mpsc::Receiver<Queued>) {
    while let Some(request) = queue.recv().await {
        let Some(hub) = hub.upgrade() else {
            return;
        };
        debug_assert_eq!(request.request.target, view);
        hub.replay_one(request).await;
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
