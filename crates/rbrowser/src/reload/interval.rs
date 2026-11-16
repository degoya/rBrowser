//! The interval reload for pages that no watch can see (a staging server): every view reloads
//! each `reload.intervalSec` (or the environment's own interval), but not while someone works in
//! a view - input pauses it for `reload.intervalPauseSec`, and a view with a focused text field
//! postpones the next one.
//!
//! Pure: the caller passes the time.

use std::time::{Duration, Instant};

/// The schedule of one interval reload.
#[derive(Clone, Debug)]
pub struct IntervalReload {
    period: Duration,
    pause: Duration,
    last_reload: Instant,
    last_input: Option<Instant>,
}

/// What the schedule says now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    /// Reload now.
    Due,
    /// Look again after this long.
    Wait(Duration),
}

impl IntervalReload {
    /// Every `period`, paused for `pause` after input; the first one a period after `now`.
    #[must_use]
    pub fn new(period: Duration, pause: Duration, now: Instant) -> Self {
        Self {
            period,
            pause,
            last_reload: now,
            last_input: None,
        }
    }

    /// The period.
    #[must_use]
    pub fn period(&self) -> Duration {
        self.period
    }

    /// Input in a view at `now` (a click, a key, typing seen by the probe).
    pub fn note_input(&mut self, now: Instant) {
        self.last_input = Some(now);
    }

    /// The views were reloaded at `now` (by this schedule or by hand).
    pub fn reloaded(&mut self, now: Instant) {
        self.last_reload = now;
    }

    /// Whether to reload at `now`, or how long to wait.
    #[must_use]
    pub fn tick(&self, now: Instant) -> Tick {
        let mut due = self.last_reload + self.period;
        if let Some(input) = self.last_input {
            due = due.max(input + self.pause);
        }
        if now >= due {
            Tick::Due
        } else {
            Tick::Wait(due - now)
        }
    }
}

#[cfg(test)]
#[path = "interval_tests.rs"]
mod tests;
