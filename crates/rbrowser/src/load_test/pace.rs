//! The rate a run is held to (RB-096-07): the scenario's target requests per second, at most
//! `load-test.maxRps`. All users share one [`Pace`]: each send takes the next free slot, the
//! slots one interval apart; a slot in the past is now, so a quiet stretch saves up no burst.
//! The clock is the caller's (tokio's, so a test runs on the paused one).

use std::time::Duration;

use tokio::time::Instant;

/// The next free slot of a rate.
#[derive(Clone, Copy, Debug)]
pub struct Pace {
    interval: Duration,
    next: Option<Instant>,
}

impl Pace {
    /// A pace of `rps` requests per second; none for no finite, positive rate.
    #[must_use]
    pub fn new(rps: f64) -> Option<Self> {
        (rps.is_finite() && rps > 0.0).then(|| Self {
            interval: Duration::from_secs_f64(1.0 / rps),
            next: None,
        })
    }

    /// When a send asked for at `now` may go; the slot after it is taken.
    pub fn slot(&mut self, now: Instant) -> Instant {
        let slot = self.next.map_or(now, |next| next.max(now));
        self.next = Some(slot + self.interval);
        slot
    }
}

#[cfg(test)]
#[path = "pace_tests.rs"]
mod tests;
