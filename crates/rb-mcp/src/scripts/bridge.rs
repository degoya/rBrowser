//! The bridge between a script and the server: every `rb.<tool>(...)` becomes one call through
//! the session's tool router ([`RouterDispatch`]) - the same route, arguments check and guards
//! as the client's own call - after the bridge's checks: no `script_run` inside a script
//! (`script.recursion`), at most `mcp.scripts.maxCalls` calls (`script.call_limit`, which then
//! ends the run's answer whatever the script does with it).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::SystemTime;

use async_trait::async_trait;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResponse, CallToolResult, ErrorCode};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer};
use serde_json::{Map, Value, json};

use super::{ScriptTools, TOOL, codes};
use crate::RbMcpServer;

/// Where the bridge sends a call.
#[async_trait]
pub trait Dispatch: Send + Sync + 'static {
    /// The session serves `tool`.
    fn has_tool(&self, tool: &str) -> bool;

    /// Runs the tool `tool` with `arguments`.
    async fn dispatch(
        &self,
        tool: &str,
        arguments: Map<String, Value>,
    ) -> Result<CallToolResponse, ErrorData>;
}

/// The session's own tool router, with the context of the `script_run` request.
pub struct RouterDispatch {
    server: RbMcpServer,
    context: RequestContext<RoleServer>,
}

impl RouterDispatch {
    /// Calls through `server`'s router in `context`.
    #[must_use]
    pub fn new(server: RbMcpServer, context: RequestContext<RoleServer>) -> Self {
        Self { server, context }
    }
}

#[async_trait]
impl Dispatch for RouterDispatch {
    fn has_tool(&self, tool: &str) -> bool {
        self.server.tool_router.has_route(tool)
    }

    async fn dispatch(
        &self,
        tool: &str,
        arguments: Map<String, Value>,
    ) -> Result<CallToolResponse, ErrorData> {
        let params = CallToolRequestParams::new(tool.to_owned()).with_arguments(arguments);
        let call = ToolCallContext::new(&self.server, params, self.context.clone());
        self.server.tool_router.call(call).await
    }
}

/// One run's bridge.
pub struct Bridge<D> {
    dispatch: D,
    max_calls: u32,
    attempts: AtomicU32,
    dispatched: AtomicU32,
    exceeded: AtomicBool,
}

impl<D: Dispatch> Bridge<D> {
    /// A bridge to `dispatch` allowing `max_calls` calls.
    pub fn new(dispatch: D, max_calls: u32) -> Self {
        Self {
            dispatch,
            max_calls,
            attempts: AtomicU32::new(0),
            dispatched: AtomicU32::new(0),
            exceeded: AtomicBool::new(false),
        }
    }

    /// The calls that reached a tool.
    pub fn calls(&self) -> u32 {
        self.dispatched.load(Ordering::Acquire)
    }

    /// The script asked for more calls than allowed.
    pub fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }
}

#[async_trait]
impl<D: Dispatch> ScriptTools for Bridge<D> {
    async fn call(&self, tool: &str, arguments: &str) -> Result<String, String> {
        if tool == TOOL {
            return Err(refusal(
                codes::RECURSION,
                "script_run cannot be called from a script",
            ));
        }
        let attempt = self
            .attempts
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        if attempt > self.max_calls || self.exceeded() {
            self.exceeded.store(true, Ordering::Release);
            return Err(refusal(
                codes::CALL_LIMIT,
                &format!(
                    "the script made more than {} tool calls (mcp.scripts.maxCalls)",
                    self.max_calls
                ),
            ));
        }
        let arguments = match serde_json::from_str::<Value>(arguments) {
            Ok(Value::Object(map)) => map,
            Ok(Value::Null) => Map::new(),
            _ => {
                return Err(refusal(
                    "invalid_arguments",
                    "the arguments of a tool call are one JSON object",
                ));
            }
        };
        if !self.dispatch.has_tool(tool) {
            return Err(refusal(
                "tool_not_found",
                &format!("this rBrowser has no tool {tool}"),
            ));
        }
        self.dispatched.fetch_add(1, Ordering::AcqRel);
        crate::activity::SERVER.record(SystemTime::now());
        tracing::debug!(tool, "script_run: a tool call from the script");
        answer_of(self.dispatch.dispatch(tool, arguments).await)
    }
}

/// A refusal as the engine reads it.
#[must_use]
pub fn refusal(code: &str, message: &str) -> String {
    json!({ "code": code, "message": message }).to_string()
}

/// A tool's response as the script gets it: the JSON of its text (one block: the value, several:
/// an array; an image as its type and size, never its bytes); an error as a refusal.
fn answer_of(response: Result<CallToolResponse, ErrorData>) -> Result<String, String> {
    match response {
        Ok(CallToolResponse::Complete(result)) => content_of(&result),
        Ok(_) => Err(refusal(
            "script.unsupported_answer",
            "the tool asked the client for input, which a script cannot give",
        )),
        Err(error) => {
            let code = error
                .data
                .as_ref()
                .and_then(|data| data.get("code"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| code_of(error.code).to_owned());
            Err(refusal(&code, &error.message))
        }
    }
}

/// The stable code of a JSON-RPC error the router answered.
fn code_of(code: ErrorCode) -> &'static str {
    if code.0 == ErrorCode::METHOD_NOT_FOUND.0 {
        "tool_not_found"
    } else if code.0 == ErrorCode::INVALID_PARAMS.0 {
        "invalid_arguments"
    } else {
        "tool_failed"
    }
}

fn content_of(result: &CallToolResult) -> Result<String, String> {
    let wire = serde_json::to_value(result).unwrap_or(Value::Null);
    let values: Vec<Value> = wire["content"]
        .as_array()
        .map(|blocks| blocks.iter().map(block_value).collect())
        .unwrap_or_default();
    if wire["isError"].as_bool().unwrap_or(false) {
        let first = values.first().cloned().unwrap_or(Value::Null);
        let code = first["code"].as_str().unwrap_or("tool_failed");
        let message = first["error"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| first.to_string());
        return Err(refusal(code, &message));
    }
    let value = match <[Value; 1]>::try_from(values) {
        Ok([one]) => one,
        Err(values) => Value::Array(values),
    };
    Ok(value.to_string())
}

fn block_value(block: &Value) -> Value {
    match block["type"].as_str() {
        Some("text") => {
            let text = block["text"].as_str().unwrap_or_default();
            serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
        }
        Some("image") => json!({
            "type": "image",
            "mimeType": block["mimeType"],
            "bytes": block["data"].as_str().map_or(0, |data| data.len() / 4 * 3),
        }),
        other => json!({ "type": other }),
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
