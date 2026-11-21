//! The loads the hub gave the views (a follow, a resync, navigation sync's fallback) and the
//! host's loading state of every view. A view loading a URL the hub gave it gets no events: its
//! old document is going away. That ends when its navigation commits, when a load of the view
//! ends after it was given (a load of the URL it already shows commits no new URL), or - for a
//! load that never starts (the same document, a refused URL) - [`LOAD_START_GRACE`] after the
//! hub gave it.
//!
//! The host counts the loads that ended per view; the hub reads that count with the loading
//! flag whenever the view changed, so an end is seen even when start and end both happened
//! before the hub looked.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use rb_engine::ViewId;

/// How long a load the hub gave may take to start before it counts as one that will not.
pub const LOAD_START_GRACE: Duration = Duration::from_secs(2);

/// A change of the host's loading state of a view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadChange {
    /// The view started loading.
    Started,
    /// A load of the view ended.
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    since: Instant,
    /// The view's ended loads when the load was given.
    baseline: u64,
    started: bool,
}

/// The loads of the views.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadTable {
    /// The loads the hub gave, until they commit or end.
    pending: BTreeMap<ViewId, Pending>,
    /// The views the host said are loading.
    busy: BTreeSet<ViewId>,
    /// Per view, the loads the host said have ended.
    ended: BTreeMap<ViewId, u64>,
}

impl LoadTable {
    /// The hub gives `view` a URL to load at `now`. A view already loading counts as started:
    /// the new load replaces the running one.
    pub fn mark(&mut self, view: ViewId, now: Instant) {
        let pending = Pending {
            since: now,
            baseline: self.ended.get(&view).copied().unwrap_or_default(),
            started: self.busy.contains(&view),
        };
        self.pending.insert(view, pending);
    }

    /// The load of `view` is over (it committed) or never happened (the navigation failed);
    /// `true` when it was pending.
    pub fn clear(&mut self, view: ViewId) -> bool {
        self.pending.remove(&view).is_some()
    }

    /// The host says `view` is `loading` (or not) and has seen `ended` loads end; the change,
    /// if it is one. A load the hub gave is over once a load ended after it was given.
    pub fn host_state(&mut self, view: ViewId, loading: bool, ended: u64) -> Option<LoadChange> {
        let before = self.ended.insert(view, ended).unwrap_or_default();
        let was_busy = if loading {
            !self.busy.insert(view)
        } else {
            self.busy.remove(&view)
        };
        if let Some(pending) = self.pending.get_mut(&view) {
            if ended > pending.baseline {
                self.pending.remove(&view);
            } else if loading {
                pending.started = true;
            }
        }
        if ended > before {
            Some(LoadChange::Finished)
        } else if loading && !was_busy {
            Some(LoadChange::Started)
        } else {
            None
        }
    }

    /// When the hub gave `view` the load it has pending at `now`; `None` without one, and for a
    /// load that never started within [`LOAD_START_GRACE`].
    pub fn given_at(&self, view: ViewId, now: Instant) -> Option<Instant> {
        self.pending
            .get(&view)
            .filter(|pending| pending.counts(now))
            .map(|pending| pending.since)
    }

    /// Whether `view` loads a URL the hub gave it at `now`.
    pub fn is_loading(&self, view: ViewId, now: Instant) -> bool {
        self.pending
            .get(&view)
            .is_some_and(|pending| pending.counts(now))
    }

    /// Forgets the loads that never started within [`LOAD_START_GRACE`].
    pub fn expire(&mut self, now: Instant) {
        self.pending.retain(|_, pending| pending.counts(now));
    }

    /// Forgets the views not in `views`.
    pub fn retain(&mut self, views: &[ViewId]) {
        self.pending.retain(|view, _| views.contains(view));
        self.busy.retain(|view| views.contains(view));
        self.ended.retain(|view, _| views.contains(view));
    }
}

impl Pending {
    fn counts(&self, now: Instant) -> bool {
        self.started || now.saturating_duration_since(self.since) < LOAD_START_GRACE
    }
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
