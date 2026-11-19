//! The pace of an export (RB-090-66, `automation.recorder.playbackPace`): `fast` writes the steps
//! back to back - each waits only for its element, as Playwright's locators do -, `recorded`
//! writes `await page.waitForTimeout(<ms>);` before a step for the pause the person took before
//! it (its `atMs` minus the step before's), each pause at most [`MAX_PAUSE_MS`].
//!
//! RB-090-84: the agent reports a menu's hover at the pointer-down of the click into its submenu
//! (RB-090-76), so that hover carries the click's time and played back to back with it. A hover
//! keeps [`HOVER_DWELL_MS`] before the step after it, taken from its own pause ([`pauses`]). Each
//! step but the first waits at least `automation.recorder.minPauseMs` (`options.minPauseMs`), so
//! a navigation's effect can be watched.

use serde::Deserialize;

/// The longest pause `recorded` keeps: a person who left the page for a coffee does not make a
/// run wait for it.
pub const MAX_PAUSE_MS: u64 = 10_000;

/// The least time a hover rests before the step after it at the `recorded` pace.
pub const HOVER_DWELL_MS: u64 = 500;

/// How the steps follow each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pace {
    /// Back to back.
    #[default]
    Fast,
    /// With the recorded pauses, each at most [`MAX_PAUSE_MS`].
    Recorded,
}

/// The pause before a step at `at_ms` after one at `previous` (none before the first step, none
/// when the steps were moved out of time order), at most [`MAX_PAUSE_MS`].
#[must_use]
pub fn pause_before(previous: Option<u64>, at_ms: u64) -> u64 {
    previous.map_or(0, |before| at_ms.saturating_sub(before).min(MAX_PAUSE_MS))
}

/// The pause before each of `steps` - `(atMs, is a hover)` in the order they run - at the
/// `recorded` pace: [`pause_before`] the step before, at least `min_ms` (at most
/// [`MAX_PAUSE_MS`]) for every step but the first, except that a hover followed sooner than
/// [`HOVER_DWELL_MS`] gives the step after it the rest of the dwell from the part of its own pause
/// above `min_ms`. The dwell moves time, it adds none.
#[must_use]
pub fn pauses(steps: &[(u64, bool)], min_ms: u64) -> Vec<u64> {
    let least = min_ms.min(MAX_PAUSE_MS);
    let mut previous = None;
    let mut out: Vec<u64> = steps
        .iter()
        .map(|&(at_ms, _)| {
            let pause = match previous {
                None => 0,
                Some(_) => pause_before(previous, at_ms).max(least),
            };
            previous = Some(at_ms);
            pause
        })
        .collect();
    for after in 1..out.len() {
        let hover = steps.get(after - 1).is_some_and(|&(_, hover)| hover);
        let gap = out[after];
        if hover && gap < HOVER_DWELL_MS {
            let floor = if after == 1 { 0 } else { least };
            let moved = (HOVER_DWELL_MS - gap).min(out[after - 1].saturating_sub(floor));
            out[after - 1] -= moved;
            out[after] += moved;
        }
    }
    out
}

#[cfg(test)]
#[path = "pace_tests.rs"]
mod tests;
