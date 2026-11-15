//! `script.*`: the preload script in a sandbox realm (the sync agent's isolated world, ADR 0001
//! Q5), evaluation in that sandbox or the page, and the `script.message` channel the agent
//! reports through.

use serde_json::{Value, json};

use crate::error::BidiError;
use crate::protocol::{Command, string_value};

/// The sandbox the agent lives in: the same name as Chromium's isolated world.
pub const SANDBOX: &str = rb_sync::WORLD_NAME;
/// A preload script sent something over its channel.
pub const MESSAGE_EVENT: &str = "script.message";

/// Runs `function` in the sandbox of every new document of `contexts` (and their frames), with
/// the channel `channel` as its one argument.
pub fn add_preload_script(function: &str, channel: &str, contexts: &[&str]) -> Command {
    Command::call(
        "script.addPreloadScript",
        json!({
            "functionDeclaration": function,
            "arguments": [{ "type": "channel", "value": { "channel": channel } }],
            "sandbox": SANDBOX,
            "contexts": contexts,
        }),
    )
}

/// Evaluates `expression` in `context`'s top frame: in the sandbox, or in the page.
pub fn evaluate(context: &str, sandbox: bool, expression: &str) -> Command {
    let target = if sandbox {
        json!({ "context": context, "sandbox": SANDBOX })
    } else {
        json!({ "context": context })
    };
    Command::call(
        "script.evaluate",
        json!({
            "expression": expression,
            "target": target,
            "awaitPromise": false,
            "resultOwnership": "none",
        }),
    )
}

/// The string an evaluation returned. An exception is an error that does not quote the page.
pub fn string_result(result: &Value) -> Result<String, BidiError> {
    let failed = |reason: &str| BidiError::Protocol {
        method: "script.evaluate".to_owned(),
        reason: reason.to_owned(),
    };
    match result.get("type").and_then(Value::as_str) {
        Some("success") => result
            .get("result")
            .and_then(string_value)
            .map(str::to_owned)
            .ok_or_else(|| failed("the script returned no string")),
        Some("exception") => Err(failed("the script threw")),
        _ => Err(failed("no evaluation result")),
    }
}

/// What a preload script sent: its channel, the context of the realm, the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelMessage {
    /// The channel (one per view).
    pub channel: String,
    /// The browsing context of the realm that sent it (a frame's own context for an iframe).
    pub context: String,
    /// The text; untrusted.
    pub data: String,
}

/// The [`ChannelMessage`] of a `script.message` event whose data is a string.
pub fn channel_message(method: &str, params: &Value) -> Option<ChannelMessage> {
    if method != MESSAGE_EVENT {
        return None;
    }
    Some(ChannelMessage {
        channel: params.get("channel")?.as_str()?.to_owned(),
        context: params.get("source")?.get("context")?.as_str()?.to_owned(),
        data: string_value(params.get("data")?)?.to_owned(),
    })
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
