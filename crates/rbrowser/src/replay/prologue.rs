//! The one evaluation in the target view's agent world before the input (RB-020-03), so a
//! replay costs one round trip before its `Input.*` commands:
//!
//! 1. the **echo marker**: `replay { ms }`, so the view's agent captures nothing the replayed
//!    input causes - a view that captured it would send it back to the others;
//! 2. with `block_submit`, the **submit guard**: a `submit` listener in the capture phase on the
//!    window that cancels every form submit for [`BLOCK_SUBMIT_MS`] - the leader submits, the
//!    view follows by URL (leader-only submit, RB-020-02). It blocks the submit, not the input;
//! 3. `resolve` of the target, when the event has one;
//! 4. `window.visualViewport`, for the pinch zoom of mobile views.
//!
//! It runs in the agent's isolated world: the page neither sees nor can stop it. Every value in
//! it is JSON, so nothing in a target can break out of the expression.

use rb_engine::EngineError;
use rb_sync::{AGENT_GLOBAL, AgentCommand, CommandResult, ElementTarget};
use serde_json::Value;

use super::input::VisualViewport;

/// How long the echo marker holds. The input it covers is handled before CDP answers its
/// command; a view that is not the leader captures nothing anyway.
pub const MARKER_MS: u32 = 300;
/// How long the submit guard holds after the prologue: a replayed click or Enter submits
/// synchronously, while its `Input.*` command runs.
pub const BLOCK_SUBMIT_MS: u32 = 1000;

/// The prologue for `target` (none for keys), with or without the submit guard.
pub fn expression(
    target: Option<&ElementTarget>,
    block_submit: bool,
) -> Result<String, EngineError> {
    let json = |command: &AgentCommand| {
        serde_json::to_string(command).map_err(|error| EngineError::Invalid(error.to_string()))
    };
    let marker = json(&AgentCommand::Replay { ms: MARKER_MS })?;
    let resolve = match target {
        Some(target) => format!(
            "agent.command({})",
            json(&AgentCommand::Resolve {
                target: target.clone()
            })?
        ),
        None => "null".to_owned(),
    };
    let guard = if block_submit {
        submit_guard()
    } else {
        String::new()
    };
    Ok(format!(
        "(()=>{{const agent=globalThis.{AGENT_GLOBAL};\
         if(!agent)return{{error:'no agent'}};\
         agent.command({marker});{guard}\
         const resolved={resolve};\
         const v=globalThis.visualViewport;\
         return{{resolved,viewport:v?[v.offsetLeft,v.offsetTop,v.scale]:null}};}})()"
    ))
}

/// The submit guard: a capture-phase `submit` listener on the window that cancels every form
/// submit for [`BLOCK_SUBMIT_MS`] (also form sync's, RB-020-05: a page may submit on `change`).
pub fn submit_guard() -> String {
    format!(
        "{{const until=Date.now()+{BLOCK_SUBMIT_MS};\
         const block=(event)=>{{if(Date.now()<=until){{event.preventDefault();event.stopImmediatePropagation();}}}};\
         addEventListener('submit',block,true);\
         setTimeout(()=>removeEventListener('submit',block,true),{BLOCK_SUBMIT_MS});}}"
    )
}

/// What the prologue answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The answer to `resolve`; `None` without a target.
    pub resolved: Option<CommandResult>,
    /// The visual viewport, when the page has one.
    pub viewport: Option<VisualViewport>,
}

/// Reads the prologue's value (`Runtime.evaluate`, `returnByValue`). The error never quotes
/// the value.
pub fn parse(value: &Value) -> Result<Answer, String> {
    if value.get("error").is_some() {
        return Err("no sync agent in the view".to_owned());
    }
    let resolved = match value.get("resolved") {
        None | Some(Value::Null) => None,
        Some(resolved) => Some(
            serde_json::from_value(resolved.clone())
                .map_err(|_| "the sync agent's answer is not a command result".to_owned())?,
        ),
    };
    let viewport =
        value
            .get("viewport")
            .and_then(Value::as_array)
            .and_then(|numbers| match numbers.as_slice() {
                [left, top, scale] => Some(VisualViewport {
                    offset_left: left.as_f64()?,
                    offset_top: top.as_f64()?,
                    scale: scale.as_f64()?,
                }),
                _ => None,
            });
    Ok(Answer { resolved, viewport })
}

#[cfg(test)]
#[path = "prologue_tests.rs"]
mod tests;
