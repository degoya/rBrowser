//! Flows (RB-040-07): `flow_list` and `flow_run`. A flow is project code: it runs only in a
//! workspace a person trusted, and no tool grants that trust - an untrusted project answers
//! `flows.untrusted` with the way to the trust dialog. Cancelling a run is the person's
//! (`flows.cancel` has no tool).

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::run_target::{RunTarget, Split};
use crate::target::{Resolved, resolve};

/// `flow_list` takes nothing.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowListParams {}

/// `flow_run`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowRunParams {
    /// The flow's name, as `flow_list` gives it (its file name without `.ts`).
    pub name: String,
    /// `"all"`, a view id or a breakpoint id; when absent the views the flow names in its `//
    /// @views` comment, else all. Also `"visible"` (the views the grid shows) or a list of view ids
    /// and breakpoint ids.
    #[serde(default)]
    pub target: Option<RunTarget>,
    /// Wait for the result (the default); `false` answers the `runId` at once and the result
    /// lands in `.rbrowser/out/flows/<runId>/result.json` (`flow_list` shows the run going on).
    #[serde(default)]
    pub wait: Option<bool>,
}

#[tool_router(router = flows_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The project's flows (.rbrowser/flows/*.ts: Playwright scripts with page, view, expect, env and step injected) with description and views, whether the workspace is trusted (flows run only then), the run going on with each view's status, and the last run's result file."
    )]
    pub(crate) async fn flow_list(
        &self,
        Parameters(_params): Parameters<FlowListParams>,
    ) -> ToolResult {
        respond(self.backend.flow_list().await)
    }

    #[tool(
        description = "Run a flow in every chosen view at once. Answers per view passed, failed, skipped (Firefox and WebKit, with the reason) or timedOut, with duration, steps, the error with its step and line and a screenshot on failure - the result.json of `rbrowser flow run`. Refused with flows.untrusted until a person trusts the workspace; one run at a time (flows.busy), none while recording (flows.recording)."
    )]
    pub(crate) async fn flow_run(
        &self,
        Parameters(params): Parameters<FlowRunParams>,
    ) -> ToolResult {
        let result = async {
            let (views, scope) = match self.split_target(params.target.as_ref()).await? {
                Split::Target(None) => (None, None),
                Split::Target(Some(target)) => {
                    let views = self.read(|backend| backend.views()).await?;
                    match resolve(Some(&target), &views)? {
                        Resolved::All => (Some(views.iter().map(|view| view.id).collect()), None),
                        Resolved::Views(ids) => (Some(ids), None),
                    }
                }
                // RB-099-10: the app resolves the scope when the run starts.
                Split::Scope(scope) => (None, Some(scope)),
            };
            self.backend
                .flow_run(&params.name, views, scope, params.wait.unwrap_or(true))
                .await
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "flows_tests.rs"]
mod tests;
