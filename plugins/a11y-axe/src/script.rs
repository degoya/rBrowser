//! The script one run sends to a view's isolated world (`evaluate`): axe-core's own file, the
//! runner (`runner/src/main.ts`) and the call with the run's options, all inside one function -
//! nothing of it is left in the world's globals afterwards (the runner removes `window.axe`),
//! and none of it reaches the page's main world. The script evaluates to a promise of the
//! runner's answer (`crate::answer`).

use serde::Serialize;

/// axe-core, the exact version `runner/package.json` pins (MPL-2.0, `NOTICE`).
pub const AXE_SOURCE: &str = include_str!("../runner/dist/axe.min.js");
/// The runner, one IIFE defining `rbA11yAxe`.
pub const RUNNER_SOURCE: &str = include_str!("../runner/dist/runner.js");

/// The most nodes one rule reports.
pub const MAX_NODES: u32 = 100;
/// The most nodes one view's answer holds.
pub const MAX_TOTAL: u32 = 2_000;
/// The most characters of a node's HTML and of its failure summary.
pub const MAX_TEXT: u32 = 300;

/// What the runner is asked for (`RunOptions` in `runner/src/run.ts`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOptions {
    pub rules: Vec<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub incomplete: bool,
    pub max_nodes: u32,
    pub max_total: u32,
    pub max_text: u32,
}

impl RunOptions {
    /// The options of a run of `rules`, with the usual bounds.
    #[must_use]
    pub fn new(
        rules: Vec<String>,
        include: &[String],
        exclude: &[String],
        incomplete: bool,
    ) -> Self {
        Self {
            rules,
            include: include.to_vec(),
            exclude: exclude.to_vec(),
            incomplete,
            max_nodes: MAX_NODES,
            max_total: MAX_TOTAL,
            max_text: MAX_TEXT,
        }
    }
}

/// The script of one run. The options go in as JSON - a JSON text is a JavaScript expression,
/// so a selector can never leave its string.
#[must_use]
pub fn script(options: &RunOptions) -> String {
    let options = serde_json::to_string(options).expect("the options always serialize");
    format!(
        "(() => {{\n{AXE_SOURCE}\n;\n{RUNNER_SOURCE}\n;\nreturn rbA11yAxe.start({options});\n}})()"
    )
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
