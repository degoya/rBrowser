//! The MCP tools of visual regression (RB-050-04): `visual_compare` and
//! `visual_baseline_update`, core tools without a prefix - their names, descriptions, arguments
//! and answers. The MCP server (RB-030-01) is not on this branch's base: it registers the tools
//! with these types and calls [`super::compare`] and [`super::update`]; the commands
//! `visual.compare` and `visual.baseline_update` take the same arguments meanwhile.
//!
//! Setting up Git LFS (`visual.lfs_setup`) has no tool: it changes the Git configuration, which
//! only the person decides.

use rb_config::ViewScope;
use rb_engine::Target;
use rb_visual::Comparison;
use serde::{Deserialize, Serialize};

/// The compare tool's name.
pub const COMPARE_TOOL: &str = "visual_compare";
/// The compare tool's description for the agent.
pub const COMPARE_DESCRIPTION: &str = "Compare views with their baselines in \
.rbrowser/baselines/<page>/<breakpoint>.png (target \"all\", the default, or a view id; scope \
\"visible\" or breakpoint ids narrows it to their views): each view is captured, diffed pixel by pixel with anti-aliasing tolerance and answered as ok, \
differs (share of differing pixels, bounding boxes, diff image path), new (no baseline), \
sizeMismatch or failed, with the metadata fields that make a comparison unreliable (another \
DPI, engine or engine version, viewport or full page). threshold (0 to 1) overrides visual.threshold. The findings \
also go to the findings panel; regressions counts differs and sizeMismatch. hideBanners and \
freezeAnimations override capture.hideBanners and capture.freezeAnimations for this run.";
/// The baseline tool's name.
pub const UPDATE_TOOL: &str = "visual_baseline_update";
/// The baseline tool's description for the agent.
pub const UPDATE_DESCRIPTION: &str = "Capture views (target \"all\", the default, or a view \
id; scope \"visible\" or breakpoint ids narrows it to their views, the other baselines stay) and \
store them as the new baselines in .rbrowser/baselines/<page>/<breakpoint>.png with \
meta.json (machine, DPI, engine and version, viewport, date), replacing the ones before. \
Answers the files written and whether Git LFS tracks the baselines. hideBanners and \
freezeAnimations override capture.hideBanners and capture.freezeAnimations for this run.";

/// The arguments of `visual_compare` and `visual.compare`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompareArgs {
    /// `"all"` (default) or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The colour tolerance, 0 to 1; `visual.threshold` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Hide cookie banners and chat widgets; `capture.hideBanners` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; `capture.freezeAnimations` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
    /// Only the views of these breakpoints: `"visible"` or breakpoint ids (RB-099-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ViewScope>,
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
        if let Some(threshold) = args.threshold
            && !(0.0..=1.0).contains(&threshold)
        {
            return Err("threshold must be a number from 0 to 1".to_owned());
        }
        Ok(args)
    }
}

/// The arguments of `visual_baseline_update` and `visual.baseline_update`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateArgs {
    /// `"all"` (default) or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// Hide cookie banners and chat widgets; `capture.hideBanners` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; `capture.freezeAnimations` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
    /// Only the views of these breakpoints: `"visible"` or breakpoint ids (RB-099-10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ViewScope>,
}

impl UpdateArgs {
    /// The call's word on hiding and freezing (RB-080-12).
    pub fn clean(&self) -> rb_visual::CleanOverrides {
        rb_visual::CleanOverrides {
            hide_banners: self.hide_banners,
            freeze_animations: self.freeze_animations,
        }
    }

    /// The arguments in `value` (`null` for none).
    pub fn parse(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())
    }
}

/// The answer of `visual_compare`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareAnswer {
    /// The findings run the comparisons were reported in (`.rbrowser/out/checks/<run>`).
    pub run_id: String,
    /// One per view, in layout order.
    pub comparisons: Vec<Comparison>,
    /// The views that differ or changed size.
    pub regressions: usize,
    /// The views whose comparison is unreliable (metadata).
    pub unreliable: usize,
    /// The folder of the diff images, when one was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// The chosen breakpoints no view shows: not compared (RB-099-10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_shown: Vec<String>,
}

impl CompareAnswer {
    /// The answer for `comparisons` of the run `run_id`.
    pub fn of(run_id: String, comparisons: Vec<Comparison>, folder: Option<String>) -> Self {
        Self {
            run_id,
            regressions: comparisons
                .iter()
                .filter(|comparison| comparison.status.is_regression())
                .count(),
            unreliable: comparisons
                .iter()
                .filter(|comparison| comparison.unreliable())
                .count(),
            comparisons,
            folder,
            not_shown: Vec::new(),
        }
    }
}

/// One baseline written (or not).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBaseline {
    /// The view.
    pub view: u32,
    /// Its label.
    pub label: String,
    /// The baseline's name.
    pub breakpoint: String,
    /// The page's folder.
    pub slug: String,
    /// The PNG written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Why it was not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Whether Git LFS keeps the baselines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsAnswer {
    /// `.gitattributes` gives `.rbrowser/baselines` the lfs filter.
    pub tracked: bool,
    /// `git lfs` is installed.
    pub installed: bool,
}

/// The answer of `visual_baseline_update`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAnswer {
    /// One per view, in layout order.
    pub baselines: Vec<UpdatedBaseline>,
    /// The LFS check; absent with `visual.lfsCheck` off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lfs: Option<LfsAnswer>,
    /// The chosen breakpoints no view shows: their baselines stay as they are (RB-099-10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_shown: Vec<String>,
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
