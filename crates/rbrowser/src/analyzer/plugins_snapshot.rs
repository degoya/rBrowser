//! The plugins in effect as a swappable snapshot (RB-092-06): readers take an [`Arc`] and keep
//! it until they are done; a live change ([`super::reload`]) puts a whole new snapshot in place
//! under the write lock and never changes one a reader holds. A generation counts the swaps, so
//! a cache built from the plugins (the panel files) knows when to build again.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

/// One value in effect, replaced whole.
pub(super) struct Snapshots<T> {
    current: RwLock<Option<Arc<T>>>,
    generation: AtomicU64,
}

impl<T> Snapshots<T> {
    /// Nothing in effect yet, generation 0.
    pub(super) const fn new() -> Self {
        Self {
            current: RwLock::new(None),
            generation: AtomicU64::new(0),
        }
    }

    /// The snapshot in effect; `None` before the first [`Snapshots::put`].
    pub(super) fn get(&self) -> Option<Arc<T>> {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Puts `next` in effect; the snapshot it replaced, which its readers keep until they drop it.
    pub(super) fn put(&self, next: Arc<T>) -> Option<Arc<T>> {
        let mut current = self.current.write().unwrap_or_else(PoisonError::into_inner);
        let old = current.replace(next);
        // Counted under the lock: a reader that sees the new generation finds the new snapshot.
        self.generation.fetch_add(1, Ordering::AcqRel);
        old
    }

    /// How many snapshots were put so far.
    pub(super) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

#[cfg(test)]
#[path = "plugins_snapshot_tests.rs"]
mod tests;
