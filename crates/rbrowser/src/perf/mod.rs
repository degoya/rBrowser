//! The performance budget (RB-020-09): eight views, two of them at DPR 3, the active one
//! scrolling at 50 fps or more and all processes together below 3 GB (K3,
//! `docs/perf-budget.md`, measured by `scripts/perf.sh`).
//!
//! | Part | Where |
//! | --- | --- |
//! | Throttling: views without area hidden, a CPU budget for inactive views | [`throttle`], `host/throttle.rs` |
//! | The catch-up of a view shown again: the leader's URL, its last scroll | [`catchup`], [`glue`] |
//! | The memory of rBrowser's processes (reusable, RB-060-10) | [`process`] |
//! | The memory warning | [`memory`], [`glue`] |
//! | The frozen-window log: how long the UI thread kept a task waiting | [`ui_stall`], `cef_app/ui_watchdog.rs` |
//! | `performance.toggle_throttle` (`Leader V Q`, MCP `settings_set`) | [`commands`] |
//! | The timing counters (RB-090-32): `debug.perf_counters` (`Leader D B`, MCP `app_perf`), `perf.json` of the diagnostics ZIP | [`counters`] |
//!
//! Settings: `performance.throttleHidden`, `performance.inactiveFps` (Layout > Grid),
//! `performance.memoryWarningMb` (Advanced > Experimental flags), `performance.counters`
//! (Advanced > Diagnostics).

pub mod catchup;
pub mod commands;
pub mod counters;
mod glue;
pub mod memory;
pub mod process;
#[cfg(windows)]
mod process_windows;
pub mod throttle;
pub mod ui_stall;

use rb_engine::ViewId;

use crate::host::Host;

pub use commands::{TOGGLE_THROTTLE, run_command};
pub use glue::{catch_up, on_start, on_user_settings_changed};
pub use throttle::{Throttle, ThrottleAction, ThrottleSettings, ThrottleState, ViewFacts};

/// What `view` is throttled to now; [`ThrottleState::Visible`] for an unknown view. A change
/// comes with `ViewEvent::Updated(view)`.
pub fn throttle_state(host: &Host, view: ViewId) -> ThrottleState {
    host.throttle_state(view)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
