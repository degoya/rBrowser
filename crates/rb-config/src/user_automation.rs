//! `automation`: the user settings of Automation > Playwright & flows (RB-040-07). The CDP proxy
//! (`cdp`) and the MCP server (`mcp`) keep their own root sections.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The folder of the flows when `automation.flows.dir` is not set.
pub const DEFAULT_FLOWS_DIR: &str = ".rbrowser/flows";

/// `automation`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AutomationSettings {
    /// Flows: Playwright scripts of the project that run in every view (RB-040-07).
    pub flows: FlowSettings,
}

/// `automation.flows`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct FlowSettings {
    /// The folder of the flows, relative to the project; `.rbrowser/flows` or a folder inside it,
    /// which workspace trust covers.
    pub dir: String,
    /// How many views run a flow at once; 0 runs all at once.
    #[schemars(range(max = 32))]
    pub parallel: u32,
    /// How long a flow may run in one view, in milliseconds.
    #[schemars(range(min = 1000, max = 3_600_000))]
    pub timeout_ms: u32,
    /// How long one action of a flow (a click, a fill, a wait for an element) may wait, in
    /// milliseconds, before it fails with its locator; never longer than the flow may run
    /// (RB-090-76).
    #[schemars(range(min = 1000, max = 3_600_000))]
    pub action_timeout_ms: u32,
    /// Keep syncing input between the views while a flow runs; off, the views of the run
    /// stop syncing until it ends (otherwise each action happens twice).
    pub sync_during_run: bool,
    /// Save a screenshot of a view whose flow failed or timed out.
    pub screenshot_on_failure: bool,
}

impl Default for FlowSettings {
    fn default() -> Self {
        Self {
            dir: DEFAULT_FLOWS_DIR.to_owned(),
            parallel: 0,
            timeout_ms: 30_000,
            action_timeout_ms: 10_000,
            sync_during_run: false,
            screenshot_on_failure: true,
        }
    }
}

#[cfg(test)]
#[path = "user_automation_tests.rs"]
mod tests;
