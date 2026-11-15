//! When a crashed sidecar is started again: after a delay that doubles with each crash or failed
//! start in a row, and not at all after too many - until `sidecar.restart`. A run that lasted
//! long enough (from its start to its death) counts as healthy and starts the count again.

use std::time::{Duration, Instant};

/// The restart rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestartPolicy {
    /// The delay before the first restart.
    pub base: Duration,
    /// The longest delay.
    pub max_delay: Duration,
    /// The crashes in a row after which no automatic restart follows.
    pub max_crashes: u32,
    /// A run at least this long resets the count.
    pub stable_after: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            base: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            max_crashes: 5,
            stable_after: Duration::from_secs(60),
        }
    }
}

/// The crashes in a row and when the current run started.
#[derive(Clone, Debug, Default)]
pub struct CrashHistory {
    in_a_row: u32,
    started: Option<Instant>,
}

impl CrashHistory {
    /// A run began.
    pub fn started(&mut self, now: Instant) {
        self.started = Some(now);
    }

    /// The run ended in a crash at `died`: the delay before the next start, or
    /// `Err(crashes in a row)` when the policy gives up.
    pub fn crashed(&mut self, policy: &RestartPolicy, died: Instant) -> Result<Duration, u32> {
        let stable = self
            .started
            .take()
            .is_some_and(|started| died.saturating_duration_since(started) >= policy.stable_after);
        if stable {
            self.in_a_row = 0;
        }
        self.count(policy)
    }

    /// A start failed (no launch, or no answer to the health ping): it counts like a crash of a
    /// run that never was.
    pub fn failed_start(&mut self, policy: &RestartPolicy) -> Result<Duration, u32> {
        self.started = None;
        self.count(policy)
    }

    fn count(&mut self, policy: &RestartPolicy) -> Result<Duration, u32> {
        self.in_a_row = self.in_a_row.saturating_add(1);
        if self.in_a_row > policy.max_crashes {
            return Err(self.in_a_row);
        }
        let factor = 2u32.saturating_pow(self.in_a_row - 1);
        Ok(policy.base.saturating_mul(factor).min(policy.max_delay))
    }

    /// Forgets the crashes (a restart by hand).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
#[path = "backoff_tests.rs"]
mod tests;
