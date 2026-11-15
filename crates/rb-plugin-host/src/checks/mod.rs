//! The check runner (RB-060-01): every enabled check plugin over the target's views, bounded in
//! parallel, its findings held to the person's rule overrides and ignore patterns, merged into
//! one list with stable ids, written to `.rbrowser/out/checks/<run-id>/findings.json`, and the
//! answer of the core MCP tool `checks_run`.
//!
//! What runs is a [`CheckSource`]: a loaded check plugin ([`crate::LoadedPlugin`]), or anything
//! answering the same two questions - so the runner is tested without WebAssembly.

use async_trait::async_trait;
use rb_plugin_types::{CheckContext, Failure, Finding, RuleInfo};

use crate::plugin::LoadedPlugin;

mod jobs;
mod merge;
mod policy;
mod progress;
mod reported;
mod runner;
mod store;
mod tool;

#[cfg(test)]
mod testing;

pub use merge::{Report, merge};
pub use policy::{
    ChecksPolicy, ChecksSettings, DEFAULT_KEEP_RUNS, DEFAULT_PARALLEL, IgnorePattern,
    MAX_KEEP_RUNS, MAX_PARALLEL, RuleLevel,
};
pub use progress::{
    Cancel, JobProgress, JobState, PluginTiming, ProgressSink, RunControl, RunOutcome, SkipReason,
    estimate_ms, timings_of,
};
pub use reported::reported_run;
pub use runner::{CheckRunner, RunRequest, RunView};
pub use store::{CHECKS_DIR, findings_path, run_id, valid_run_id, write_run};
pub use tool::{
    CHECKS_RUN_DESCRIPTION, CHECKS_RUN_TOOL, ChecksRunAnswer, ChecksRunArgs, MAX_ANSWER_FINDINGS,
};

/// One check plugin as the runner sees it.
#[async_trait]
pub trait CheckSource: Send + Sync {
    /// The plugin's id: the first part of its qualified rule ids (`seo.images`).
    fn plugin_id(&self) -> &str;

    /// `check.describe`.
    async fn describe(&self) -> Result<Vec<RuleInfo>, Failure>;

    /// `check.run`.
    async fn run(&self, ctx: CheckContext) -> Result<Vec<Finding>, Failure>;
}

#[async_trait]
impl CheckSource for LoadedPlugin {
    fn plugin_id(&self) -> &str {
        &self.manifest().id
    }

    async fn describe(&self) -> Result<Vec<RuleInfo>, Failure> {
        LoadedPlugin::describe(self).await
    }

    async fn run(&self, ctx: CheckContext) -> Result<Vec<Finding>, Failure> {
        self.run_check(&ctx).await
    }
}
