//! How long the window's UI thread may keep a queued task waiting before the log says so
//! (`cef_app/ui_watchdog.rs`): the measure of a frozen window, portable and tested here.

use std::time::Duration;

/// A wait the log reports.
pub const SLOW: Duration = Duration::from_millis(500);
/// A wait the log reports while it lasts.
pub const STUCK: Duration = Duration::from_secs(10);

/// How a probe's wait reads in the log; `None` below [`SLOW`].
#[must_use]
pub fn verdict(waited: Duration) -> Option<&'static str> {
    (waited >= SLOW).then_some("the UI thread was busy; the window did not react")
}

#[cfg(test)]
#[path = "ui_stall_tests.rs"]
mod tests;
