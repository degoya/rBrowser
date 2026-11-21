//! When a client last reached the server (RB-091-09): every `/mcp` request that passed the
//! token's check counts, so the MCP setup's "Check connection" can say whether a client is
//! connected - the time and a count, never who or what it asked, never a token.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

/// The requests a server took since it started.
#[derive(Debug, Default)]
pub struct Activity {
    state: Mutex<State>,
}

#[derive(Debug, Default, Clone, Copy)]
struct State {
    last: Option<SystemTime>,
    calls: u64,
}

impl Activity {
    /// No request yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                last: None,
                calls: 0,
            }),
        }
    }

    /// A request passed the checks at `at`.
    pub fn record(&self, at: SystemTime) {
        let mut state = self.locked();
        state.last = Some(state.last.map_or(at, |last| last.max(at)));
        state.calls = state.calls.saturating_add(1);
    }

    fn locked(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The server started again: nothing reached it yet.
    pub fn reset(&self) {
        *self.locked() = State::default();
    }

    /// How long ago the last request came, seen at `now` (zero for a clock that went back),
    /// and how many came; `None` before the first.
    #[must_use]
    pub fn since_last(&self, now: SystemTime) -> (Option<Duration>, u64) {
        let state = *self.locked();
        let ago = state
            .last
            .map(|last| now.duration_since(last).unwrap_or(Duration::ZERO));
        (ago, state.calls)
    }
}

/// The server of this process.
pub static SERVER: Activity = Activity::new();

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
