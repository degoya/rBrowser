//! DevTools (RB-080-43): `devtools_inspect_at`, the context menu's "Inspect element" over MCP -
//! the command `devtools.inspect_at`. It opens DevTools for one view and selects the element
//! under a point in the Elements panel, never one of rBrowser's own page hosts (RB-090-51).
//! `x` and `y` are required here: without a point the command waits for a click in the view,
//! and nobody clicks for an agent. The tool sends `open: true`: `devtools.openOnInspect` is the
//! person's preference for the menu and the shortcut, an agent's call opens DevTools whatever it
//! says (RB-080-45). `devtools_open` lives with the app tools (`app.rs`).

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve_one};

/// `devtools_inspect_at`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectAtParams {
    /// One view: a view id or a breakpoint id shown once; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The point's x, in CSS px of the view's page from its top-left corner.
    pub x: f64,
    /// The point's y, in CSS px of the view's page from its top-left corner.
    pub y: f64,
}

#[tool_router(router = devtools_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Open DevTools for one view and select the element at x, y (CSS px of the page from the viewport's top-left) in the Elements panel, like the context menu's Inspect element; rBrowser's own overlays are passed over. Opens DevTools whatever devtools.openOnInspect says. Answers the view, the DevTools mode, the element's backendNodeId and selected (false in a DevTools window)."
    )]
    pub(crate) async fn devtools_inspect_at(
        &self,
        Parameters(params): Parameters<InspectAtParams>,
    ) -> ToolResult {
        let result = async {
            if ![params.x, params.y]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
            {
                return Err(ToolFailure::new(
                    "args.invalid",
                    "x and y are non-negative numbers".to_owned(),
                ));
            }
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(params.target.as_ref(), &views)?;
            let args = object([
                ("view", view.map(|view| json!(view))),
                ("x", Some(Value::from(params.x))),
                ("y", Some(Value::from(params.y))),
                ("open", Some(Value::Bool(true))),
            ]);
            self.command("devtools.inspect_at", args).await
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "devtools_tests.rs"]
mod tests;
