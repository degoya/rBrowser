//! `bug_report` (RB-093-04, the command `report.create`): a bug report of the views written to
//! the project's `.rbrowser/out/reports/<time>/` (or the person's `report.folder`), after the
//! person saved it in the preview when `report.preview` is on - screenshots with the password fields blacked
//! out, the overview, console errors and warnings, failed requests, page, environment,
//! breakpoint, emulation and version as `report.md` and `report.zip`, every text masked. The
//! answer is the paths, never the content: an agent reads the files it is meant to read.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{Resolved, TargetArg, resolve};

/// The command behind `bug_report`.
const CREATE: &str = "report.create";
/// The fields of the command's answer the tool hands on: paths only.
const PATH_FIELDS: [&str; 3] = ["path", "markdown", "zip"];

/// `bug_report`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BugReportParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// What went wrong, in one or two sentences (at most 2000 characters); masked like the rest
    /// and written into report.md.
    #[serde(default)]
    pub note: Option<String>,
}

#[tool_router(router = bug_report_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Write a bug report of the views for a ticket: a screenshot of each view (password fields blacked out), the overview, the console errors and warnings, the failed requests, and page, environment, breakpoint, size, emulation, engine and version - as report.md and report.zip in a new folder below .rbrowser/out/reports/ (or report.folder); report.include picks the parts. While report.preview is on (default) it waits until the person saves the preview; cancelled, the call fails. Every text is masked; request headers and bodies are never read. note: what went wrong. Answers { path, markdown, zip } - paths only, never the content. Needs an open project."
    )]
    pub(crate) async fn bug_report(
        &self,
        Parameters(params): Parameters<BugReportParams>,
    ) -> ToolResult {
        respond(self.create_bug_report(params).await)
    }
}

impl RbMcpServer {
    async fn create_bug_report(&self, params: BugReportParams) -> Result<Value, ToolFailure> {
        let target = match params.target {
            None => None,
            Some(target) => {
                let known = self.read(|backend| backend.views()).await?;
                match resolve(Some(&target), &known)? {
                    Resolved::All => None,
                    Resolved::Views(ids) => Some(json!(ids)),
                }
            }
        };
        let args = object([("target", target), ("note", params.note.map(Value::String))]);
        let answer = self.command(CREATE, args).await?;
        Ok(paths_only(&answer))
    }
}

/// The path fields of the command's `answer`, nothing else.
fn paths_only(answer: &Value) -> Value {
    let fields: Map<String, Value> = PATH_FIELDS
        .iter()
        .filter_map(|name| {
            answer
                .get(*name)
                .filter(|value| value.is_string())
                .map(|value| ((*name).to_owned(), value.clone()))
        })
        .collect();
    Value::Object(fields)
}

#[cfg(test)]
#[path = "bug_report_tests.rs"]
mod tests;
