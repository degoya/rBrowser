//! Flows (RB-040-07): `flow_list` and `flow_run`. A flow is project code: it runs only in a
//! workspace a person trusted, and no tool grants that trust - an untrusted project answers
//! `flows.untrusted` with the way to the trust dialog. Cancelling a run is the person's
//! (`flows.cancel` has no tool).

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::target::{Resolved, TargetArg, resolve};

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
    /// `"all"`, a view id or a breakpoint id; when absent, the views the flow names in its
    /// `// @views` comment, else all.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// Wait for the result (the default); `false` answers the `runId` at once and the result
    /// lands in `.rbrowser/out/flows/<runId>/result.json` (`flow_list` shows the run going on).
    #[serde(default)]
    pub wait: Option<bool>,
}

#[tool_router(router = flows_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The project's flows (.rbrowser/flows/*.ts: Playwright scripts with page, view, expect, env and step injected) with their description and views, whether the workspace is trusted (flows run only then; trust is a person's decision in rBrowser), the run going on with each view's status, and the last run's result file."
    )]
    pub(crate) async fn flow_list(
        &self,
        Parameters(_params): Parameters<FlowListParams>,
    ) -> ToolResult {
        respond(self.backend.flow_list().await)
    }

    #[tool(
        description = "Runs a flow in every chosen view at once and answers per view passed, failed, skipped (Firefox and WebKit views, with the reason) or timedOut, with duration, steps, the error with its step and line, and a screenshot file on failure - the same result.json the shell and `rbrowser flow run` show. Refused with flows.untrusted until a person trusts the workspace; one run at a time (flows.busy)."
    )]
    pub(crate) async fn flow_run(
        &self,
        Parameters(params): Parameters<FlowRunParams>,
    ) -> ToolResult {
        let result = async {
            let views = match params.target {
                None => None,
                Some(target) => {
                    let views = self.read(|backend| backend.views()).await?;
                    match resolve(Some(&target), &views)? {
                        Resolved::All => Some(views.iter().map(|view| view.id).collect()),
                        Resolved::Views(ids) => Some(ids),
                    }
                }
            };
            self.backend
                .flow_run(&params.name, views, params.wait.unwrap_or(true))
                .await
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "flows_tests.rs"]
mod tests;
