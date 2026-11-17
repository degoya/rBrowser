//! The WebDriver BiDi wire format: a command frame out, and what comes in - the answer to a
//! command (by its id), an error the remote end could not route, or an event. Pure, without a
//! socket. Incoming frames are untrusted text: an error names the position, never the frame.

use serde_json::{Value, json};

/// One command to send: its method and parameters. `Debug` shows the method only: parameters
/// carry replayed form values and cookie values.
#[derive(Clone, PartialEq)]
pub struct Command {
    /// The method (`browsingContext.navigate`).
    pub method: &'static str,
    /// Its parameters.
    pub params: Value,
}

impl Command {
    /// `method` with `params`.
    pub fn call(method: &'static str, params: Value) -> Self {
        Self { method, params }
    }
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

/// An error the remote end answered: BiDi's `error` code and `message`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The error code (`no such frame`, `unknown command`, `unsupported operation`).
    pub error: String,
    /// The remote end's explanation.
    pub message: String,
}

/// An event: `method` (`script.message`) and its `params`.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// The event name.
    pub method: String,
    /// Its parameters.
    pub params: Value,
}

/// One incoming frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// The answer to the command `id`.
    Answer {
        /// The command's id.
        id: u64,
        /// Its result, or the error.
        outcome: Result<Value, Failure>,
    },
    /// An error without a command id (the remote end could not read a frame).
    Unrouted(Failure),
    /// An event.
    Event(Event),
}

/// The frame of the command `id`.
pub fn command_frame(id: u64, method: &str, params: Value) -> String {
    json!({ "id": id, "method": method, "params": params }).to_string()
}

/// Reads one incoming frame. The error never quotes the frame.
pub fn parse_incoming(text: &str) -> Result<Incoming, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| {
        format!(
            "not JSON (line {}, column {})",
            error.line(),
            error.column()
        )
    })?;
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let id = value.get("id");
    match kind {
        "success" => {
            let id = id
                .and_then(Value::as_u64)
                .ok_or_else(|| "a success without a numeric id".to_owned())?;
            let result = value.get("result").cloned().unwrap_or(Value::Null);
            Ok(Incoming::Answer {
                id,
                outcome: Ok(result),
            })
        }
        "error" => {
            let failure = Failure {
                error: text_of(&value, "error"),
                message: text_of(&value, "message"),
            };
            match id.and_then(Value::as_u64) {
                Some(id) => Ok(Incoming::Answer {
                    id,
                    outcome: Err(failure),
                }),
                None => Ok(Incoming::Unrouted(failure)),
            }
        }
        "event" => {
            let method = value
                .get("method")
                .and_then(Value::as_str)
                .ok_or_else(|| "an event without a method".to_owned())?;
            Ok(Incoming::Event(Event {
                method: method.to_owned(),
                params: value.get("params").cloned().unwrap_or(Value::Null),
            }))
        }
        _ => Err("a frame of no known type".to_owned()),
    }
}

/// The text of a BiDi `string` remote value (`{ "type": "string", "value": ... }`).
pub fn string_value(value: &Value) -> Option<&str> {
    if value.get("type").and_then(Value::as_str) != Some("string") {
        return None;
    }
    value.get("value").and_then(Value::as_str)
}

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
