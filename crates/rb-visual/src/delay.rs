//! The delay before a capture (`capture.defaultDelay`, `capture.delay_cycle`): 0, 5 or 10 s from
//! the UI, any length up to [`MAX_DELAY_MS`] from MCP (`delayMs`), with a countdown in whole
//! seconds for the view header.

use std::time::Duration;

/// The longest delay MCP may ask for.
pub const MAX_DELAY_MS: u64 = 60_000;

/// The UI's delay steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Delay {
    /// At once.
    #[default]
    None,
    /// After 5 s.
    Five,
    /// After 10 s.
    Ten,
}

impl Delay {
    /// The step for `secs`; `None` for anything but 0, 5 and 10.
    pub fn from_secs(secs: u64) -> Option<Self> {
        match secs {
            0 => Some(Self::None),
            5 => Some(Self::Five),
            10 => Some(Self::Ten),
            _ => None,
        }
    }

    /// The seconds.
    pub fn secs(self) -> u64 {
        match self {
            Self::None => 0,
            Self::Five => 5,
            Self::Ten => 10,
        }
    }

    /// The wait.
    pub fn duration(self) -> Duration {
        Duration::from_secs(self.secs())
    }

    /// The next step of `capture.delay_cycle`: 0, 5, 10, 0, ...
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Five,
            Self::Five => Self::Ten,
            Self::Ten => Self::None,
        }
    }
}

/// `delay_ms` as a wait, at most [`MAX_DELAY_MS`].
pub fn clamp_delay_ms(delay_ms: u64) -> Duration {
    Duration::from_millis(delay_ms.min(MAX_DELAY_MS))
}

/// The seconds the countdown shows, one a second: `5, 4, 3, 2, 1` for 5 s; a part second counts
/// as a whole one.
pub fn countdown(wait: Duration) -> Vec<u64> {
    let secs = wait.as_millis().div_ceil(1000);
    let secs = u64::try_from(secs).unwrap_or(u64::MAX);
    (1..=secs).rev().collect()
}

#[cfg(test)]
#[path = "delay_tests.rs"]
mod tests;
