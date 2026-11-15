//! JSON-RPC 2.0 messages as rBrowser writes and reads them on the sidecar's stdio (the Node side:
//! `sidecar/src/rpc.ts`): one message per line, UTF-8, `\n` terminated. rBrowser sends requests
//! (numeric ids) and `$/cancel`; the sidecar sends answers and notifications (`$/progress`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::SidecarError;

/// The notification that cancels a running request: `{ "id": <request id> }`.
pub const CANCEL: &str = "$/cancel";
/// The notification that reports progress of a request: `{ "id": <request id>, ... }`.
pub const PROGRESS: &str = "$/progress";
/// The longest line read from the sidecar; a longer one is skipped and logged.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

/// The JSON-RPC error codes rBrowser looks at.
pub mod code {
    /// The request was cancelled (LSP's code).
    pub const REQUEST_CANCELLED: i64 = -32800;
    /// The method is unknown.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The parameters are wrong.
    pub const INVALID_PARAMS: i64 = -32602;
}

/// The error object of an answer.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ErrorObject {
    /// The code.
    pub code: i64,
    /// The message.
    pub message: String,
    /// More detail, if any.
    #[serde(default)]
    pub data: Option<Value>,
}

/// A notification from the sidecar.
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    /// The method.
    pub method: String,
    /// The parameters (`Null` when absent).
    pub params: Value,
}

/// One message read from the sidecar.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// The answer to request `id`.
    Response {
        /// The request id.
        id: u64,
        /// The result, or the error object.
        outcome: Result<Value, ErrorObject>,
    },
    /// A notification.
    Notification(Notification),
}

#[derive(Serialize)]
struct Outgoing<'a> {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    method: &'a str,
    #[serde(skip_serializing_if = "Value::is_null")]
    params: &'a Value,
}

fn line(message: &Outgoing<'_>) -> Result<String, SidecarError> {
    let mut text = serde_json::to_string(message)
        .map_err(|error| SidecarError::Protocol(error.to_string()))?;
    text.push('\n');
    Ok(text)
}

/// A request as a line.
pub fn request_line(id: u64, method: &str, params: &Value) -> Result<String, SidecarError> {
    line(&Outgoing {
        jsonrpc: "2.0",
        id: Some(id),
        method,
        params,
    })
}

/// A notification as a line.
pub fn notification_line(method: &str, params: &Value) -> Result<String, SidecarError> {
    line(&Outgoing {
        jsonrpc: "2.0",
        id: None,
        method,
        params,
    })
}

/// Parses one line from the sidecar. An answer with a `null` id (the sidecar could not read a
/// line of ours) and anything else that is no message of the protocol is an error.
pub fn parse_line(text: &str) -> Result<Incoming, SidecarError> {
    let protocol = |what: &str| SidecarError::Protocol(what.to_owned());
    let value: Value = serde_json::from_str(text).map_err(|_| protocol("a line is not JSON"))?;
    let Value::Object(mut message) = value else {
        return Err(protocol("a message is not an object"));
    };
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(protocol("a message is not JSON-RPC 2.0"));
    }
    if let Some(Value::String(method)) = message.remove("method") {
        if message.contains_key("id") {
            return Err(protocol("the sidecar sent a request"));
        }
        let params = message.remove("params").unwrap_or(Value::Null);
        return Ok(Incoming::Notification(Notification { method, params }));
    }
    let id = match message.get("id") {
        Some(Value::Number(id)) => id.as_u64().ok_or_else(|| protocol("an id is no u64"))?,
        Some(Value::Null) => return Err(unaddressed(&mut message)),
        _ => return Err(protocol("an answer has no id")),
    };
    let outcome = if let Some(result) = message.remove("result") {
        Ok(result)
    } else if let Some(error) = message.remove("error") {
        Err(serde_json::from_value(error).map_err(|_| protocol("a malformed error object"))?)
    } else {
        return Err(protocol("an answer has neither result nor error"));
    };
    Ok(Incoming::Response { id, outcome })
}

/// An error answer without an id: the sidecar could not read one of our lines.
fn unaddressed(message: &mut Map<String, Value>) -> SidecarError {
    let detail = message
        .remove("error")
        .and_then(|error| serde_json::from_value::<ErrorObject>(error).ok())
        .map_or_else(|| "no detail".to_owned(), |error| error.message);
    SidecarError::Protocol(format!("the sidecar could not read a request: {detail}"))
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
