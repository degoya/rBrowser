//! The page zoom inside a view (RB-090-20): `view_page_zoom`, the commands
//! `view.page_zoom.reset` (100 %, or a factor), `view.page_zoom.in` and `view.page_zoom.out`
//! (Chromium's steps), once per resolved target. Not the grid's zoom of a cell.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// The smallest and the largest factor Chromium zooms a page to.
const RANGE: (f64, f64) = (0.25, 5.0);

/// `view_page_zoom`: a factor (`1` = 100 %) or a step.
#[derive(Debug, Clone, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum PageZoomLevel {
    /// The factor: 1 for 100 %, 0.25 to 5.
    Factor(f64),
    /// `"in"` or `"out"`: one of Chromium's steps.
    Step(String),
}

/// `view_page_zoom`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageZoomParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// The factor (1 = 100 %, 0.25 to 5) or `"in"` / `"out"` for one of Chromium's steps;
    /// 100 % when absent.
    #[serde(default)]
    pub level: Option<PageZoomLevel>,
}

#[tool_router(router = page_zoom_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Zoom the page inside the targeted views as Chrome's Ctrl+Plus, Ctrl+Minus and Ctrl+0 do (Chromium's page zoom, not the grid's zoom of a cell): the page lays out at another CSS width, so breakpoints apply differently. level: a factor (1 = 100 %, 0.25 to 5), \"in\" or \"out\" for one step; 100 % when absent. With views.pageZoom.sync every view takes the level; Chromium keeps one zoom per site and session. A Firefox view zooms its stand-in only; WebKit views do not zoom. Answers views: [{ view, level, percent }]."
    )]
    pub(crate) async fn view_page_zoom(
        &self,
        Parameters(params): Parameters<PageZoomParams>,
    ) -> ToolResult {
        let (command, args) = match params.level {
            None => ("view.page_zoom.reset", json!({})),
            Some(PageZoomLevel::Factor(level))
                if level.is_finite() && (RANGE.0..=RANGE.1).contains(&level) =>
            {
                ("view.page_zoom.reset", json!({ "level": level }))
            }
            Some(PageZoomLevel::Step(step)) if step == "in" => ("view.page_zoom.in", json!({})),
            Some(PageZoomLevel::Step(step)) if step == "out" => ("view.page_zoom.out", json!({})),
            Some(_) => {
                return respond::<Value>(Err(ToolFailure::new(
                    "args.invalid",
                    "level is a factor from 0.25 to 5 (1 = 100 %), \"in\" or \"out\"",
                )));
            }
        };
        respond(self.per_target(command, params.target.as_ref(), args).await)
    }
}

#[cfg(test)]
#[path = "page_zoom_tests.rs"]
mod tests;
