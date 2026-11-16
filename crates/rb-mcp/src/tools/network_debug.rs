//! The HTTP debugger (RB-080-37), run by the host (`crates/rbrowser/src/mcp/network_tools.rs`):
//! `network_breakpoints`, `network_continue`, `network_abort`, `network_compose` and
//! `network_repeat`. A continue with edits answers only while `mcp.allowEvaluate` is on; a
//! compose or repeat only while `network-panel.allowComposeFromMcp` is on (they send with the
//! person's cookies). Every answer is masked: no cookie, no token.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The host tools.
pub const BREAKPOINTS_TOOL: &str = "network_breakpoints";
/// `network_continue`.
pub const CONTINUE_TOOL: &str = "network_continue";
/// `network_abort`.
pub const ABORT_TOOL: &str = "network_abort";
/// `network_compose`.
pub const COMPOSE_TOOL: &str = "network_compose";
/// `network_repeat`.
pub const REPEAT_TOOL: &str = "network_repeat";

/// What `network_breakpoints` does.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum BreakpointAction {
    /// The breakpoints and the held requests.
    #[default]
    List,
    /// Add one (`urlPattern` required).
    Add,
    /// Remove one (`id` required).
    Remove,
    /// Switch one on or off (`id` and `enabled` required).
    Enable,
}

/// `network_breakpoints`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BreakpointsParams {
    /// `list` (default), `add`, `remove` or `enable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<BreakpointAction>,
    /// The breakpoint's id (remove, enable; optional for add).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The URL glob (`*` any characters, `?` one), or a regular expression with `regex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regex: Option<bool>,
    /// `GET`, `POST`, ...; any when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// CDP's resource type (`XHR`, `Fetch`, `Document`, ...); any when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    /// `request` (default) or `response`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    /// Only these views (ids); every view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<Vec<u32>>,
    /// For `enable`; also the starting state of `add` (on by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// The edits of a continue or a repeat.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The whole header list, `[[name, value], ...]`; a masked value (`***`) means as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,
    /// The body as text (`bodyBase64` for bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    /// The response's status (response stage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

/// `network_continue`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinueParams {
    /// The held request (`network_breakpoints` lists them).
    pub id: String,
    /// The same request held in every view too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_views: Option<bool>,
    /// What changes before it goes on; only while `mcp.allowEvaluate` is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditsParams>,
}

/// `network_abort`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AbortParams {
    /// The held request.
    pub id: String,
    /// The same request held in every view too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_views: Option<bool>,
}

/// A request to send.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestParams {
    /// `GET` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// An `http(s)` URL.
    pub url: String,
    /// `[[name, value], ...]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
}

/// `network_compose`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComposeParams {
    pub request: RequestParams,
    /// The view whose cookies, user agent and headers the request takes; none when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<u32>,
    /// How often to send, 1 to 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<u32>,
}

/// `network_repeat`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepeatParams {
    /// The view that saw the request.
    pub view: u32,
    /// Its request id (`network_requests`, `networkpanel_list`).
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<EditsParams>,
    /// How often to send, 1 to 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<u32>,
}

#[tool_router(router = network_debug_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The HTTP debugger's breakpoints over every Chromium view: action list (default) answers the breakpoints, their switch (enabled) and the requests held right now - each with id, view, stage (request: method, URL, headers and body editable; response: status, headers and body), masked, the body only while mcp.allowEvaluate is on (bodyWithheld otherwise) - and timeoutMs, after which a held request goes on unchanged. action add (urlPattern as glob or regex, method, resourceType, stage, views), remove (id) and enable (id, enabled) change them; a breakpoint holds the matching request of every view it covers until network_continue or network_abort, or the timeout."
    )]
    pub(crate) async fn network_breakpoints(
        &self,
        Parameters(params): Parameters<BreakpointsParams>,
    ) -> ToolResult {
        respond(self.host_masked(BREAKPOINTS_TOOL, &params).await)
    }

    #[tool(
        description = "Let a held request go on (id from network_breakpoints). allViews: the same request held in every other view too. edits (method, url, headers as [[name, value]], body or bodyBase64, status at the response stage) only while mcp.allowEvaluate is on - an edited request or response is content the page gets; a masked value sent back means as it was. Answers how many requests went on and how many stay held."
    )]
    pub(crate) async fn network_continue(
        &self,
        Parameters(params): Parameters<ContinueParams>,
    ) -> ToolResult {
        respond(self.host_masked(CONTINUE_TOOL, &params).await)
    }

    #[tool(
        description = "Fail a held request (id from network_breakpoints) as aborted; allViews: the same request held in every other view too."
    )]
    pub(crate) async fn network_abort(
        &self,
        Parameters(params): Parameters<AbortParams>,
    ) -> ToolResult {
        respond(self.host_masked(ABORT_TOOL, &params).await)
    }

    #[tool(
        description = "Send a request of your own (method, url, headers, body) from rBrowser's core - with the cookies, user agent and extra headers of view when given - times in a row (1 to 10, one after another), and answer each result: status, headers, body (cut at 256 KiB), timeMs; a redirect is answered (location), never followed. Off by default: a person switches it on with network-panel.allowComposeFromMcp, because it sends with the person's cookies; while off the answer is an error with code compose_disabled and settingsKey. Answers are masked."
    )]
    pub(crate) async fn network_compose(
        &self,
        Parameters(params): Parameters<ComposeParams>,
    ) -> ToolResult {
        respond(self.host_masked(COMPOSE_TOOL, &params).await)
    }

    #[tool(
        description = "Send a captured request again (view and requestId from network_requests or networkpanel_list) from that view's context, with edits (method, url, headers, body), times in a row (1 to 10), and answer each result as network_compose does. Off by default (network-panel.allowComposeFromMcp, code compose_disabled). Answers are masked."
    )]
    pub(crate) async fn network_repeat(
        &self,
        Parameters(params): Parameters<RepeatParams>,
    ) -> ToolResult {
        respond(self.host_masked(REPEAT_TOOL, &params).await)
    }
}

impl RbMcpServer {
    async fn host_masked<T: Serialize>(
        &self,
        tool: &str,
        params: &T,
    ) -> Result<Value, ToolFailure> {
        let args = serde_json::to_value(params)
            .map_err(|error| ToolFailure::new("internal", error.to_string()))?;
        let answer = self.backend.host_tool(tool, args).await?;
        Ok(self.backend.mask(&answer))
    }
}

/// The breakpoint fields of `params` as the host takes them (`list` sends none).
#[must_use]
pub fn breakpoint_fields(params: &BreakpointsParams) -> Value {
    object([
        ("id", params.id.clone().map(Value::String)),
        ("urlPattern", params.url_pattern.clone().map(Value::String)),
        ("regex", params.regex.map(Value::Bool)),
        ("method", params.method.clone().map(Value::String)),
        (
            "resourceType",
            params.resource_type.clone().map(Value::String),
        ),
        ("stage", params.stage.clone().map(Value::String)),
        ("views", params.views.clone().map(|views| json!(views))),
        ("enabled", params.enabled.map(Value::Bool)),
    ])
}

#[cfg(test)]
#[path = "network_debug_tests.rs"]
mod tests;
