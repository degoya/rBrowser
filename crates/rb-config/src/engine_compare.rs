//! `engineCompare`: Capture > Engine compare (RB-094-03) - how different the shots of one page in
//! Chromium, Firefox and WebKit may be, and how long each engine's page settles before its shot.
//! Read by `crates/rbrowser/src/engine_compare/`, which holds every value to its bounds again.
//! Re-exported from the crate root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `engineCompare.waitMs` when not set: the pause after the page loaded in each engine.
pub const DEFAULT_WAIT_MS: u32 = 1000;

/// `engineCompare`: Capture > Engine compare (RB-094-03).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct EngineCompareConfig {
    /// How different two pixels may be and still count as equal, 0 to 1; `visual.threshold`
    /// when absent. Anti-aliased pixels (font smoothing) never count.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub threshold: Option<f64>,
    /// The share of differing pixels, in percent, up to which two engines still count as the
    /// same (0 to 100).
    #[schemars(range(min = 0.0, max = 100.0))]
    pub max_diff_percent: f64,
    /// The pause after the page loaded in each engine, before its screenshot, in milliseconds
    /// (0 to 30000).
    #[schemars(range(max = 30000))]
    pub wait_ms: u32,
}

impl Default for EngineCompareConfig {
    fn default() -> Self {
        Self {
            threshold: None,
            max_diff_percent: 0.0,
            wait_ms: DEFAULT_WAIT_MS,
        }
    }
}

#[cfg(test)]
#[path = "engine_compare_tests.rs"]
mod tests;
