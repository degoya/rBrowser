//! The users of a run over its time and the ceilings a person set (RB-096-07). The ceilings -
//! users, rate, duration - are settings only (owner, 2026-11-21: no fixed limit in code; by
//! default 50 users and 5 minutes): a scenario over one is refused with the setting named, never
//! cut down quietly.

use std::time::Duration;

use serde_json::{Map, Value};

use super::scenario::Profile;

/// `load-test.maxUsers`: the most virtual users of a run.
pub const MAX_USERS_KEY: &str = "load-test.maxUsers";
/// `load-test.maxRps`: the most requests per second of a run, all users together.
pub const MAX_RPS_KEY: &str = "load-test.maxRps";
/// `load-test.maxDurationS`: the longest run, in seconds.
pub const MAX_DURATION_KEY: &str = "load-test.maxDurationS";
/// The defaults (owner, 2026-11-21: 50 users, 5 minutes).
pub const DEFAULT_MAX_USERS: u32 = 50;
pub const DEFAULT_MAX_RPS: f64 = 200.0;
pub const DEFAULT_MAX_DURATION_S: u64 = 300;

/// The ceilings in effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub max_users: u32,
    pub max_rps: f64,
    pub max_duration_s: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_users: DEFAULT_MAX_USERS,
            max_rps: DEFAULT_MAX_RPS,
            max_duration_s: DEFAULT_MAX_DURATION_S,
        }
    }
}

impl Limits {
    /// The ceilings of the plugin's settings `values`; a value that is no positive number is
    /// its default.
    #[must_use]
    pub fn of(values: &Map<String, Value>) -> Self {
        let whole = |key: &str| values.get(key).and_then(Value::as_u64).filter(|n| *n >= 1);
        let real = |key: &str| {
            values
                .get(key)
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite() && *n >= 1.0)
        };
        let defaults = Self::default();
        Self {
            max_users: whole(MAX_USERS_KEY)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(defaults.max_users),
            max_rps: real(MAX_RPS_KEY).unwrap_or(defaults.max_rps),
            max_duration_s: whole(MAX_DURATION_KEY).unwrap_or(defaults.max_duration_s),
        }
    }
}

/// A profile over a ceiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    /// `users`, `rps` or `duration`.
    pub what: &'static str,
    pub wanted: f64,
    pub limit: f64,
    /// The setting that raises it.
    pub setting: &'static str,
}

/// Whether `profile` stays within `limits`.
///
/// # Errors
///
/// The first ceiling it passes.
pub fn check_limits(profile: &Profile, limits: &Limits) -> Result<(), Over> {
    if profile.users > limits.max_users {
        return Err(Over {
            what: "users",
            wanted: f64::from(profile.users),
            limit: f64::from(limits.max_users),
            setting: MAX_USERS_KEY,
        });
    }
    if let Some(rps) = profile.rps
        && rps > limits.max_rps
    {
        return Err(Over {
            what: "rps",
            wanted: rps,
            limit: limits.max_rps,
            setting: MAX_RPS_KEY,
        });
    }
    if profile.duration_s() > limits.max_duration_s {
        return Err(Over {
            what: "duration",
            wanted: profile.duration_s() as f64,
            limit: limits.max_duration_s as f64,
            setting: MAX_DURATION_KEY,
        });
    }
    Ok(())
}

/// The requests per second a run is held to: the scenario's, or the settings' ceiling.
#[must_use]
pub fn rate_of(profile: &Profile, limits: &Limits) -> f64 {
    profile.rps.unwrap_or(limits.max_rps).min(limits.max_rps)
}

/// The users that run `elapsed` into the run: climbing from one to `users` over the ramp up,
/// `users` while holding, falling back to one over the ramp down, none after the end.
#[must_use]
pub fn users_at(profile: &Profile, elapsed: Duration) -> u32 {
    // A hair off a whole number counts as that number: 0.1 * 10 is not quite 1.
    const EPSILON: f64 = 1e-9;
    let t = elapsed.as_secs_f64();
    let users = f64::from(profile.users);
    let up = f64::from(profile.ramp_up_s);
    let hold_end = up + f64::from(profile.hold_s);
    let end = profile.duration_s() as f64;
    let count = if t >= end {
        0.0
    } else if t < up {
        ((users * t / up) + EPSILON).floor() + 1.0
    } else if t < hold_end {
        users
    } else {
        let down = f64::from(profile.ramp_down_s);
        (users * (end - t) / down - EPSILON).ceil()
    };
    // The count lies in 0..=users, a whole number.
    count.clamp(0.0, users) as u32
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
