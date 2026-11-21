//! `checks_export` (RB-092-05, the command `checks.export`): the last check run as a file -
//! SARIF 2.1.0, JUnit XML, one offline HTML page or the run's JSON - written below the project's
//! `.rbrowser/out/` or the person's export folder; the answer is the path. The core holds the
//! path to those folders; an agent cannot widen them (`checks.export.dir` is protected).

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The file format of `checks_export`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// SARIF 2.1.0: one run per check plugin.
    Sarif,
    /// JUnit XML: one test suite per breakpoint, one test case per rule.
    Junit,
    /// One HTML page that opens offline.
    Html,
    /// The run's `findings.json`.
    Json,
}

/// `checks_export`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChecksExportParams {
    /// The format; the setting checks.export.defaultFormat when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ExportFormat>,
    /// Where to write: a relative path below the export folder, or an absolute one inside the
    /// project's .rbrowser/out/ or the export folder; `<run-id>.<extension>` in the export
    /// folder when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[tool_router(router = checks_export_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Export the last check run (checks_run, the findings panel) as a file: format sarif (SARIF 2.1.0, one run per plugin, breakpoints, views, engines and URL in each result's properties), junit (JUnit XML, one test suite per breakpoint, one test case per rule), html (one page that opens offline) or json (the run's findings.json); checks.export.defaultFormat when absent. path: a relative path lands below the export folder (checks.export.dir, else .rbrowser/out/exports); an absolute one must lie inside the project's .rbrowser/out/ or that folder - anything else, .. included, is refused. Answers { path, format }. Fails with checks.export.error.no_run before the first run. Acts on no view."
    )]
    pub(crate) async fn checks_export(
        &self,
        Parameters(params): Parameters<ChecksExportParams>,
    ) -> ToolResult {
        let result = match serde_json::to_value(&params) {
            Ok(args) => self.command("checks.export", args).await,
            Err(error) => Err(ToolFailure::new("internal", error.to_string())),
        };
        respond(result)
    }
}

#[cfg(test)]
#[path = "checks_export_tests.rs"]
mod tests;
