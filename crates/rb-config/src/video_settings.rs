//! Capture > Video (RB-080-02): `capture.video.*`, which the project config and the user
//! settings may both hold (the project wins per key, as the rest of `capture`), and
//! `tools.ffmpegPath`, an ffmpeg of the user's own - user settings only, so a checked-in
//! project can never name a program rBrowser runs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The container and codec of a recording.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VideoFormat {
    /// MP4 with H.264 from Windows Media Foundation (`h264_mf`).
    #[default]
    Mp4,
    /// WebM with VP9 (`libvpx-vp9`).
    Webm,
}

/// `capture.video`. A key not set is `None`, so the project can win per key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoConfig {
    /// MP4 (H.264 through Windows Media Foundation) or WebM (VP9).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = "mp4"))]
    pub format: Option<VideoFormat>,
    /// Frames per second of the file; a frame that did not change is repeated.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 60), extend("default" = 30))]
    pub fps: Option<u32>,
    /// A recording stops by itself after this many seconds and its files are written.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 3600), extend("default" = 300))]
    pub max_duration_sec: Option<u32>,
    /// Image quality from 1 to 100, of the captured frames and of the encoder.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 100), extend("default" = 80))]
    pub quality: Option<u32>,
}

impl VideoConfig {
    /// These values (the project's) over `user`: a key set here wins.
    #[must_use]
    pub fn over(&self, user: &VideoConfig) -> VideoConfig {
        VideoConfig {
            format: self.format.or(user.format),
            fps: self.fps.or(user.fps),
            max_duration_sec: self.max_duration_sec.or(user.max_duration_sec),
            quality: self.quality.or(user.quality),
        }
    }
}

/// `tools` of the user settings: the managed tools' own overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsSettings {
    /// An ffmpeg of your own: the absolute path of the program. Empty uses the LGPL build
    /// rBrowser downloads when you run "Video: download ffmpeg".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ffmpeg_path: Option<String>,
}

#[cfg(test)]
#[path = "video_settings_tests.rs"]
mod tests;
