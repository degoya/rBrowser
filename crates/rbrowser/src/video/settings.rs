//! Capture > Video in effect: `capture.video.*` of the project over the user's, each value held
//! to its bounds, and `tools.ffmpegPath` from the user settings only - a project never names the
//! program rBrowser runs. The output folder is the project's `.rbrowser/out/video/<date-time>/`,
//! made of real folders only ([`make_run_folder`]).

use std::path::{Path, PathBuf};
use std::time::Duration;

use rb_config::{ToolsSettings, VideoConfig, VideoFormat};
use rb_engine::ViewId;
use rb_visual::naming::sanitize_segment;
use rb_visual::sitemap_store::RunStore;
use rb_visual::{Stamp, VisualError};

use super::encoder::extension;

/// `capture.video.fps` when not set.
pub const DEFAULT_FPS: u32 = 30;
/// `capture.video.maxDurationSec` when not set.
pub const DEFAULT_MAX_SECS: u32 = 300;
/// The longest recording.
pub const MAX_SECS: u32 = 3600;
/// `capture.video.quality` when not set.
pub const DEFAULT_QUALITY: u32 = 80;
/// The folder of the recordings below the project root.
pub const OUT_DIR: [&str; 3] = [".rbrowser", "out", "video"];

/// The video settings in effect.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoSettings {
    /// MP4 or WebM.
    pub format: VideoFormat,
    /// Frames per second, 1 to 60.
    pub fps: u32,
    /// The longest recording.
    pub max_duration: Duration,
    /// 1 to 100.
    pub quality: u32,
    /// The user's own ffmpeg, an absolute path.
    pub ffmpeg_path: Option<PathBuf>,
}

impl VideoSettings {
    /// The project's values over the user's, bounded; the user's `tools`.
    pub fn resolve(project: &VideoConfig, user: &VideoConfig, tools: &ToolsSettings) -> Self {
        let merged = project.over(user);
        let ffmpeg_path = tools
            .ffmpeg_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .filter(|path| {
                let absolute = path.is_absolute() || looks_like_windows_absolute(path);
                if !absolute {
                    tracing::warn!("tools.ffmpegPath is not an absolute path and is not used");
                }
                absolute
            });
        Self {
            format: merged.format.unwrap_or_default(),
            fps: merged
                .fps
                .unwrap_or(DEFAULT_FPS)
                .clamp(1, super::pacing::MAX_FPS),
            max_duration: Duration::from_secs(u64::from(
                merged
                    .max_duration_sec
                    .unwrap_or(DEFAULT_MAX_SECS)
                    .clamp(1, MAX_SECS),
            )),
            quality: merged.quality.unwrap_or(DEFAULT_QUALITY).clamp(1, 100),
            ffmpeg_path,
        }
    }
}

/// `C:\...` or `\\server\...` read on Linux too (the tests and WSL paths in the settings).
fn looks_like_windows_absolute(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    matches!(bytes, [letter, b':', b'\\' | b'/', ..] if letter.is_ascii_alphabetic())
}

/// The folder of one recording: `<root>/.rbrowser/out/video/<date-time>`.
pub fn run_folder(root: &Path, stamp: &Stamp) -> PathBuf {
    let mut folder = root.to_path_buf();
    folder.extend(OUT_DIR);
    folder.push(stamp.date_time());
    folder
}

/// Makes the folder of one recording ([`run_folder`]), each folder on the way a real one - never
/// a link or junction a project checked in, which could lead out of it - and answers it.
///
/// # Errors
///
/// A link or file on the way, or a folder that cannot be made.
pub fn make_run_folder(root: &Path, stamp: &Stamp) -> Result<PathBuf, VisualError> {
    RunStore::open_in(root, &OUT_DIR, &stamp.date_time(), true)?
        .map(|store| store.dir().to_path_buf())
        .ok_or_else(|| VisualError::Folder("the recording's folder was not made".to_owned()))
}

/// The file of each view: `<breakpoint>.<ext>` (the size for a free one), made unique with the
/// view id where two views show the same breakpoint.
pub fn file_names(views: &[(ViewId, String)], format: VideoFormat) -> Vec<(ViewId, String)> {
    let ext = extension(format);
    let stems: Vec<String> = views
        .iter()
        .map(|(_, name)| sanitize_segment(name))
        .collect();
    views
        .iter()
        .zip(&stems)
        .map(|((view, _), stem)| {
            let shared = stems.iter().filter(|other| *other == stem).count() > 1;
            let name = if shared {
                format!("{stem}-{}.{ext}", view.0)
            } else {
                format!("{stem}.{ext}")
            };
            (*view, name)
        })
        .collect()
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
