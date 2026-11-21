//! `automation`: the user settings of Automation > Playwright & flows (RB-040-07). The CDP proxy
//! (`cdp`) and the MCP server (`mcp`) keep their own root sections.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The folder of the flows when `automation.flows.dir` is not set.
pub const DEFAULT_FLOWS_DIR: &str = ".rbrowser/flows";

/// The layout modes `automation.flows.runLayout` offers; a preset's id may be typed instead.
pub const RUN_LAYOUTS: [&str; 4] = ["grid", "horizontal", "split", "canvas"];

/// `automation.flows.hiddenViews` (RB-090-84): what a run does with the views the layout hides -
/// horizontal mode scrolls views out of sight, focus mode shows one breakpoint - whose pages the
/// browser throttles, so their actions wait in vain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HiddenViews {
    /// The run plays in the views shown; the hidden ones and the breakpoints the flow names that
    /// no view shows are named as skipped in the result and a notice.
    #[default]
    Skip,
    /// The run shows the layout `runLayout` while it goes on, and the one before after it.
    Layout,
}

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
    /// What a run does with views the layout hides: `skip` plays in the views shown and names the
    /// rest, `layout` shows `runLayout` for the run (RB-090-84).
    pub hidden_views: HiddenViews,
    /// With `hiddenViews: skip`, a played recording whose views the layout all hides brings the
    /// first into sight - the canvas centred on it, the strip scrolled to it, focus mode on its
    /// breakpoint - instead of failing (RB-090-89). Off, it fails as any run with no view shown.
    pub reveal_recording: bool,
    /// "Delete" in the Flows panel asks in the panel before the flow's file is deleted
    /// (RB-090-91). Off, a click deletes it at once.
    pub confirm_delete: bool,
    /// The layout a run shows with `hiddenViews: layout`: `grid`, `horizontal`, `split` or
    /// `canvas` over the preset shown, or a preset's id.
    /// The settings window offers the modes and the project's presets (source `layouts`).
    #[schemars(extend("x-options" = "layouts"))]
    pub run_layout: String,
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
            hidden_views: HiddenViews::Skip,
            reveal_recording: true,
            confirm_delete: true,
            run_layout: "grid".to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "user_automation_tests.rs"]
mod tests;
