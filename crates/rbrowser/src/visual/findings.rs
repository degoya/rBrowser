//! The comparisons as a check run of the plugin `visual-regression`: the run's request holds
//! them as the plugin's arguments (`{ "visual": { "comparisons", "failOnMetaMismatch" } }`),
//! names only that plugin and the compared views, and goes through the same runner, policy
//! (`checks.rules`, `checks.ignore`), output folder and findings panel as `checks.run_all`
//! (`crate::checks::run_request`).

use rb_engine::Target;
use rb_plugin_host::{RunRequest, RunView};
use rb_plugin_types::CheckTarget;
use rb_visual::Comparison;
use serde::Serialize;

use super::run::ViewTarget;

/// The plugin that turns comparisons into findings.
pub const PLUGIN_ID: &str = "visual-regression";

/// The plugin's arguments.
#[derive(Serialize)]
struct Args<'a> {
    visual: VisualRun<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VisualRun<'a> {
    comparisons: &'a [Comparison],
    fail_on_meta_mismatch: bool,
}

/// The arguments the plugin gets: the comparisons and whether an unreliable one is an error.
pub fn check_args(comparisons: &[Comparison], fail_on_meta_mismatch: bool) -> String {
    let args = Args {
        visual: VisualRun {
            comparisons,
            fail_on_meta_mismatch,
        },
    };
    serde_json::to_string(&args).unwrap_or_else(|error| {
        tracing::warn!(%error, "the comparisons do not serialize");
        "{}".to_owned()
    })
}

/// The check run of `comparisons` over `views`, with the id `run_id`, started at `started_at`
/// (Unix seconds).
pub fn request(
    run_id: String,
    started_at: u64,
    target: Target,
    views: &[ViewTarget],
    comparisons: &[Comparison],
    fail_on_meta_mismatch: bool,
) -> RunRequest {
    RunRequest {
        run_id,
        started_at,
        target: match target {
            Target::All => CheckTarget::All,
            Target::View(id) => CheckTarget::View(id.0),
        },
        views: views
            .iter()
            .map(|view| RunView {
                id: view.id.0,
                breakpoint: Some(view.name.clone()),
                url: Some(view.url.clone()).filter(|url| !url.is_empty()),
            })
            .collect(),
        url: None,
        plugins: Some(vec![PLUGIN_ID.to_owned()]),
        args: check_args(comparisons, fail_on_meta_mismatch),
    }
}

#[cfg(test)]
#[path = "findings_tests.rs"]
mod tests;
