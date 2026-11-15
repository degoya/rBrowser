//! The one evaluation in a follower's agent world that applies a form action (RB-020-05): the
//! echo marker, with `block_submit` the submit guard (a page may submit its form on `change`;
//! leader-only submit holds), then the agent's `apply` or `focus`. Every value in it is JSON,
//! so nothing in a value or target can break out of the expression. Neither the expression nor
//! the agent's answer is ever logged or quoted: a value may be a password.

use rb_engine::EngineError;
use rb_sync::{AGENT_GLOBAL, AgentCommand, CommandResult, ReplayOutcome};
use serde_json::Value;

use crate::replay::prologue::{MARKER_MS, submit_guard};

/// The expression for `command`, with or without the submit guard. It answers
/// `{ applied: <CommandResult> }`, or `{ error }` without an agent.
pub fn expression(command: &AgentCommand, block_submit: bool) -> Result<String, EngineError> {
    let json = |command: &AgentCommand| {
        serde_json::to_string(command).map_err(|error| EngineError::Invalid(error.to_string()))
    };
    let marker = json(&AgentCommand::Replay { ms: MARKER_MS })?;
    let command = json(command)?;
    let guard = if block_submit {
        submit_guard()
    } else {
        String::new()
    };
    Ok(format!(
        "(()=>{{const agent=globalThis.{AGENT_GLOBAL};\
         if(!agent)return{{error:'no agent'}};\
         agent.command({marker});{guard}\
         return{{applied:agent.command({command})}};}})()"
    ))
}

/// The replay outcome of the expression's value. A refusal is `Failed` with a fixed text.
pub fn outcome_of(value: &Value) -> ReplayOutcome {
    if value.get("error").is_some() {
        return ReplayOutcome::Failed("no sync agent in the view".to_owned());
    }
    let answer = value
        .get("applied")
        .cloned()
        .and_then(|applied| serde_json::from_value::<CommandResult>(applied).ok());
    match answer {
        Some(CommandResult::Done) => ReplayOutcome::Done,
        Some(CommandResult::NotFound) => ReplayOutcome::NotFound,
        Some(CommandResult::NotVisible) => ReplayOutcome::NotVisible,
        Some(CommandResult::Error { .. }) => {
            ReplayOutcome::Failed("the sync agent refused the form state".to_owned())
        }
        Some(CommandResult::Ok { .. }) | None => {
            ReplayOutcome::Failed("the sync agent's form answer is not a result".to_owned())
        }
    }
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
