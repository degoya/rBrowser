//! `envCompare`: Capture > Environment compare (RB-080-03) - which environments and breakpoints
//! a comparison run takes, how different two shots may be, and how the shots are taken. The
//! wait conditions and the banner selectors are those of Capture > Sitemap (`sitemap.*`).
//! Read by `crates/rbrowser/src/compare/`, which holds every value to its bounds again.
//! Re-exported from the crate root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `envCompare`: Capture > Environment compare (RB-080-03).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct EnvCompareConfig {
    /// The environment ids compared when a call names none; every environment when empty.
    /// The settings window offers the project's environments (`x-options`, RB-080-72).
    #[schemars(extend("x-options" = "environments"))]
    pub environments: Vec<String>,
    /// The breakpoint ids compared when a call names none; every Chromium breakpoint when
    /// empty. The settings window offers the project's breakpoints.
    #[schemars(extend("x-options" = "breakpoints"))]
    pub breakpoints: Vec<String>,
    /// How different two pixels may be and still count as equal, 0 to 1; `visual.threshold`
    /// when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub threshold: Option<f64>,
    /// The share of differing pixels, in percent, up to which two shots still count as the
    /// same (0 to 100).
    #[schemars(range(min = 0.0, max = 100.0))]
    pub max_diff_percent: f64,
    /// Capture the whole page, not only the viewport.
    pub full_page: bool,
    /// A further pause before each screenshot, in milliseconds (0 to 30000).
    #[schemars(range(max = 30000))]
    pub wait_ms: u32,
    /// Shots taken at once, each in its own hidden view (1 to 8).
    #[schemars(range(min = 1, max = 8))]
    pub parallel: u32,
}

impl Default for EnvCompareConfig {
    fn default() -> Self {
        Self {
            environments: Vec::new(),
            breakpoints: Vec::new(),
            threshold: None,
            max_diff_percent: 0.0,
            full_page: true,
            wait_ms: 0,
            parallel: 4,
        }
    }
}

#[cfg(test)]
#[path = "env_compare_tests.rs"]
mod tests;
