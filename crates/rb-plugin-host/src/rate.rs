//! The rate limit of `http`: at most `rate_limit_per_second` requests of one plugin, spread evenly.
//! A request beyond it waits for its slot; the wait is host time, credited back to the call.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::recover::LockOrRecover as _;

/// The next free slot of one plugin, shared by all its stores.
#[derive(Debug)]
pub(crate) struct RateLimiter {
    interval: Duration,
    next: Mutex<Option<Instant>>,
}

impl RateLimiter {
    /// `per_second` requests a second, at least one.
    pub(crate) fn new(per_second: u32) -> Self {
        Self {
            interval: Duration::from_secs(1) / per_second.max(1),
            next: Mutex::new(None),
        }
    }

    /// Takes the next slot; answers how long to wait before using it.
    pub(crate) fn reserve(&self) -> Duration {
        self.reserve_at(Instant::now())
    }

    pub(crate) fn reserve_at(&self, now: Instant) -> Duration {
        let mut next = self.next.lock_or_recover("rate.next");
        let slot = next.map_or(now, |next| next.max(now));
        *next = Some(slot + self.interval);
        slot.saturating_duration_since(now)
    }
}

#[cfg(test)]
#[path = "rate_tests.rs"]
mod tests;
