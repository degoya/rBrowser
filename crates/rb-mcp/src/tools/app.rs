//! The app: `app_info`, `wsl_info`, `diagnostics`, `devtools_open`, `plugins_list`.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, json_result, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve_one};

/// `wsl_info`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WslParams {
    /// `info` (default) or `restart` (the helper, then the report).
    #[serde(default)]
    pub action: Option<String>,
}

/// `diagnostics`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticsParams {
    /// A file name ending in `.zip` inside the diagnostics folder; a fresh name when absent.
    #[serde(default)]
    pub out: Option<String>,
}

/// `devtools_open`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevtoolsParams {
    /// One view (id or breakpoint id); the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// `docked-bottom`, `docked-right` or `window`; the setting's mode when absent.
    #[serde(default)]
    pub mode: Option<String>,
}

#[tool_router(router = app_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "rBrowser's version, commit, build time and platform, the engine versions (CEF/Chromium, Firefox, WebKit, Node), the API versions and the data and log folders - the facts of Settings > About, without the licence texts."
    )]
    pub(crate) async fn app_info(&self) -> ToolResult {
        respond(self.read(|backend| backend.app_info()).await)
    }

    #[tool(
        description = "WSL: the installed distributions, the one in use, its networking mode (mirrored or NAT) and whether 127.0.0.1 inside it reaches rBrowser, and the helper's state. action restart restarts the helper first."
    )]
    pub(crate) async fn wsl_info(&self, Parameters(params): Parameters<WslParams>) -> ToolResult {
        match params.action.as_deref() {
            None | Some("info") => {}
            Some("restart") => {
                if let Err(failure) = self.command("wsl.restart_helper", Value::Null).await {
                    return respond::<Value>(Err(failure));
                }
            }
            Some(other) => {
                return respond::<Value>(Err(ToolFailure::new(
                    "invalid_arguments",
                    format!("action is info or restart, not `{other}`"),
                )));
            }
        }
        json_result(&self.backend.wsl_info().await)
    }

    #[tool(
        description = "Write the diagnostics ZIP (logs, crash dumps, versions, the masked config) into the diagnostics folder and answer its path. Nothing is sent anywhere."
    )]
    pub(crate) async fn diagnostics(
        &self,
        Parameters(params): Parameters<DiagnosticsParams>,
    ) -> ToolResult {
        // Named arguments (an object, never `null`): the core writes to the diagnostics folder
        // and opens no dialog and no Explorer window for an agent's call (RB-080-73).
        let args = object([("out", params.out.map(Value::String))]);
        respond(self.command("diagnostics.create", args).await)
    }

    #[tool(
        description = "Open DevTools for one view (target: a view id or breakpoint id, default the active view); mode docked-bottom, docked-right or window (default the setting)."
    )]
    pub(crate) async fn devtools_open(
        &self,
        Parameters(params): Parameters<DevtoolsParams>,
    ) -> ToolResult {
        let result = async {
            let views = self.read(|backend| backend.views()).await?;
            let view = resolve_one(params.target.as_ref(), &views)?;
            let args = object([
                ("target", view.map(|view| json!(view))),
                ("mode", params.mode.map(Value::String)),
            ]);
            self.command("devtools.open", args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "List the installed plugins - loaded, disabled and refused: id, name, version, description, source (bundled, user, project), signature, state (with the reason when one did not load), permissions, MCP prefix, tools and the number of commands and settings. Installing, enabling or trusting a plugin is a person's decision in rBrowser, never over MCP."
    )]
    pub(crate) async fn plugins_list(&self) -> ToolResult {
        respond(self.command("plugins.list", Value::Null).await)
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
