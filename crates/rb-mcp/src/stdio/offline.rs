//! What `rbrowser mcp` answers itself while no rBrowser is reachable (RB-030-02, K7): the client
//! connects (`initialize` and `ping` succeed, notifications are taken), and every other request
//! gets a JSON-RPC error that says why and what to do - never silence, never a closed pipe. The
//! next message looks for the instance again, so a client keeps working once rBrowser runs.

use rmcp::model::ProtocolVersion;
use serde_json::{Value, json};

/// The protocol version answered when the client names none.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// The JSON-RPC error code of "no instance" (the implementation-defined server range).
pub const NO_INSTANCE_CODE: i64 = -32000;
/// The stable code in the error's `data`.
pub const NO_INSTANCE: &str = "mcp.no_instance";

/// The answer to `message` (one JSON-RPC line) while no instance is reachable for `why`;
/// `None` for a notification.
#[must_use]
pub fn answer(message: &str, why: &str) -> Option<Value> {
    let Ok(request) = serde_json::from_str::<Value>(message) else {
        return Some(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32700, "message": "parse error: the line is not JSON" },
        }));
    };
    let id = request.get("id").filter(|id| !id.is_null())?.clone();
    let method = request["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => initialize(&request, why),
        "ping" => json!({}),
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": NO_INSTANCE_CODE,
                    "message": why,
                    "data": {
                        "code": NO_INSTANCE,
                        (crate::codes::LEGACY_FIELD): crate::codes::legacy(NO_INSTANCE),
                    },
                },
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// The `initialize` result: the protocol version ([`negotiate`]), the tools capability - with
/// `listChanged`, which the bridge sends once it reaches rBrowser -, the prompts and resources
/// the server declares (RB-095-01) and the state in the instructions.
fn initialize(request: &Value, why: &str) -> Value {
    json!({
        "protocolVersion": negotiate(request["params"]["protocolVersion"].as_str()),
        "capabilities": { "tools": { "listChanged": true }, "prompts": {}, "resources": {} },
        "serverInfo": { "name": crate::instance::APP, "version": env!("CARGO_PKG_VERSION") },
        "instructions": format!(
            "rBrowser is not reachable yet: {why}. Tool calls fail until it runs; this bridge \
             finds it at the next call."
        ),
    })
}

/// The client's protocol version when the server takes it with an `initialize` handshake, else
/// [`PROTOCOL_VERSION`]: the bridge later sends the answered version as `MCP-Protocol-Version`,
/// and the server refuses one it does not know with HTTP 400.
fn negotiate(asked: Option<&str>) -> &str {
    asked
        .filter(|asked| {
            ProtocolVersion::KNOWN_VERSIONS
                .iter()
                .any(|known| known.as_str() == *asked && known.has_initialize())
        })
        .unwrap_or(PROTOCOL_VERSION)
}

#[cfg(test)]
#[path = "offline_tests.rs"]
mod tests;
