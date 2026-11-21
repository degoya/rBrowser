//! Sitemap capture (RB-050-05): `sitemap_capture`, `sitemap_status`, `sitemap_cancel` and
//! `sitemap_gallery` (RB-090-83), run by the host (`crates/rbrowser/src/sitemap/app.rs`). A capture is no view tool: its pages load in
//! hidden render views of their own, one per breakpoint, never in the grid - so the tools take
//! breakpoints, not `target`. `sitemap_capture` answers once the pages are found and the run goes
//! on in the background; `sitemap_status` follows it; `sitemap_gallery` opens a run's gallery for
//! the person, never in the grid. The commands `sitemap.capture`, `sitemap.cancel`,
//! `sitemap.gallery` and `panels.sitemap` are covered by these tools.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;

/// The host tools behind the three tools.
const CAPTURE_TOOL: &str = "sitemap_capture";
const STATUS_TOOL: &str = "sitemap_status";
const CANCEL_TOOL: &str = "sitemap_cancel";
const GALLERY_TOOL: &str = "sitemap_gallery";

/// `sitemap_capture`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SitemapCaptureParams {
    /// A site (its robots.txt and sitemap.xml are found), a sitemap or sitemap index URL (gzip
    /// too), or a project-relative .xml, .xml.gz or .txt file; the project's startUrl when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Take the project bookmarks instead of a sitemap.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bookmarks: bool,
    /// Breakpoint ids; every Chromium breakpoint when absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breakpoints: Vec<String>,
    /// An environment id: the sitemap's URLs move to its origin, the path kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// At most this many pages; the setting sitemap.maxPages when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    /// Only pages matching one of these globs over the path (or `re:<regex>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<String>>,
    /// Leave out pages matching one of these patterns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
    /// Compare each shot with its visual-regression baseline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub compare: bool,
    /// A runId to continue: what is done stays, failed shots are tried again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
    /// The session (cookies, storage) every page is captured in; each breakpoint's own when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Hide cookie banners and chat widgets; the setting capture.hideBanners when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; the setting capture.freezeAnimations when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
}

/// `sitemap_status`, `sitemap_cancel` and `sitemap_gallery`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SitemapRunParams {
    /// The run (`sitemap_capture` answers it); the latest (for cancel: the running) one when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

fn args_of(params: &impl Serialize) -> Result<serde_json::Value, ToolFailure> {
    serde_json::to_value(params).map_err(|error| ToolFailure::new("internal", error.to_string()))
}

#[tool_router(router = sitemap_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Capture every page of a sitemap in every breakpoint, in hidden render views with each breakpoint's emulation, without blocking the grid. url: a site (robots.txt and sitemap.xml are found), a sitemap or sitemap index (gzip too), or a project-relative .xml, .xml.gz or .txt URL list; the project's startUrl when absent; bookmarks: true takes the project bookmarks. Optional: breakpoints (ids), env (an environment id: URLs move to its origin), max, include/exclude (globs or re:regex), compare (against the visual-regression baselines), resume (a runId: done shots stay, failed ones are retried, nothing is captured twice), session. Answers once the pages are found - the capture goes on in the background - with runId, the folder .rbrowser/out/sitemap-capture/<runId>/ (PNGs, run.json, index.html gallery) and the shots pending; follow it with sitemap_status. One run at a time. hideBanners and freezeAnimations override capture.hideBanners and capture.freezeAnimations (banners hidden, animations frozen before each capture, undone after)."
    )]
    pub(crate) async fn sitemap_capture(
        &self,
        Parameters(params): Parameters<SitemapCaptureParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            let answer = self.backend.host_tool(CAPTURE_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "The state of a sitemap capture (runId; the latest when absent): running, cancelled or finished; pages, breakpoints, total, done, failed, pending and the estimated seconds left; the folder and the gallery once written; the pages with an HTTP error, a redirect or a failed shot."
    )]
    pub(crate) async fn sitemap_status(
        &self,
        Parameters(params): Parameters<SitemapRunParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            let answer = self.backend.host_tool(STATUS_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Cancel a running sitemap capture (runId; the running one when absent). What was captured stays and run.json is written; sitemap_capture with resume: runId takes the rest without capturing a page twice. Answers the runId and whether it was still running."
    )]
    pub(crate) async fn sitemap_cancel(
        &self,
        Parameters(params): Parameters<SitemapRunParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            self.backend.host_tool(CANCEL_TOOL, args).await
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Open the gallery of a sitemap capture (runId; the latest run when absent) for the person in a window of rBrowser's own - never in the system browser or the grid's views. A running capture opens too; the window follows it. Answers the runId, the gallery file index.html (null until the capture has ended and written it) and where it opened (window)."
    )]
    pub(crate) async fn sitemap_gallery(
        &self,
        Parameters(params): Parameters<SitemapRunParams>,
    ) -> ToolResult {
        let result = async {
            let args = args_of(&params)?;
            let answer = self.backend.host_tool(GALLERY_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "sitemap_tests.rs"]
mod tests;
