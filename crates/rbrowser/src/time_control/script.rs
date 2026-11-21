//! What a view's page gets (RB-094-07), as text and CDP parameters: the clock script
//! (`clock.js`, the main world before the page's scripts) with its configuration, the
//! expression that resets it in the document there, the animation step (`step.js`, rBrowser's
//! isolated world) and the document timeline's rate. Pure; `hooks.rs` sends them. The agent's
//! tests run both scripts (`agent/src/clock-script.test.ts`) and the e2e run composes them as
//! here (`agent/scripts/clock-e2e.mjs`).

use serde_json::{Value, json};

use super::model::Clock;

/// The page clock: defines its API under `Symbol.for(CLOCK_KEY)` once per realm.
pub const CLOCK_SOURCE: &str = include_str!("clock.js");
/// The animation step: a function of the step in ms.
pub const STEP_SOURCE: &str = include_str!("step.js");
/// The `Symbol.for` name of the clock's API.
pub const CLOCK_KEY: &str = "rbrowser.clock";
/// Gives the document there its real time back; `false` where the clock never ran.
pub const RESET_EXPRESSION: &str = r#"globalThis[Symbol.for("rbrowser.clock")]?.reset() ?? false"#;
/// The CDP method of the document timeline's rate.
pub const RATE_METHOD: &str = "Animation.setPlaybackRate";

/// The script registered in a view for `clock`: the clock, then its configuration as JSON - data,
/// never code.
#[must_use]
pub fn install_source(clock: &Clock) -> String {
    format!(
        "{CLOCK_SOURCE}\n;globalThis[Symbol.for(\"{CLOCK_KEY}\")].set({});",
        clock.config()
    )
}

/// The step of `ms` as one expression.
#[must_use]
pub fn step_expression(ms: u32) -> String {
    format!("({})({ms})", STEP_SOURCE.trim_end())
}

/// The parameters of [`RATE_METHOD`] for `rate`.
#[must_use]
pub fn rate_params(rate: f64) -> Value {
    json!({ "playbackRate": rate })
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
