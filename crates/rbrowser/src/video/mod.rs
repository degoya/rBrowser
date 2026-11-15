//! Video recording (RB-080-02): one, several or all Chromium views recorded at once, each into
//! its own file `.rbrowser/out/video/<date-time>/<breakpoint>.<mp4|webm>`, started and stopped
//! together by a shortcut, the palette or MCP (`record_video`).
//!
//! - [`recorder`]: `Page.startScreencast` per view, every frame acked, paced to a constant frame
//!   rate ([`pacing`]) and piped to ffmpeg ([`encoder`]: `h264_mf` or `libvpx-vp9`, never
//!   `libx264`), stopped by hand or after `capture.video.maxDurationSec`.
//! - [`ffmpeg`]: the user's own ffmpeg or the managed LGPL build (`crate::tools`), which is
//!   downloaded only by the person's command `tools.ffmpeg.install` after the consent notice.
//! - [`settings`]: Capture > Video in effect and the output names.
//! - [`app`]: the commands ([`defs`]), the shared state and the MCP tool's entry point.
//!
//! Firefox and WebKit views are not recorded; a recording names them as unavailable.

pub mod app;
pub mod defs;
pub mod encoder;
pub mod ffmpeg;
pub mod pacing;
pub mod recorder;
pub mod settings;

pub use defs::register;

use rb_engine::EngineError;

/// Why a recording did not start or stop.
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    /// No ffmpeg: neither the user's own nor the managed download.
    #[error(
        "rBrowser needs ffmpeg to record video and has none yet. In rBrowser, run the command \
         \"Video: download ffmpeg\" (Ctrl+Shift+Space X F), which says what it downloads, or set \
         tools.ffmpegPath to an ffmpeg of your own; MCP cannot start the download"
    )]
    FfmpegMissing,
    /// The ffmpeg found cannot be used.
    #[error("{0}")]
    Ffmpeg(String),
    /// A recording runs already.
    #[error("a recording is running; stop it first")]
    Busy,
    /// No recording runs.
    #[error("no recording is running")]
    NotRecording,
    /// Files need an open project.
    #[error("open a project to record video")]
    NoProject,
    /// No Chromium view to record.
    #[error("there is no Chromium view to record")]
    NoViews,
    /// The browser side.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// The folder or the encoder.
    #[error("{0}")]
    Failed(String),
}

impl VideoError {
    /// The stable code of an MCP failure.
    pub fn code(&self) -> &'static str {
        match self {
            Self::FfmpegMissing => "video.ffmpeg_missing",
            Self::Ffmpeg(_) => "video.ffmpeg_unusable",
            Self::Busy => "video.busy",
            Self::NotRecording => "video.not_recording",
            Self::NoProject => "video.no_project",
            Self::NoViews => "view_not_found",
            Self::Engine(_) | Self::Failed(_) => "video.failed",
        }
    }
}
