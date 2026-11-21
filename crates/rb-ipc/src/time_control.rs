//! The page clock and the animation rates in the snapshot (RB-094-07): present while the clock
//! of the views is not the real time or a view's animations do not run at 100 %, so the
//! activity bar shows it with Step, Real time and 100 % (`timeControl`; mirror:
//! `ui/src/ipc/activity.ts`). The clock is one for every view; the rates are per view.

use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

/// What is changed; absent from the snapshot while nothing is.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeControlActivity {
    /// The views' clock, while one is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockActivity>,
    /// The views whose animations do not run at 100 %, in view order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<ViewRate>,
}

impl TimeControlActivity {
    /// Nothing changed: the real time and every animation at 100 %.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clock.is_none() && self.animations.is_empty()
    }
}

/// The views' clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockActivity {
    /// Where it started: `2026-12-24T10:00:00` (each page's local time, `local`) or an instant
    /// in RFC 3339, UTC.
    pub start: String,
    /// `start` is a wall time each page reads in its own time zone.
    pub local: bool,
    /// 0 fixed, 1 real time, more is faster.
    pub speed: f64,
}

/// A view's animation rate: 0 stopped, 1 normal.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewRate {
    pub view: ViewId,
    pub rate: f64,
}

#[cfg(test)]
#[path = "time_control_tests.rs"]
mod tests;
