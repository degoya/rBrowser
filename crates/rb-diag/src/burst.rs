//! A watch on the pace of an event that should be rare, so a loop that repeats it at frame rate
//! shows in the log at its default level (RB-099-07: a grid built anew with every frame for two
//! hours left no line in `rbrowser.log`, only a flood of Chromium warnings in `cef.log`).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Counts events within a sliding window and tells when more than a limit came in it - at most
/// once per quiet time, so a loop that goes on writes one line a minute, not one per event.
#[derive(Clone, Debug)]
pub struct BurstWatch {
    window: Duration,
    limit: usize,
    quiet: Duration,
    times: VecDeque<Instant>,
    told: Option<Instant>,
}

impl BurstWatch {
    /// More than `limit` events within `window` are a burst, told at most once per `quiet`.
    pub const fn new(window: Duration, limit: usize, quiet: Duration) -> Self {
        Self {
            window,
            limit,
            quiet,
            times: VecDeque::new(),
            told: None,
        }
    }

    /// Notes an event at `now`: the number of events within the window when they are a burst
    /// and none was told within the quiet time, `None` otherwise.
    pub fn note(&mut self, now: Instant) -> Option<usize> {
        while self
            .times
            .front()
            .is_some_and(|first| now.saturating_duration_since(*first) >= self.window)
        {
            self.times.pop_front();
        }
        self.times.push_back(now);
        let count = self.times.len();
        if count <= self.limit {
            return None;
        }
        if self
            .told
            .is_some_and(|told| now.saturating_duration_since(told) < self.quiet)
        {
            return None;
        }
        self.told = Some(now);
        Some(count)
    }
}

#[cfg(test)]
#[path = "burst_tests.rs"]
mod tests;
