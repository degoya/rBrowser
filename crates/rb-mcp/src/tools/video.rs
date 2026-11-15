//! Video recording (RB-080-02): `record_video`, run by the host
//! (`crates/rbrowser/src/video/defs.rs`). One recording at a time over the views of `target`
//! (resolved here, so a breakpoint id names every view showing it, and every view starts in the
//! same recording); `stop` answers the files. The tool never downloads ffmpeg: without one it
//! answers `video.ffmpeg_missing` and the person downloads it in rBrowser. It covers the
//! commands `capture.video.toggle` and `capture.video.toggle_all`.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve};

/// The host tool behind the tool.
const RECORD_TOOL: &str = "record_video";

/// What `record_video` does.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum VideoAction {
    /// Start recording the target's views.
    Start,
    /// Stop and answer the files.
    Stop,
    /// The recording, the ffmpeg in use and the last result.
    #[default]
    Status,
}

/// `record_video`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordVideoParams {
    /// `start`, `stop` or `status`.
    pub action: VideoAction,
    /// For `start`: `"all"` (default), a view id or a breakpoint id.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = video_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Record views as video files (Chromium views; Firefox and WebKit are not recorded). action start: target \"all\" (default), a view id or a breakpoint id - all start and stop together, one file per view in .rbrowser/out/video/<date-time>/<breakpoint>.<mp4|webm> at the viewport times the DPR (bounded), capture.video.fps frames per second; answers the views, the folder and maxDurationSec, after which the recording stops by itself. action stop: stops and answers each file's path, frames and seconds, or its error. action status: whether a recording runs, the ffmpeg in use and the last result. rBrowser needs ffmpeg: without it start answers video.ffmpeg_missing - the LGPL download is the person's decision in rBrowser, never MCP's."
    )]
    pub(crate) async fn record_video(
        &self,
        Parameters(params): Parameters<RecordVideoParams>,
    ) -> ToolResult {
        let result = async {
            let args = match params.action {
                VideoAction::Start => {
                    let views = self.read(|backend| backend.views()).await?;
                    let ids = resolve(params.target.as_ref(), &views)?.view_ids(&views);
                    json!({ "action": "start", "views": ids })
                }
                VideoAction::Stop if params.target.is_some() => {
                    return Err(ToolFailure::new(
                        "invalid_arguments",
                        "stop ends the whole recording and takes no target",
                    ));
                }
                VideoAction::Stop => json!({ "action": "stop" }),
                VideoAction::Status => json!({ "action": "status" }),
            };
            let answer: Value = self.backend.host_tool(RECORD_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "video_tests.rs"]
mod tests;
