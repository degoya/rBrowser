//! The recordings as the Videos panel shows them (RB-110-12): the answer to `getVideos`,
//! `deleteVideo` and `convertVideo`; `videosChanged` tells the panel to ask again. The MP4
//! exports and WebM conversions queued and running (RB-090-52) come with the list. TypeScript
//! mirror: `ui/src/ipc/videos.ts`.

use serde::{Deserialize, Serialize};

/// The recordings of the open project, newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoList {
    /// A project is open: recordings need one.
    pub project: bool,
    /// The recordings' folder, `<root>/.rbrowser/out/video`, while a project is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// `capture.video.autoplay`: a chosen recording starts playing.
    pub autoplay: bool,
    /// Why the folder could not be read (a link on the way, a disk error); the list is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The recordings, newest first.
    pub recordings: Vec<VideoRecording>,
    /// A WebM can be exported as MP4 here (RB-090-52): Windows (`h264_mf`) and macOS
    /// (`h264_videotoolbox`) have an H.264 encoder in the LGPL ffmpeg, Linux has none.
    #[serde(default)]
    pub mp4_export: bool,
    /// The exports and conversions waiting and running, the running one first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversions: Vec<VideoConversion>,
}

/// What a video is made into (RB-090-52): an MP4 of a WebM to share, a WebM of an old MP4
/// recording to play in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoTarget {
    /// MP4 with H.264.
    Mp4,
    /// WebM with VP9 (VP8 where the ffmpeg has no VP9).
    Webm,
}

/// One export or conversion, waiting or running.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoConversion {
    /// The recording's id.
    pub recording: String,
    /// The video it is made from, `mobile.webm`.
    pub file: String,
    /// The file it makes beside it, `mobile.mp4`.
    pub target: String,
    /// What it makes.
    pub to: VideoTarget,
    /// It runs now (else it waits for the one before).
    pub running: bool,
    /// How far it is, 0 to 100, once ffmpeg said the video's length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
}

/// One recording: a folder with one file per view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRecording {
    /// The folder name, `2026-11-16_14-03-07`.
    pub id: String,
    /// The folder's path.
    pub path: String,
    /// Its files, by name.
    pub files: Vec<VideoFile>,
}

/// One recorded view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFile {
    /// `mobile.mp4`.
    pub name: String,
    /// The breakpoint (or size) it shows: the name without its extension.
    pub breakpoint: String,
    /// Its size.
    pub bytes: u64,
    /// The file's path.
    pub path: String,
    /// Where the panel plays it: `rbrowser://video/<id>/<name>`.
    pub url: String,
    /// Its poster, its first frame as a JPEG, on the same route; absent when the recorder made
    /// none (a recording from before posters, or ffmpeg failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
}
