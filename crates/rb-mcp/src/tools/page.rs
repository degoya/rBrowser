//! The page: `evaluate` (page JavaScript in the main world, off unless `mcp.allowEvaluate`) and
//! `snapshot` (the accessibility tree). Both answer per view, masked by the host and cut at
//! `mcp.maxAnswerBytes`.

use std::collections::HashMap;

use rb_engine::{ViewId, ViewInfo};
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, capped, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve};

/// The setting that allows `evaluate`.
pub const ALLOW_EVALUATE: &str = "mcp.allowEvaluate";
/// The code of `evaluate` while it is off.
pub const EVALUATE_DISABLED: &str = "evaluate_disabled";
/// The default and the most of `evaluate`'s `timeoutMs`.
const EVALUATE_TIMEOUT: (u32, u32) = (10_000, 60_000);
/// The default `maxNodes` of `snapshot`.
const SNAPSHOT_NODES: usize = 1000;

/// `evaluate`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluateParams {
    /// A JavaScript expression, run in the page's main world.
    pub expression: String,
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// Wait for a promise the expression returns (default true).
    #[serde(default)]
    pub await_promise: Option<bool>,
    /// How long it may run, in milliseconds (default 10000, at most 60000).
    #[serde(default)]
    pub timeout_ms: Option<u32>,
}

/// `snapshot`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The most nodes per view (default 1000).
    #[serde(default)]
    pub max_nodes: Option<usize>,
}

#[tool_router(router = page_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Run a JavaScript expression in the page of the targeted views (main world) and answer each view's value (by value; promises awaited). Off by default: a person switches it on with the setting mcp.allowEvaluate; while off the answer is an error with code evaluate_disabled and settingsKey. Answers are masked and cut at mcp.maxAnswerBytes. target: \"all\" (default), a view id or a breakpoint id."
    )]
    pub(crate) async fn evaluate(
        &self,
        Parameters(params): Parameters<EvaluateParams>,
    ) -> ToolResult {
        let options = self.backend.options();
        if !options.allow_evaluate {
            return respond::<Value>(Err(ToolFailure::new(
                EVALUATE_DISABLED,
                "evaluate is switched off; a person can switch it on in rBrowser at Settings > \
                 Security & Privacy > Evaluate (rbrowser://settings/mcp.allowEvaluate)",
            )
            .with_setting(ALLOW_EVALUATE)));
        }
        let timeout = params
            .timeout_ms
            .unwrap_or(EVALUATE_TIMEOUT.0)
            .clamp(1, EVALUATE_TIMEOUT.1);
        let request = json!({
            "expression": params.expression,
            "returnByValue": true,
            "awaitPromise": params.await_promise.unwrap_or(true),
            "userGesture": false,
            "timeout": timeout,
        });
        let result = self
            .per_view(params.target.as_ref(), |view, info| {
                let request = request.clone();
                async move {
                    let entry = match self.backend.cdp(view, "Runtime.evaluate", request).await {
                        Ok(answer) => evaluated(&answer),
                        Err(failure) => json!({ "error": failure.message }),
                    };
                    with_view(entry, info)
                }
            })
            .await;
        respond(result.map(|views| self.finish(views, options.max_answer_bytes)))
    }

    #[tool(
        description = "The accessibility tree of the targeted views (CDP Accessibility.getFullAXTree), trimmed to the nodes that matter: id, parent, role, name, value. maxNodes caps each view (default 1000); answers are cut at mcp.maxAnswerBytes. target: \"all\" (default), a view id or a breakpoint id."
    )]
    pub(crate) async fn snapshot(
        &self,
        Parameters(params): Parameters<SnapshotParams>,
    ) -> ToolResult {
        let options = self.backend.options();
        let max_nodes = params.max_nodes.unwrap_or(SNAPSHOT_NODES).max(1);
        let result = self
            .per_view(params.target.as_ref(), |view, info| async move {
                let entry = match self
                    .backend
                    .cdp(view, "Accessibility.getFullAXTree", json!({}))
                    .await
                {
                    Ok(answer) => trim_tree(&answer, max_nodes),
                    Err(failure) => json!({ "error": failure.message }),
                };
                with_view(entry, info)
            })
            .await;
        respond(result.map(|views| self.finish(views, options.max_answer_bytes)))
    }
}

impl RbMcpServer {
    /// Runs `each` for every view `target` names, in grid order.
    pub(crate) async fn per_view<F, Fut>(
        &self,
        target: Option<&TargetArg>,
        each: F,
    ) -> Result<Vec<Value>, ToolFailure>
    where
        F: Fn(ViewId, ViewInfo) -> Fut,
        Fut: std::future::Future<Output = Value>,
    {
        let views = self.read(|backend| backend.views()).await?;
        let ids = resolve(target, &views)?.view_ids(&views);
        let mut answers = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(info) = views.iter().find(|view| view.id == id) {
                answers.push(each(id, info.clone()).await);
            }
        }
        Ok(answers)
    }

    /// The answers masked and each cut at its share of `max_bytes`.
    fn finish(&self, views: Vec<Value>, max_bytes: usize) -> Value {
        let share = max_bytes / views.len().max(1);
        Value::Array(
            views
                .into_iter()
                .map(|view| capped(self.backend.mask(&view), share))
                .collect(),
        )
    }
}

fn with_view(mut entry: Value, info: ViewInfo) -> Value {
    entry["view"] = json!(info.id);
    entry["breakpoint"] = json!(info.breakpoint);
    entry
}

/// `Runtime.evaluate`'s answer as `{type, value}` or `{error}`.
pub fn evaluated(answer: &Value) -> Value {
    if let Some(details) = answer.get("exceptionDetails") {
        let text = details["exception"]["description"]
            .as_str()
            .or_else(|| details["text"].as_str())
            .unwrap_or("the expression threw");
        return json!({ "error": text });
    }
    let result = &answer["result"];
    let value = match result.get("value") {
        Some(value) => value.clone(),
        None => result
            .get("unserializableValue")
            .or_else(|| result.get("description"))
            .cloned()
            .unwrap_or(Value::Null),
    };
    json!({ "type": result["type"], "value": value })
}

/// The nodes of `Accessibility.getFullAXTree` without the ignored ones and the inline text
/// boxes, each with its nearest kept ancestor as parent; at most `max_nodes`.
pub fn trim_tree(answer: &Value, max_nodes: usize) -> Value {
    let empty = Vec::new();
    let nodes: &[Value] = answer["nodes"].as_array().unwrap_or(&empty);
    let kept = |node: &Value| {
        !node["ignored"].as_bool().unwrap_or(false)
            && node["role"]["value"].as_str() != Some("InlineTextBox")
    };
    let parents: HashMap<&str, (Option<&str>, bool)> = nodes
        .iter()
        .filter_map(|node| {
            let id = node["nodeId"].as_str()?;
            Some((id, (node["parentId"].as_str(), kept(node))))
        })
        .collect();
    let all: Vec<Value> = nodes
        .iter()
        .filter(|node| kept(node))
        .map(|node| {
            let mut out = json!({
                "id": node["nodeId"],
                "role": node["role"]["value"],
            });
            if let Some(parent) = kept_parent(&parents, node["parentId"].as_str(), nodes.len()) {
                out["parent"] = json!(parent);
            }
            for field in ["name", "value", "description"] {
                if let Some(text) = node[field]["value"]
                    .as_str()
                    .filter(|text| !text.is_empty())
                {
                    out[field] = json!(text);
                }
            }
            out
        })
        .collect();
    let total = all.len();
    let nodes: Vec<Value> = all.into_iter().take(max_nodes).collect();
    json!({ "nodes": nodes, "total": total, "truncated": total > max_nodes })
}

/// The nearest ancestor from `parent` up that is kept; at most `limit` steps (no cycles).
fn kept_parent<'a>(
    parents: &HashMap<&'a str, (Option<&'a str>, bool)>,
    mut parent: Option<&'a str>,
    limit: usize,
) -> Option<String> {
    for _ in 0..=limit {
        let id = parent?;
        match parents.get(id) {
            Some((_, true)) => return Some(id.to_owned()),
            Some((up, false)) => parent = *up,
            None => return None,
        }
    }
    None
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
