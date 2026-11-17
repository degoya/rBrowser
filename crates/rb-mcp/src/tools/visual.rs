//! Visual regression (RB-050-04): `visual_compare` and `visual_baseline_update`, run by the host
//! once per target and waited for (the commands `visual.compare` and `visual.baseline_update`
//! answer at once and notify later). Setting up Git LFS (`visual.lfs_setup`) has no tool: it
//! changes the Git configuration, which only the person decides.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use super::forward::Answers;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// `visual_compare`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompareParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// The colour tolerance, 0 to 1; the setting `visual.threshold` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Hide cookie banners and chat widgets; the setting capture.hideBanners when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; the setting capture.freezeAnimations when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
}

/// `visual_baseline_update`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BaselineUpdateParams {
    /// `"all"` (default), a view id or a breakpoint id.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
    /// Hide cookie banners and chat widgets; the setting capture.hideBanners when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; the setting capture.freezeAnimations when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
}

fn args_of(params: &impl Serialize) -> Result<serde_json::Value, ToolFailure> {
    serde_json::to_value(params).map_err(|error| ToolFailure::new("internal", error.to_string()))
}

#[tool_router(router = visual_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Compare views with their baselines in .rbrowser/baselines/<page>/<breakpoint>.png (target \"all\" (default), a view id or a breakpoint id): each view is captured, diffed pixel by pixel with anti-aliasing tolerance and answered as ok, differs (share of differing pixels, bounding boxes, diff image path), new (no baseline), sizeMismatch or failed, with the metadata fields that make a comparison unreliable (another machine, DPI or engine version). threshold (0 to 1) overrides visual.threshold. The findings also go to the findings panel; regressions counts differs and sizeMismatch. One run at a time. hideBanners and freezeAnimations override capture.hideBanners and capture.freezeAnimations (banners hidden, animations frozen before each capture, undone after)."
    )]
    pub(crate) async fn visual_compare(
        &self,
        Parameters(params): Parameters<CompareParams>,
    ) -> ToolResult {
        let result = async {
            if params
                .threshold
                .is_some_and(|threshold| !(0.0..=1.0).contains(&threshold))
            {
                return Err(ToolFailure::new(
                    "invalid_arguments",
                    "threshold must be a number from 0 to 1",
                ));
            }
            let args = args_of(&params)?;
            self.host_per_target(
                "visual_compare",
                params.target.as_ref(),
                args,
                Answers::Totals,
            )
            .await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Capture views (target \"all\" (default), a view id or a breakpoint id) and store them as the new baselines in .rbrowser/baselines/<page>/<breakpoint>.png with meta.json (machine, DPI, engine and version, viewport, date), replacing the ones before. Answers the files written and whether Git LFS tracks the baselines; setting up Git LFS is the person's command in rBrowser. hideBanners and freezeAnimations override capture.hideBanners and capture.freezeAnimations (banners hidden, animations frozen before each capture, undone after)."
    )]
    pub(crate) async fn visual_baseline_update(
        &self,
        Parameters(params): Parameters<BaselineUpdateParams>,
    ) -> ToolResult {
        let result = match args_of(&params) {
            Ok(args) => {
                self.host_per_target(
                    "visual_baseline_update",
                    params.target.as_ref(),
                    args,
                    Answers::Joined,
                )
                .await
            }
            Err(failure) => Err(failure),
        };
        respond(result)
    }
}

#[cfg(test)]
#[path = "visual_tests.rs"]
mod tests;
