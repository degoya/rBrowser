//! The device metrics of a view are applied latest-wins: a burst of bounds changes (a window
//! resize, a splitter drag) runs one worker per view, which applies again only while new
//! requests came in meanwhile. Each round reads the view's state as it is then, so the last
//! size always wins; nothing waits per request.

use std::collections::HashMap;
use std::sync::Mutex;

use rb_engine::ViewId;

use super::{Host, attach};
use crate::cdp::lock;

/// The views with a metrics worker, each with whether it must run once more.
#[derive(Default)]
pub(super) struct ApplyQueue {
    slots: Mutex<HashMap<ViewId, Slot>>,
}

#[derive(Default)]
struct Slot {
    again: bool,
    force: bool,
}

impl ApplyQueue {
    /// Marks `view` for an apply (all of it with `force`); `true` when no worker runs for it and
    /// the caller must start one.
    pub(super) fn request(&self, view: ViewId, force: bool) -> bool {
        let mut slots = lock(&self.slots);
        match slots.get_mut(&view) {
            Some(slot) => {
                slot.again = true;
                slot.force |= force;
                false
            }
            None => {
                slots.insert(view, Slot { again: true, force });
                true
            }
        }
    }

    /// The next round of `view`'s worker: `Some(force)` to apply once more, `None` when nothing
    /// came in since its last round (the worker ends; the next request starts a new one).
    pub(super) fn next(&self, view: ViewId) -> Option<bool> {
        let mut slots = lock(&self.slots);
        let slot = slots.get_mut(&view)?;
        if !slot.again {
            slots.remove(&view);
            return None;
        }
        slot.again = false;
        Some(std::mem::take(&mut slot.force))
    }

    /// No worker of `view` runs or is asked for (RB-099-18).
    #[cfg(test)]
    pub(super) fn idle(&self, view: ViewId) -> bool {
        !lock(&self.slots).contains_key(&view)
    }
}

impl Host {
    /// Applies `view`'s metrics (all of it with `force`) on its worker, coalesced with the
    /// requests that come in while it runs.
    pub(super) fn spawn_metrics(&self, view: ViewId, force: bool) {
        let Some(host) = self.me.upgrade() else {
            return;
        };
        if !self.applying.request(view, force) {
            return;
        }
        self.runtime.spawn(async move {
            while let Some(force) = host.applying.next(view) {
                if let Err(error) = attach::apply_metrics(&host, view, force).await {
                    tracing::warn!(%view, %error, "device metrics not applied");
                }
            }
        });
    }
}

#[cfg(test)]
impl Host {
    /// Every apply of `view` asked for so far has ended - the attach's and the metrics
    /// worker's - so a call none of them sent will not come from them (RB-099-18). The attach
    /// is marked before `on_load_end` returns, a worker's request before `spawn_apply` does.
    pub(super) fn applies_settled(&self, view: ViewId) -> bool {
        let attaching = self
            .state()
            .views
            .get(&view)
            .is_some_and(|entry| entry.attaching);
        !attaching && self.applying.idle(view)
    }
}

#[cfg(test)]
#[path = "apply_queue_tests.rs"]
mod tests;
