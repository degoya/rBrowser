//! The core MCP tool `checks_run` as its argument and answer model; the MCP server (RB-030-01)
//! registers it under [`CHECKS_RUN_TOOL`] and the command `checks.run_all` is covered by it.

use rb_plugin_types::{
    CheckFailure, CheckSkip, CheckTarget, Failure, FailureKind, FindingsFile, FindingsSummary,
    MergedFinding, codes,
};
use serde::{Deserialize, Serialize};

use super::progress::{JobProgress, RunOutcome};
use crate::valid_plugin_id;

/// The tool's name: a core tool, unprefixed.
pub const CHECKS_RUN_TOOL: &str = "checks_run";

/// The tool's description for MCP clients.
pub const CHECKS_RUN_DESCRIPTION: &str = "Runs every enabled check plugin (SEO, accessibility, \
     console, ...) over the views of `target` (\"all\" or a view id) and answers the merged \
     findings - deduplicated across views and plugins, each with a stable id - and a summary per \
     severity and plugin. `plugins` limits the run to those plugin ids, `url` checks that page \
     instead of the views' own. The run is also written to .rbrowser/out/checks/.";

/// The most findings one answer carries; the summary still counts all of them.
pub const MAX_ANSWER_FINDINGS: usize = 500;

/// The most plugins one call names.
const MAX_PLUGINS: usize = 64;

/// The longest URL accepted.
const MAX_URL_CHARS: usize = 8 * 1024;

/// The arguments of `checks_run`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ChecksRunArgs {
    /// `"all"` (the default) or a view id.
    pub target: CheckTarget,
    /// Only these check plugins (ids); every enabled one when absent.
    pub plugins: Option<Vec<String>>,
    /// The page to check, `http(s)`; each view's own when absent.
    pub url: Option<String>,
}

fn bad_argument(name: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("argument", name)
}

impl ChecksRunArgs {
    /// The arguments in `value`, checked: a known shape, plugin ids, an `http(s)` URL.
    ///
    /// # Errors
    ///
    /// `plugins.error.invalid` naming the argument.
    pub fn parse(value: &serde_json::Value) -> Result<Self, Failure> {
        let args: Self =
            serde_json::from_value(value.clone()).map_err(|_| bad_argument("arguments"))?;
        if let Some(plugins) = &args.plugins
            && (plugins.len() > MAX_PLUGINS || !plugins.iter().all(|id| valid_plugin_id(id)))
        {
            return Err(bad_argument("plugins"));
        }
        if let Some(url) = &args.url {
            let web = url.starts_with("https://") || url.starts_with("http://");
            if !web
                || url.chars().count() > MAX_URL_CHARS
                || url.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(bad_argument("url"));
            }
        }
        Ok(args)
    }
}

/// The answer of `checks_run`.
#[derive(Clone, Debug, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChecksRunAnswer {
    pub run_id: String,
    pub target: CheckTarget,
    /// The first [`MAX_ANSWER_FINDINGS`], highest severity first.
    pub findings: Vec<MergedFinding>,
    /// Whether `findings` was cut; `findings.json` of the run has all.
    pub truncated: bool,
    pub summary: FindingsSummary,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<CheckFailure>,
    /// The plugin x view pairs not checked (engine), so none reads as clean (RB-080-98).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<CheckSkip>,
    /// How each plugin x view job ended: done with its count, failed with its code and its
    /// parameters, skipped with the reason (RB-080-89) - `engine` with the view's engine for a
    /// view the plugin cannot serve (RB-080-98).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<JobProgress>,
    /// The run was cancelled (`checks.cancel`): the findings are those of the jobs that ended.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cancelled: bool,
}

impl ChecksRunAnswer {
    /// The answer for the run `file`.
    #[must_use]
    pub fn from_file(file: &FindingsFile) -> Self {
        Self {
            run_id: file.run_id.clone(),
            target: file.target,
            findings: file
                .findings
                .iter()
                .take(MAX_ANSWER_FINDINGS)
                .cloned()
                .collect(),
            truncated: file.findings.len() > MAX_ANSWER_FINDINGS,
            summary: file.summary.clone(),
            failures: file.failures.clone(),
            skipped: file.skipped.clone(),
            jobs: Vec::new(),
            cancelled: false,
        }
    }

    /// The answer for `outcome`: its run, and how each job ended.
    #[must_use]
    pub fn from_outcome(outcome: &RunOutcome) -> Self {
        Self {
            jobs: outcome.jobs.clone(),
            cancelled: outcome.cancelled,
            ..Self::from_file(&outcome.file)
        }
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
