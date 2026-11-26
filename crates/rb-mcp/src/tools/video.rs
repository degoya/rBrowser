//! Video recording (RB-080-02): `record_video`, run by the host
//! (`crates/rbrowser/src/video/defs.rs`). One recording at a time over the views of `target`
//! (resolved here, so a breakpoint id names every view showing it, and every view starts in the
//! same recording); `stop` answers the files. The tool never downloads ffmpeg: without one it
//! answers `video.ffmpeg_missing` and the person downloads it in rBrowser. It covers the
//! commands `capture.video.toggle`, `capture.video.toggle_all` and `capture.video.pause`
//! (`pause`, `resume`, RB-090-68).
//!
//! `video_list` (RB-110-12) lists the recordings the Videos panel shows; a read. No tool
//! deletes one: that is the person's, after the panel's confirmation - an agent keeps the folder
//! small with `capture.video.keepRecordings` and `capture.video.keepDays`.
//!
//! The MP4 export and the WebM conversion of the Videos panel (RB-090-52) have no tool: they
//! are the person's way to share or watch a file; an agent works with the WebM `video_list`
//! names and sees the jobs there.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::NoArgs;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{TargetArg, resolve};

/// The host tool behind the tool.
const RECORD_TOOL: &str = "record_video";
/// The host tool behind the list (RB-110-12).
const LIST_TOOL: &str = "video_list";

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
    /// Pause the recording: the files leave the pause out (RB-090-68).
    Pause,
    /// Resume a paused recording.
    Resume,
    /// The recording, the ffmpeg in use and the last result.
    #[default]
    Status,
}

/// `record_video`.
#[derive(Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordVideoParams {
    /// `start`, `stop`, `pause`, `resume` or `status`.
    pub action: VideoAction,
    /// `"all"` (default), a view id or a breakpoint id. For `start` only.
    #[serde(default, skip_serializing)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = video_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Record views as WebM video (Chromium views only). action start: every targeted view starts and stops together, one file per view in .rbrowser/out/video/<date-time>/<breakpoint>.webm at capture.video.fps; answers the views, the folder, the codec and maxDurationSec, after which it stops by itself. stop: each file's path, frames, seconds and dropped frames, or its error. pause/resume: the pause is left out of every file; answers paused and changed (video.not_recording without a recording). status: whether a recording runs, the ffmpeg in use and the last result. Needs ffmpeg (video.ffmpeg_missing); its download is the person's decision, never MCP's."
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
                VideoAction::Stop | VideoAction::Pause | VideoAction::Resume
                    if params.target.is_some() =>
                {
                    return Err(ToolFailure::new(
                        "args.invalid",
                        "stop, pause and resume act on the whole recording and take no target",
                    ));
                }
                VideoAction::Stop => json!({ "action": "stop" }),
                VideoAction::Pause => json!({ "action": "pause" }),
                VideoAction::Resume => json!({ "action": "resume" }),
                VideoAction::Status => json!({ "action": "status" }),
            };
            let answer: Value = self.backend.host_tool(RECORD_TOOL, args).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "The video recordings of the open project, newest first: project (false without one), folder (.rbrowser/out/video), running, autoplay (capture.video.autoplay) and per recording its id (<date-time>), path and files (name, breakpoint, bytes, path; a WebM per view, an MP4 beside it is the person's export), mp4Export and the conversions running. No tool deletes a recording: the person does, and capture.video.keepRecordings and keepDays prune old ones."
    )]
    pub(crate) async fn video_list(&self, _args: Parameters<NoArgs>) -> ToolResult {
        let result = async {
            let answer: Value = self.backend.host_tool(LIST_TOOL, json!({})).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "video_tests.rs"]
mod tests;
