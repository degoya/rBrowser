//! The MCP tool of the engine comparison (RB-094-03), `compare_engines`: one page at one
//! breakpoint in Chromium, Firefox and WebKit, each engine pair diffed; it answers when the run
//! is done, with every file as a path and per pair the share of differing pixels and the
//! differing areas. A core tool: its name, arguments and bounds. `rb-mcp`
//! (`tools/compare_engines.rs`) resolves `target` into a view id and hands it to the host,
//! which runs [`super::app`]; the command `compare.engines` takes the same arguments.

use serde::{Deserialize, Serialize};

use super::plan::{ENGINES, Overrides};

/// The tool's name.
pub const COMPARE_TOOL: &str = "compare_engines";
/// The longest URL.
pub const MAX_URL_LEN: usize = 2048;
/// The longest breakpoint id or engine name.
pub const MAX_NAME_LEN: usize = 128;

/// The arguments of `compare_engines` and `compare.engines`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompareEnginesArgs {
    /// The breakpoint id; the target view's when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakpoint: Option<String>,
    /// `chromium`, `firefox`, `webkit`; all three when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines: Vec<String>,
    /// The page; the target view's when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The target view (resolved by `rb-mcp`; the active view when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<u32>,
    /// Over `engineCompare.threshold`, 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Over `engineCompare.maxDiffPercent`, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_diff_percent: Option<f64>,
    /// The command only: open `report.html` in the active view when done (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<bool>,
}

impl CompareEnginesArgs {
    /// The arguments in `value` (`null` for none), checked.
    pub fn parse(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let args: Self =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        args.check()?;
        Ok(args)
    }

    /// Whether the arguments are within their bounds.
    pub fn check(&self) -> Result<(), String> {
        if self.engines.len() > ENGINES.len() {
            return Err(format!("at most {} engines", ENGINES.len()));
        }
        let bad_name = |name: &String| name.is_empty() || name.len() > MAX_NAME_LEN;
        if self.engines.iter().any(bad_name) || self.breakpoint.as_ref().is_some_and(bad_name) {
            return Err(format!(
                "engine names and the breakpoint id are 1 to {MAX_NAME_LEN} characters"
            ));
        }
        if self
            .url
            .as_ref()
            .is_some_and(|url| url.is_empty() || url.len() > MAX_URL_LEN)
        {
            return Err(format!("url is 1 to {MAX_URL_LEN} characters"));
        }
        if self
            .threshold
            .is_some_and(|threshold| !(0.0..=1.0).contains(&threshold))
        {
            return Err("threshold is a number from 0 to 1".to_owned());
        }
        if self
            .max_diff_percent
            .is_some_and(|percent| !(0.0..=100.0).contains(&percent))
        {
            return Err("maxDiffPercent is a number from 0 to 100".to_owned());
        }
        Ok(())
    }

    /// The call's word over `engineCompare`.
    pub fn overrides(&self) -> Overrides {
        Overrides {
            threshold: self.threshold,
            max_diff_percent: self.max_diff_percent,
        }
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
