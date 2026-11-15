//! The replay of the scroll kind of sync in a Chromium view (RB-020-04), per `sync.scrollMode`:
//!
//! | Event | `anchor` (default) | `wheel` |
//! | --- | --- | --- |
//! | `scroll` | the agent's `scroll` command: the anchor onto the edge, exact start and end, the same share without the anchor | nothing (the wheel carries it) |
//! | `wheel` | nothing (the scroll carries it) | the agent's `point` for the element under the pointer, then `Input.dispatchMouseEvent` `mouseWheel` there |
//!
//! Anchor mode sets the position directly, so a page's smooth scrolling does not animate
//! against the next frame; wheel mode is for pages that take over scrolling (scroll hijacking)
//! and ignore a set position. Both run in the agent's isolated world, in one round trip, and set
//! the replay marker first: the view captures nothing the scroll causes, so nothing echoes back.
//! A missing container is `NotFound`; scroll never desyncs a view (`rb_sync::marks_desync`).

use rb_config::ScrollMode;
use rb_engine::{EngineError, ViewId};
use rb_sync::{
    AGENT_GLOBAL, AgentCommand, CommandResult, ElementTarget, ReplayOutcome, ScrollData, SyncEvent,
    WheelData,
};
use serde_json::{Value, json};

use crate::replay::ReplayPort;
use crate::replay::input::{MOUSE, input_point};
use crate::replay::prologue::{MARKER_MS, parse};

/// CSS px of a wheel delta in lines (`deltaMode` 1), as Chromium counts a line.
pub const LINE_PX: f64 = 40.0;
/// CSS px of a wheel delta in pages (`deltaMode` 2).
pub const PAGE_PX: f64 = 800.0;
/// The largest wheel delta sent, CSS px per axis: a page cannot make one view jump far.
pub const MAX_WHEEL_PX: f64 = 10_000.0;

/// What `event` does in `view` when it is of the scroll kind: `Ok(true)` when the view was
/// scrolled, `Ok(false)` when the mode leaves the event to its sibling (`scroll` or `wheel`);
/// `None` for every other event.
pub async fn replay(
    port: &dyn ReplayPort,
    view: ViewId,
    event: &SyncEvent,
) -> Option<Result<bool, ReplayOutcome>> {
    let result = match (event, port.scroll_mode()) {
        (SyncEvent::Scroll(data), ScrollMode::Anchor) => {
            apply(port, view, data, None).await.map(|()| true)
        }
        (SyncEvent::Wheel(wheel), ScrollMode::Wheel) => {
            wheel_into(port, view, wheel).await.map(|()| true)
        }
        (SyncEvent::Scroll(_) | SyncEvent::Wheel(_), _) => Ok(false),
        _ => return None,
    };
    Some(result)
}

/// Applies `data` in `view` with the agent's `scroll` command. With `require`, that target must
/// resolve first (MCP's `element`): `NotFound`/`NotVisible` when it does not.
pub async fn apply(
    port: &dyn ReplayPort,
    view: ViewId,
    data: &ScrollData,
    require: Option<&ElementTarget>,
) -> Result<(), ReplayOutcome> {
    let source = scroll_expression(data, require).map_err(failed)?;
    let value = port.world(view, source).await.map_err(failed)?;
    scroll_outcome(&value)
}

/// The expression for [`apply`]. Every value in it is JSON, so nothing in a selector can break
/// out of it.
pub fn scroll_expression(
    data: &ScrollData,
    require: Option<&ElementTarget>,
) -> Result<String, EngineError> {
    let scroll = to_json(&AgentCommand::Scroll(data.clone()))?;
    let check = match require {
        Some(target) => format!(
            "const found=agent.command({});if(!found||found.status!=='ok')return found;",
            to_json(&AgentCommand::Resolve {
                target: target.clone()
            })?
        ),
        None => String::new(),
    };
    Ok(format!(
        "(()=>{{const agent=globalThis.{AGENT_GLOBAL};\
         if(!agent)return{{error:'no agent'}};\
         {check}return agent.command({scroll});}})()"
    ))
}

/// Reads the answer of [`scroll_expression`]. The error never quotes the value.
pub fn scroll_outcome(value: &Value) -> Result<(), ReplayOutcome> {
    if value.get("error").is_some() {
        return Err(ReplayOutcome::Failed(
            "no sync agent in the view".to_owned(),
        ));
    }
    match serde_json::from_value::<CommandResult>(value.clone()) {
        Ok(CommandResult::Done) => Ok(()),
        Ok(CommandResult::NotFound) => Err(ReplayOutcome::NotFound),
        Ok(CommandResult::NotVisible) => Err(ReplayOutcome::NotVisible),
        Ok(CommandResult::Error { .. }) => Err(ReplayOutcome::Failed(
            "the sync agent refused the scroll".to_owned(),
        )),
        Ok(CommandResult::Ok { .. }) | Err(_) => Err(ReplayOutcome::Failed(
            "the sync agent's answer is not a scroll result".to_owned(),
        )),
    }
}

/// The expression for the wheel: the replay marker, the agent's `point` for `target` and the
/// visual viewport, answered as the replay's prologue answers (`replay::prologue::parse`).
pub fn point_expression(target: &ElementTarget) -> Result<String, EngineError> {
    let marker = to_json(&AgentCommand::Replay { ms: MARKER_MS })?;
    let point = to_json(&AgentCommand::Point {
        target: target.clone(),
    })?;
    Ok(format!(
        "(()=>{{const agent=globalThis.{AGENT_GLOBAL};\
         if(!agent)return{{error:'no agent'}};\
         agent.command({marker});\
         const resolved=agent.command({point});\
         const v=globalThis.visualViewport;\
         return{{resolved,viewport:v?[v.offsetLeft,v.offsetTop,v.scale]:null}};}})()"
    ))
}

/// `Input.dispatchMouseEvent` `mouseWheel` at (`x`, `y`) with `wheel`'s deltas in CSS px.
pub fn wheel_params(x: f64, y: f64, wheel: &WheelData) -> Value {
    let unit = match wheel.delta_mode {
        1 => LINE_PX,
        2 => PAGE_PX,
        _ => 1.0,
    };
    let delta = |value: f64| {
        let px = value * unit;
        if px.is_finite() {
            px.clamp(-MAX_WHEEL_PX, MAX_WHEEL_PX)
        } else {
            0.0
        }
    };
    json!({
        "type": "mouseWheel",
        "x": x,
        "y": y,
        "deltaX": delta(wheel.delta_x),
        "deltaY": delta(wheel.delta_y),
        "modifiers": wheel.modifiers,
        "button": "none",
        "pointerType": "mouse",
    })
}

/// Turns the wheel in `view` over the element the leader's wheel turned over.
async fn wheel_into(
    port: &dyn ReplayPort,
    view: ViewId,
    wheel: &WheelData,
) -> Result<(), ReplayOutcome> {
    let source = point_expression(&wheel.target).map_err(failed)?;
    let value = port.world(view, source).await.map_err(failed)?;
    let answer = parse(&value).map_err(ReplayOutcome::Failed)?;
    let Some(CommandResult::Ok { x, y, .. }) = answer.resolved else {
        return Err(ReplayOutcome::Failed(
            "the sync agent did not place the wheel".to_owned(),
        ));
    };
    let (x, y) = input_point(x, y, answer.viewport);
    port.send(view, MOUSE, wheel_params(x, y, wheel))
        .await
        .map_err(failed)?;
    Ok(())
}

fn to_json(command: &AgentCommand) -> Result<String, EngineError> {
    serde_json::to_string(command).map_err(|error| EngineError::Invalid(error.to_string()))
}

/// An engine error as a failed replay: CDP's error names the method and its reason.
fn failed(error: EngineError) -> ReplayOutcome {
    ReplayOutcome::Failed(error.to_string())
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
