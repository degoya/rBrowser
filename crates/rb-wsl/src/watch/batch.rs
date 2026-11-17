//! The debounce: changes are collected until none came for the quiet time (`reload.debounceMs`),
//! then go out as one [`Batch`]. A save that writes 50 files gives one reload, not 50. A steady
//! stream of changes (a build writing for seconds) still goes out after [`longest_wait`].
//!
//! Pure: the caller passes the time and sleeps until [`Debouncer::deadline`].

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::{Change, ChangeKind};

/// `reload.debounceMs` by default.
pub const DEFAULT_DEBOUNCE_MS: u64 = 200;
/// The most paths in one batch; more become a [`Batch::Rescan`] (a checkout, a clean build),
/// which reloads everything anyway and keeps the protocol line short.
pub const MAX_BATCH: usize = 500;

/// What a debounce hands on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Batch {
    /// These paths changed, each once, sorted.
    Changes(Vec<Change>),
    /// Something changed, unknown what (an inotify overflow, too many changes).
    Rescan,
}

/// Collects changes until the quiet time has passed.
#[derive(Debug, Clone)]
pub struct Debouncer {
    quiet: Duration,
    pending: BTreeMap<String, ChangeKind>,
    rescan: bool,
    first: Option<Instant>,
    last: Option<Instant>,
}

impl Debouncer {
    /// A debounce with the quiet time `quiet`.
    #[must_use]
    pub fn new(quiet: Duration) -> Self {
        Self {
            quiet,
            pending: BTreeMap::new(),
            rescan: false,
            first: None,
            last: None,
        }
    }

    /// A change at `now`.
    pub fn push(&mut self, change: Change, now: Instant) {
        self.touch(now);
        if !self.rescan {
            self.pending.insert(change.path, change.kind);
            if self.pending.len() > MAX_BATCH {
                self.push_rescan(now);
            }
        }
    }

    /// Something changed at `now`, unknown what.
    pub fn push_rescan(&mut self, now: Instant) {
        self.touch(now);
        self.rescan = true;
        self.pending.clear();
    }

    /// When the batch is due; `None` when nothing waits.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        let (first, last) = (self.first?, self.last?);
        Some((last + self.quiet).min(first + longest_wait(self.quiet)))
    }

    /// The batch, when it is due at `now`.
    pub fn take(&mut self, now: Instant) -> Option<Batch> {
        if self.deadline()? > now {
            return None;
        }
        self.first = None;
        self.last = None;
        if std::mem::take(&mut self.rescan) {
            return Some(Batch::Rescan);
        }
        let changes = std::mem::take(&mut self.pending)
            .into_iter()
            .map(|(path, kind)| Change { path, kind })
            .collect();
        Some(Batch::Changes(changes))
    }

    fn touch(&mut self, now: Instant) {
        self.first.get_or_insert(now);
        self.last = Some(now);
    }
}

/// The longest a batch waits under a steady stream: five quiet times, at least a second.
#[must_use]
pub fn longest_wait(quiet: Duration) -> Duration {
    quiet.saturating_mul(5).max(Duration::from_secs(1))
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
