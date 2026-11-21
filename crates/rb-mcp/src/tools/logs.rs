//! What the pages reported: `console_logs`, `network_requests`, `errors` - the per-view ring
//! buffers of [`crate::logs`], newest last, masked by the host.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::logs::LogKind;
use crate::target::{TargetArg, resolve};

/// The default `limit`.
const LIMIT: usize = 100;

/// `console_logs`, `network_requests`, `errors`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The most entries per view, the newest (default 100).
    #[serde(default)]
    pub limit: Option<usize>,
    /// `console_logs`: only this level (`error`, `warning`, `log`, ...).
    #[serde(default)]
    pub level: Option<String>,
    /// `network_requests`: only failed requests and HTTP errors (status 400 and up).
    #[serde(default)]
    pub failed_only: bool,
    /// Empty the buffers after reading.
    #[serde(default)]
    pub clear: bool,
}

#[tool_router(router = logs_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The console messages of the targeted views since the MCP server started (or the last clear): level, source, text, url, line, time (ms since epoch); newest last. level filters, limit caps per view (default 100), clear empties the buffer. Buffer size: setting mcp.consoleLogLimit."
    )]
    pub(crate) async fn console_logs(
        &self,
        Parameters(params): Parameters<LogParams>,
    ) -> ToolResult {
        self.logs(LogKind::Console, params).await
    }

    #[tool(
        description = "The network requests of the targeted views: url, method, resourceType, status, mimeType, fromCache, size, decodedSize, time, durationMs, failed, response (ttfbMs, encoding, cacheControl, expires, etag, lastModified) - never headers or bodies; newest last. failedOnly keeps failures and HTTP errors, limit caps per view (default 100), clear empties the buffer. Buffer size: setting mcp.networkRequestLimit. Requests are collected from the first call on (from the start with the setting mcp.collectNetwork): a first call answers nothing yet - reload the page, then call again."
    )]
    pub(crate) async fn network_requests(
        &self,
        Parameters(params): Parameters<LogParams>,
    ) -> ToolResult {
        self.logs(LogKind::Network, params).await
    }

    #[tool(
        description = "The uncaught exceptions of the targeted views: text, url, line, column, time; newest last. limit caps per view (default 100), clear empties the buffer. Buffer size: setting mcp.errorLimit."
    )]
    pub(crate) async fn errors(&self, Parameters(params): Parameters<LogParams>) -> ToolResult {
        self.logs(LogKind::Errors, params).await
    }
}

impl RbMcpServer {
    async fn logs(&self, kind: LogKind, params: LogParams) -> ToolResult {
        let views = match self.read(|backend| backend.views()).await {
            Ok(views) => views,
            Err(failure) => return respond::<Value>(Err(failure)),
        };
        let ids = match resolve(params.target.as_ref(), &views) {
            Ok(resolved) => resolved.view_ids(&views),
            Err(failure) => return respond::<Value>(Err(failure)),
        };
        let store = self.backend.logs();
        let limit = params.limit.unwrap_or(LIMIT);
        let answer: Vec<Value> = ids
            .into_iter()
            .map(|view| {
                // Filtered before the limit, so `limit` counts what is answered.
                let all = usize::MAX;
                let entries: Vec<Value> = match kind {
                    LogKind::Console => store
                        .console(view, all)
                        .into_iter()
                        .filter(|entry| {
                            params
                                .level
                                .as_deref()
                                .is_none_or(|level| entry.level == level)
                        })
                        .map(|entry| json!(entry))
                        .collect(),
                    LogKind::Network => store
                        .network(view, all)
                        .into_iter()
                        .filter(|entry| {
                            !params.failed_only
                                || entry.failed.is_some()
                                || entry.status.is_some_and(|status| status >= 400)
                        })
                        .map(|entry| json!(entry))
                        .collect(),
                    LogKind::Errors => store
                        .errors(view, all)
                        .into_iter()
                        .map(|entry| json!(entry))
                        .collect(),
                };
                if params.clear {
                    store.clear(view, kind);
                }
                let skip = entries.len().saturating_sub(limit);
                let breakpoint = views
                    .iter()
                    .find(|info| info.id == view)
                    .and_then(|info| info.breakpoint.clone());
                self.backend.mask(&json!({
                    "view": view,
                    "breakpoint": breakpoint,
                    "entries": entries.into_iter().skip(skip).collect::<Vec<_>>(),
                }))
            })
            .collect();
        respond::<Value>(Ok(Value::Array(answer)))
    }
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
