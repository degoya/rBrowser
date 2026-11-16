//! The sync agent in a Firefox view: the preload function that boots the bundle in the sandbox
//! with the view's channel as its binding, the channel names, and the core's commands - the
//! agent's own (`__rbAgent.command`: resolve, scroll, apply, focus, as in Chromium) and the one
//! page operation the replay does itself in the sandbox (blocking a submit). Every value goes in
//! as a JSON literal, so nothing in it can break out of the expression.

use rb_engine::ViewId;
use rb_sync::{AGENT_GLOBAL, AgentCommand, AgentConfig, BINDING_NAME, BOOT_GLOBAL, CommandResult};
use serde::Serialize;

use crate::client::BidiClient;
use crate::error::BidiError;
use crate::script::{evaluate, string_result};

/// The prefix of a view's channel: `rb-agent-<view>`.
pub const CHANNEL_PREFIX: &str = "rb-agent-";

/// The channel of `view`.
pub fn channel(view: ViewId) -> String {
    format!("{CHANNEL_PREFIX}{}", view.0)
}

/// The view of a channel name; `None` for a channel that is not an agent's.
pub fn view_of(channel: &str) -> Option<ViewId> {
    channel
        .strip_prefix(CHANNEL_PREFIX)?
        .parse()
        .ok()
        .map(ViewId)
}

/// The preload function: the channel becomes the binding `__rbEmit`, the configuration
/// `__rbAgentBoot`, then the bundle boots - all in the sandbox. `None` without a bundle.
pub fn preload_source(bundle: &str, config: &AgentConfig) -> Option<String> {
    if bundle.trim().is_empty() {
        return None;
    }
    let config = serde_json::to_string(config).ok()?;
    Some(format!(
        "(emit) => {{\n\"use strict\";\nglobalThis.{BINDING_NAME} = (payload) => {{ emit(payload); }};\nglobalThis.{BOOT_GLOBAL} = {config};\n{bundle}\n}}"
    ))
}

/// Runs `command` in the agent of `context`'s top frame.
pub async fn command(
    client: &BidiClient,
    context: &str,
    command: &AgentCommand,
) -> Result<CommandResult, BidiError> {
    let json = json_of(command)?;
    let expression = format!("JSON.stringify(globalThis.{AGENT_GLOBAL}.command({json}))");
    let answer = client.send(evaluate(context, true, &expression)).await?;
    let text = string_result(&answer)?;
    serde_json::from_str(&text).map_err(|_| BidiError::Protocol {
        method: "script.evaluate".to_owned(),
        reason: "the agent's answer is not a command result".to_owned(),
    })
}

/// What a page operation answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAnswer {
    /// Done.
    Ok,
    /// The element is not there.
    NotFound,
}

/// No form submits for `d.ms` (the leader submits; the view follows by URL).
pub const BLOCK_SUBMIT: &str = "const stop=(e)=>{e.preventDefault();e.stopImmediatePropagation();};window.addEventListener('submit',stop,true);setTimeout(()=>window.removeEventListener('submit',stop,true),d.ms);return 'ok';";

/// Runs the page operation `body` with `data` as `d` in the sandbox of `context`'s top frame.
pub async fn page(
    client: &BidiClient,
    context: &str,
    body: &str,
    data: &impl Serialize,
) -> Result<PageAnswer, BidiError> {
    let json = json_of(data)?;
    let expression = format!("((d)=>{{{body}}})({json})");
    let answer = client.send(evaluate(context, true, &expression)).await?;
    match string_result(&answer)?.as_str() {
        "ok" => Ok(PageAnswer::Ok),
        "not_found" => Ok(PageAnswer::NotFound),
        _ => Err(BidiError::Protocol {
            method: "script.evaluate".to_owned(),
            reason: "unexpected answer of a page operation".to_owned(),
        }),
    }
}

fn json_of(value: &impl Serialize) -> Result<String, BidiError> {
    serde_json::to_string(value).map_err(|_| BidiError::Protocol {
        method: "script.evaluate".to_owned(),
        reason: "the command cannot be written as JSON".to_owned(),
    })
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
