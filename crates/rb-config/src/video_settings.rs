//! Capture > Video (RB-080-02; retention and autoplay of the Videos panel, RB-110-12):
//! `capture.video.*`, which the project config and the user
//! settings may both hold (the project wins per key, as the rest of `capture`), and
//! `tools.ffmpegPath`, an ffmpeg of the user's own - user settings only, so a checked-in
//! project can never name a program rBrowser runs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `capture.video`. A key not set is `None`, so the project can win per key. Recordings are
/// always WebM: `capture.video.format` is gone (RB-090-52, `crate::retired`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoConfig {
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
    /// The Videos panel (RB-110-12): after a recording, only the newest this many recordings
    /// rBrowser made are kept and older ones are deleted from `.rbrowser/out/video/`; 0 keeps
    /// all. Between the project's value and the user's the one that keeps more wins.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 1000), extend("default" = 0))]
    pub keep_recordings: Option<u32>,
    /// After a recording, recordings rBrowser made older than this many days are deleted from
    /// `.rbrowser/out/video/`; 0 keeps all. Between the project's value and the user's the one
    /// that keeps longer wins.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 3650), extend("default" = 0))]
    pub keep_days: Option<u32>,
    /// The Videos panel starts a recording as soon as it is chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("default" = false))]
    pub autoplay: Option<bool>,
}

impl VideoConfig {
    /// These values (the project's) over `user`: a key set here wins.
    #[must_use]
    pub fn over(&self, user: &VideoConfig) -> VideoConfig {
        VideoConfig {
            fps: self.fps.or(user.fps),
            max_duration_sec: self.max_duration_sec.or(user.max_duration_sec),
            quality: self.quality.or(user.quality),
            keep_recordings: self.keep_recordings.or(user.keep_recordings),
            keep_days: self.keep_days.or(user.keep_days),
            autoplay: self.autoplay.or(user.autoplay),
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
