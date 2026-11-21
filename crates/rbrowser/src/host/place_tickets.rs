//! Latest wins for the window's placements (RB-094-18). Each placement of the layout modes'
//! views (`UiBackend::place_views`) runs as a task of its own on the UI thread; while that thread
//! is busy - painting, the board's requests - the moves of a gesture queued up behind each other
//! and the window replayed every stale one. Each placement gets a ticket when it is posted, and
//! a view's place is applied only by the newest placement posted for it: an older one that runs
//! after a newer one was posted leaves that view alone. The order of the tasks is kept, so the
//! last place applied is the last one posted. Pure: the window keeps one behind a lock.

use std::collections::BTreeMap;

use rb_engine::ViewId;

use super::PlacedView;

/// The newest placement posted for each view not yet placed.
#[derive(Debug, Default)]
pub struct PlaceTickets {
    /// The ticket given last.
    next: u64,
    /// The newest ticket of each view whose placement has not run yet.
    newest: BTreeMap<ViewId, u64>,
}

impl PlaceTickets {
    /// No placement posted yet; `const` for the window's static.
    pub const fn new() -> Self {
        Self {
            next: 0,
            newest: BTreeMap::new(),
        }
    }

    /// `placed` is posted to the window: its ticket.
    pub fn post(&mut self, placed: &[PlacedView]) -> u64 {
        self.next = self.next.wrapping_add(1);
        for place in placed {
            self.newest.insert(place.view, self.next);
        }
        self.next
    }

    /// The placement `ticket` runs: the places of `placed` it still applies, and how many it
    /// skips because a newer placement of their view was posted since.
    pub fn take(&mut self, ticket: u64, placed: Vec<PlacedView>) -> (Vec<PlacedView>, usize) {
        let total = placed.len();
        let fresh: Vec<PlacedView> = placed
            .into_iter()
            .filter(|place| {
                let newest = self.newest.get(&place.view) == Some(&ticket);
                if newest {
                    self.newest.remove(&place.view);
                }
                newest
            })
            .collect();
        let skipped = total - fresh.len();
        (fresh, skipped)
    }
}

#[cfg(test)]
#[path = "place_tickets_tests.rs"]
mod tests;
