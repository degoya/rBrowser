//! The locks of the views' applies (RB-099-19). One per view: a later size of a view never
//! loses against an earlier one, and the other views apply beside it - a resize of eight views
//! no longer sends their metrics one after the other. One per browser context, around the
//! geolocation grants only: a view's grant plan reads which origins the other views of its
//! context hold, so two plans of one context must not run at once.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use rb_engine::ViewId;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::cdp::lock;

/// The per-view and per-context apply locks of a host.
#[derive(Default)]
pub(super) struct ApplyLocks {
    views: Keyed<ViewId>,
    contexts: Keyed<String>,
}

impl ApplyLocks {
    /// Waits until no other apply of `view` runs; held until the guard drops.
    pub(super) async fn view(&self, view: ViewId) -> OwnedMutexGuard<()> {
        self.views.lock(view).await
    }

    /// Waits until no other grant plan of the browser context `context` runs; held until the
    /// guard drops.
    pub(super) async fn context(&self, context: &str) -> OwnedMutexGuard<()> {
        self.contexts.lock(context.to_owned()).await
    }
}

/// One async lock per key, made when first asked for; a lock nobody holds or waits for is
/// dropped at the next call, so the map stays as small as the work under way.
struct Keyed<K> {
    locks: Mutex<HashMap<K, Arc<AsyncMutex<()>>>>,
}

impl<K> Default for Keyed<K> {
    fn default() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Eq + Hash> Keyed<K> {
    async fn lock(&self, key: K) -> OwnedMutexGuard<()> {
        let one = {
            let mut locks = lock(&self.locks);
            // Held or awaited elsewhere means another `Arc` than the map's.
            locks.retain(|_, one| Arc::strong_count(one) > 1);
            Arc::clone(
                locks
                    .entry(key)
                    .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
            )
        };
        one.lock_owned().await
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        lock(&self.locks).len()
    }
}

#[cfg(test)]
#[path = "apply_locks_tests.rs"]
mod tests;
