//! The pace of an export (RB-090-66, `automation.recorder.playbackPace`): `fast` writes the steps
//! back to back - each waits only for its element, as Playwright's locators do -, `recorded`
//! writes `await page.waitForTimeout(<ms>);` before a step for the pause the person took before
//! it (its `atMs` minus the step before's), each pause at most [`MAX_PAUSE_MS`].

use serde::Deserialize;

/// The longest pause `recorded` keeps: a person who left the page for a coffee does not make a
/// run wait for it.
pub const MAX_PAUSE_MS: u64 = 10_000;

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

#[cfg(test)]
#[path = "pace_tests.rs"]
mod tests;
