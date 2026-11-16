//! A plugin that keeps failing is suspended until the person resumes it (`docs/plugins.md`,
//! "Installation and lifecycle": unhealthy, then disabled after repeated failures).
//!
//! Copied from rCommander's `crates/rc-plugin-host/src/health.rs`. After `plugins.crashLimit`
//! failures - a trap, a timeout, exhausted fuel or memory - within [`FAILURE_WINDOW`] no call of
//! the plugin runs. The state lives in memory; the setting is `plugins.crashLimit`
//! (`schema/parts/plugins.json`), the Resume button the settings window's (RB-010-08).

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rb_plugin_types::{Failure, FailureKind, codes};

use crate::call_gate::{CallGate, CallSlot};

/// Failures counted towards a suspension are those of the last five minutes.
pub const FAILURE_WINDOW: Duration = Duration::from_secs(5 * 60);
/// `plugins.crashLimit`'s default and range.
pub const DEFAULT_CRASH_LIMIT: u32 = 3;
pub const MIN_CRASH_LIMIT: u32 = 1;
pub const MAX_CRASH_LIMIT: u32 = 10;

#[derive(Debug)]
struct State {
    limit: u32,
    failures: VecDeque<Instant>,
    suspended: bool,
}

/// The failure count and the suspension of one plugin, shared by all its stores, and how many
/// of its calls run at once.
#[derive(Debug)]
pub struct PluginHealth {
    state: Mutex<State>,
    calls: Arc<CallGate>,
}

impl Default for PluginHealth {
    fn default() -> Self {
        Self::new(DEFAULT_CRASH_LIMIT)
    }
}

impl PluginHealth {
    /// A plugin suspended after `crash_limit` failures, held to [`MIN_CRASH_LIMIT`] to
    /// [`MAX_CRASH_LIMIT`].
    #[must_use]
    pub fn new(crash_limit: u32) -> Self {
        Self {
            state: Mutex::new(State {
                limit: crash_limit.clamp(MIN_CRASH_LIMIT, MAX_CRASH_LIMIT),
                failures: VecDeque::new(),
                suspended: false,
            }),
            calls: Arc::new(CallGate::default()),
        }
    }

    /// Follows a changed `plugins.crashLimit`.
    pub fn set_crash_limit(&self, crash_limit: u32) {
        if let Ok(mut state) = self.state.lock() {
            state.limit = crash_limit.clamp(MIN_CRASH_LIMIT, MAX_CRASH_LIMIT);
        }
    }

    /// Counts one failure; `true` when it suspended the plugin.
    pub fn record_failure(&self) -> bool {
        self.record_failure_at(Instant::now())
    }

    pub(crate) fn record_failure_at(&self, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        while state
            .failures
            .front()
            .is_some_and(|first| now.saturating_duration_since(*first) > FAILURE_WINDOW)
        {
            state.failures.pop_front();
        }
        state.failures.push_back(now);
        if !state.suspended && state.failures.len() >= state.limit as usize {
            state.suspended = true;
            return true;
        }
        false
    }

    #[must_use]
    pub fn is_suspended(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.suspended)
            .unwrap_or(true)
    }

    /// The person's Resume: the plugin runs again, its count starts from zero.
    pub fn resume(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.suspended = false;
            state.failures.clear();
        }
    }

    /// Waits until fewer than `max_concurrent` calls of the plugin run - its manifest's
    /// `limits.max_concurrent` - and counts the caller's in until the slot is dropped.
    pub async fn enter(&self, max_concurrent: u32) -> CallSlot {
        self.calls.enter(max_concurrent).await
    }

    /// Whether a call entering now with `max_concurrent` would wait for another one to end
    /// (RB-080-90): a panel's call says it waits instead of seeming to hang.
    #[must_use]
    pub fn busy(&self, max_concurrent: u32) -> bool {
        self.calls.busy(max_concurrent)
    }

    /// `plugins.error.suspended` while the plugin may not run.
    ///
    /// # Errors
    ///
    /// The plugin is suspended.
    pub fn check(&self) -> Result<(), Failure> {
        if self.is_suspended() {
            return Err(Failure::coded(FailureKind::Other, codes::SUSPENDED));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
