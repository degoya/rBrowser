//! The MCP tools of the environment comparison (RB-080-03): `env_compare` (plans the run,
//! starts it in hidden render views and answers its `runId`), `env_compare_result` (status and
//! the share of differing pixels per pair, breakpoint and path) and `env_compare_live` (the
//! active breakpoint once per environment, side by side, synced) and `env_compare_cancel` (stops
//! the run in progress) - core tools: their names,
//! arguments and answers. `rb-mcp` (`tools/env_compare.rs`) resolves `target` into view ids
//! and hands them to the host, which runs [`super::app`]; the commands take the same arguments.

use serde::{Deserialize, Serialize};

/// The run tool's name.
pub const COMPARE_TOOL: &str = "env_compare";
/// The result tool's name.
pub const RESULT_TOOL: &str = "env_compare_result";
/// The live tool's name.
pub const LIVE_TOOL: &str = "env_compare_live";
/// The cancel tool's name.
pub const CANCEL_TOOL: &str = "env_compare_cancel";

/// The most paths one run takes.
pub const MAX_PATHS: usize = 50;
/// The most environments, breakpoints and views named in one call.
pub const MAX_NAMES: usize = 64;
/// The longest path or URL.
pub const MAX_PATH_LEN: usize = 2048;

/// The arguments of `env_compare` and `env.compare.run`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompareArgs {
    /// Environment ids; `envCompare.environments`, else every environment, when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<String>,
    /// Paths (`/about`) or URLs under an environment; the target view's path when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Breakpoint ids; the target views' breakpoints, else `envCompare.breakpoints`, else every
    /// Chromium breakpoint, when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breakpoints: Vec<String>,
    /// The target views (resolved by `rb-mcp`): their breakpoints and the first one's path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<u32>,
    /// Over `envCompare.fullPage`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_page: Option<bool>,
    /// Over `envCompare.threshold`, 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Hide cookie banners and chat widgets; `capture.hideBanners` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; `capture.freezeAnimations` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
}

impl CompareArgs {
    /// The call's word on hiding and freezing (RB-080-12).
    pub fn clean(&self) -> rb_visual::CleanOverrides {
        rb_visual::CleanOverrides {
            hide_banners: self.hide_banners,
            freeze_animations: self.freeze_animations,
        }
    }

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

    /// Whether the arguments are within their limits.
    pub fn check(&self) -> Result<(), String> {
        if self.paths.len() > MAX_PATHS {
            return Err(format!("at most {MAX_PATHS} paths"));
        }
        if self
            .paths
            .iter()
            .any(|path| path.is_empty() || path.len() > MAX_PATH_LEN)
        {
            return Err(format!("a path is 1 to {MAX_PATH_LEN} characters"));
        }
        if [
            self.environments.len(),
            self.breakpoints.len(),
            self.views.len(),
        ]
        .into_iter()
        .any(|count| count > MAX_NAMES)
        {
            return Err(format!(
                "at most {MAX_NAMES} environments, breakpoints and views"
            ));
        }
        if self
            .environments
            .iter()
            .chain(&self.breakpoints)
            .any(|id| id.is_empty() || id.len() > 128)
        {
            return Err("environment and breakpoint ids are 1 to 128 characters".to_owned());
        }
        if self
            .threshold
            .is_some_and(|threshold| !(0.0..=1.0).contains(&threshold))
        {
            return Err("threshold is a number from 0 to 1".to_owned());
        }
        Ok(())
    }
}

/// The answer of `env_compare`: the run started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub run_id: String,
    /// The run's folder.
    pub folder: String,
    /// Environments times breakpoints times paths.
    pub shots: usize,
    /// Environment pairs times breakpoints times paths.
    pub diffs: usize,
}

/// The answer of `env_compare_cancel`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAnswer {
    pub run_id: String,
    /// Whether it was still running.
    pub cancelled: bool,
}

/// The arguments of `env_compare_result`, `env_compare_cancel`, `env.compare.report` and
/// `env.compare.cancel`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultArgs {
    /// The run; the latest when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

impl ResultArgs {
    /// The arguments in `value` (`null` for none).
    pub fn parse(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let args: Self =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        if args
            .run_id
            .as_deref()
            .is_some_and(|id| id.is_empty() || id.len() > 128)
        {
            return Err("runId is 1 to 128 characters".to_owned());
        }
        Ok(args)
    }
}

/// The arguments of `env_compare_live` and `env.compare.live`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LiveArgs {
    /// Environment ids; `envCompare.environments`, else every environment, when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<String>,
    /// The view whose breakpoint and page are compared; the active view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<u32>,
    /// On or off; toggles when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl LiveArgs {
    /// The arguments in `value` (`null` for none).
    pub fn parse(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let args: Self =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        if args.environments.len() > MAX_NAMES
            || args
                .environments
                .iter()
                .any(|id| id.is_empty() || id.len() > 128)
        {
            return Err(format!(
                "at most {MAX_NAMES} environment ids of 1 to 128 characters"
            ));
        }
        Ok(args)
    }
}

/// The answer of `env_compare_live`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAnswer {
    /// Whether the comparison grid is shown now.
    pub enabled: bool,
    /// Each environment's id and the URL its view loads (empty when switched off).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<LiveView>,
}

/// One view of the live comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveView {
    pub view: u32,
    pub environment: String,
    pub url: String,
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
