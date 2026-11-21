//! The page clock and the animation rates (RB-094-07): `clock_set`, the command
//! `emulation.clock` - one clock for every view, so it takes no target - and `animations_set`,
//! the command `overlays.animations` once per resolved target.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// `clock_set`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClockParams {
    /// Where the clock starts: `2026-12-24T10:00` (each page's local time, its emulated time
    /// zone included), `2026-12-24T09:00:00Z` or with an offset (one moment for every view),
    /// `2026-12-24` (midnight) or `now`. Absent with `speed`: the running clock goes on.
    #[serde(default)]
    pub time: Option<String>,
    /// How fast it runs: 0 fixed, 1 real time, up to 100. Absent: the running clock's speed,
    /// else the setting `emulation.clock.speed` (0 by default).
    #[serde(default)]
    pub speed: Option<f64>,
    /// `true`: every view gets the real time back; alone.
    #[serde(default)]
    pub reset: Option<bool>,
}

/// `animations_set`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationsParams {
    /// `"all"`, a view id or a breakpoint id; every view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The animation rate: 0 stops them, 0.1 is 10 %, 1 normal, up to 10.
    #[serde(default)]
    pub rate: Option<f64>,
    /// Moves stopped animations this many ms on (1 to 60000) instead of setting a rate.
    #[serde(default)]
    pub step_ms: Option<u32>,
}

#[tool_router(router = time_control_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Set the clock the pages read in every Chromium view at once - Date (new Date(), Date.now()) and Intl.DateTimeFormat without a date - to check calendars, countdowns and \"3 days ago\" alike in every breakpoint: time starts it (2026-12-24T10:00 is each page's local time, its emulated time zone included; 2026-12-24T09:00:00Z or an offset is one moment for every view; now), speed 0 holds it fixed, 1 runs it in real time, more runs it faster; speed alone changes the running clock from its current time. reset: true gives every view the real time back. Timers, performance.now() and requestAnimationFrame stay real; the clock holds across navigations and reaches the page already shown, but a page that read the time once shows the new one after a reload. One clock for all views, so it takes no target; Firefox and WebKit views keep the real time and are listed as unsupported. Answers clock ({ start, local, speed } or null), views, failed: [{ view, error }] and unsupported: [{ view, reason }]."
    )]
    pub(crate) async fn clock_set(
        &self,
        Parameters(params): Parameters<ClockParams>,
    ) -> ToolResult {
        respond(self.clock(params).await)
    }

    #[tool(
        description = "Slow down, stop or step the animations of the targeted Chromium views - CSS animations and transitions and Web Animations on the document timeline (Animation.setPlaybackRate) - for screenshots and visual diffs that do not depend on timing: rate 0 stops them, 0.1 is 10 %, 0.25 is 25 %, 1 normal (up to 10); stepMs moves stopped animations that many ms on, one step at a time. The rate holds across navigations and screenshots until it is set again. target: \"all\", a view id or a breakpoint id; default every view. Firefox and WebKit views are listed as unsupported. Answers rate (or step and moved), views, failed: [{ view, error }] and unsupported: [{ view, reason }]."
    )]
    pub(crate) async fn animations_set(
        &self,
        Parameters(params): Parameters<AnimationsParams>,
    ) -> ToolResult {
        respond(self.animations(params).await)
    }
}

impl RbMcpServer {
    async fn clock(&self, params: ClockParams) -> Result<Value, ToolFailure> {
        let mut args = Map::new();
        if let Some(time) = params.time {
            args.insert("time".to_owned(), json!(time));
        }
        if let Some(speed) = params.speed {
            args.insert("speed".to_owned(), json!(speed));
        }
        if let Some(reset) = params.reset {
            args.insert("reset".to_owned(), json!(reset));
        }
        // Without arguments the command toggles, as its shortcut does: an agent says what it
        // wants.
        if args.is_empty() {
            return Err(invalid("give time, speed or reset: true"));
        }
        self.command("emulation.clock", Value::Object(args)).await
    }

    async fn animations(&self, params: AnimationsParams) -> Result<Value, ToolFailure> {
        let args = match (params.rate, params.step_ms) {
            (Some(rate), None) => json!({ "rate": rate }),
            (None, Some(step)) => json!({ "step": step }),
            (Some(_), Some(_)) => return Err(invalid("rate or stepMs, not both")),
            (None, None) => return Err(invalid("give rate (0 stops, 1 is normal) or stepMs")),
        };
        self.per_target("overlays.animations", params.target.as_ref(), args)
            .await
    }
}

fn invalid(text: &str) -> ToolFailure {
    ToolFailure::new("invalid_arguments", text)
}

#[cfg(test)]
#[path = "time_control_tests.rs"]
mod tests;
