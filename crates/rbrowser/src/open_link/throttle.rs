//! At most [`MAX_OPENS`] links in [`WINDOW`]: a page or panel that clicks for the person, or a
//! loop, cannot flood the desktop with browser windows. The panels' bridge holds them to a user
//! gesture before they get here; this is the core's own bound for every caller.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The links one window of time lets through.
pub const MAX_OPENS: usize = 3;
/// The window.
pub const WINDOW: Duration = Duration::from_secs(5);

/// The links opened in the last [`WINDOW`].
#[derive(Debug, Default)]
pub struct Throttle {
    opened: VecDeque<Instant>,
}

impl Throttle {
    /// No link opened yet.
    pub const fn new() -> Self {
        Self {
            opened: VecDeque::new(),
        }
    }

    /// Whether a link may open at `now`; one that may is counted as opened.
    pub fn allow(&mut self, now: Instant) -> bool {
        while self
            .opened
            .front()
            .is_some_and(|at| now.duration_since(*at) >= WINDOW)
        {
            self.opened.pop_front();
        }
        if self.opened.len() >= MAX_OPENS {
            return false;
        }
        self.opened.push_back(now);
        true
    }

    /// The link counted at `at` did not open after all: its slot is free again.
    pub fn release(&mut self, at: Instant) {
        if let Some(index) = self.opened.iter().rposition(|opened| *opened == at) {
            self.opened.remove(index);
        }
    }
}

#[cfg(test)]
#[path = "throttle_tests.rs"]
mod tests;
