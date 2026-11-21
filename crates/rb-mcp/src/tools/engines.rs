//! External engines (RB-040-01, RB-040-02, RB-040-04): `engine_restart`, the command
//! `engine.restart` once per view of `target` (once for the whole engine without one).
//! `engine.firefox.toggle` and `engine.webkit.toggle` are covered by it too: an agent restarts
//! an engine's views; switching an engine on or off for the session is the person's toggle.
//! `engines_list` (RB-080-32) is a read the host answers: the installed Firefox editions with
//! path, kind and version, and the configured path.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::forward::joined;
use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{Resolved, TargetArg, resolve};

/// The command behind `engine_restart`.
const RESTART: &str = "engine.restart";
/// The host tool behind `engines_list`.
pub const ENGINES_LIST_TOOL: &str = "engines_list";

/// The engines that run in windows of their own; Chromium runs the grid and restarts only with
/// rBrowser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExternalEngine {
    /// Firefox over WebDriver BiDi.
    Firefox,
    /// WebKit through Playwright in the sidecar.
    Webkit,
    /// Safari over safaridriver, macOS only (RB-095-03).
    Safari,
    /// iOS on the Simulator or a device over safaridriver, macOS only (RB-095-04).
    Ios,
}

/// `engine_restart`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineRestartParams {
    /// `firefox`, `webkit`, `safari` or `ios` (both macOS only).
    pub engine: ExternalEngine,
    /// `"all"` (every view of the engine, the default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = engines_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Restart an external engine's views (engine: \"firefox\", \"webkit\", \"safari\" or \"ios\" - Safari and iOS on macOS only): every view of that engine, or those of `target` (\"all\" (default), a view id or a breakpoint id). Each view opens again with its last URL. Fails with a readable message when no view of that engine is open or the engine is not available. Answers the restarted view ids."
    )]
    pub(crate) async fn engine_restart(
        &self,
        Parameters(params): Parameters<EngineRestartParams>,
    ) -> ToolResult {
        respond(self.restart_engine(params).await)
    }

    #[tool(
        description = "List the installed Firefox editions rBrowser can drive: for each its path, kind (release, esr, developer, nightly) and version, in the order rBrowser searches (the registry's App Paths and uninstall entries, then the editions' folders under Program Files and the local app data); the first is what an empty engines.firefox.path starts. Also the configured path and, when it names no file any more, missing: true. A read only; acts on no view and starts nothing."
    )]
    pub(crate) async fn engines_list(&self) -> ToolResult {
        respond(self.backend.host_tool(ENGINES_LIST_TOOL, json!({})).await)
    }
}

impl RbMcpServer {
    async fn restart_engine(&self, params: EngineRestartParams) -> Result<Value, ToolFailure> {
        let views = match params.target {
            None => vec![None],
            Some(target) => {
                let known = self.read(|backend| backend.views()).await?;
                match resolve(Some(&target), &known)? {
                    Resolved::All => vec![None],
                    Resolved::Views(ids) => ids.into_iter().map(Some).collect(),
                }
            }
        };
        let mut answer = Value::Null;
        for view in views {
            let args = object([
                ("engine", Some(json!(params.engine))),
                ("view", view.map(|view| json!(view.0))),
            ]);
            answer = joined(answer, self.command(RESTART, args).await?);
        }
        Ok(answer)
    }
}

#[cfg(test)]
#[path = "engines_tests.rs"]
mod tests;
