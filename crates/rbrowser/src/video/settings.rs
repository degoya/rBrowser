//! Capture > Video in effect: `capture.video.*` of the project over the user's, each value held
//! to its bounds, and `tools.ffmpegPath` from the user settings only - a project never names the
//! program rBrowser runs. The output folder is the project's `.rbrowser/out/video/<date-time>/`,
//! made of real folders only ([`make_run_folder`]).

use std::path::{Path, PathBuf};
use std::time::Duration;

use rb_config::{ToolsSettings, VideoConfig};
use rb_engine::ViewId;
use rb_visual::naming::sanitize_segment;
use rb_visual::sitemap_store::RunStore;
use rb_visual::{Stamp, VisualError};

use super::encoder::EXTENSION;

/// `capture.video.fps` when not set.
pub const DEFAULT_FPS: u32 = 30;
/// `capture.video.maxDurationSec` when not set.
pub const DEFAULT_MAX_SECS: u32 = 300;
/// The longest recording.
pub const MAX_SECS: u32 = 3600;
/// `capture.video.quality` when not set.
pub const DEFAULT_QUALITY: u32 = 80;
/// The most recordings `capture.video.keepRecordings` keeps (RB-110-12).
pub const MAX_KEEP_RECORDINGS: u32 = 1000;
/// The most days `capture.video.keepDays` keeps.
pub const MAX_KEEP_DAYS: u32 = 3650;
/// The folder of the recordings below the project root.
pub const OUT_DIR: [&str; 3] = [".rbrowser", "out", "video"];

/// The video settings in effect.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoSettings {
    /// Frames per second, 1 to 60.
    pub fps: u32,
    /// The longest recording.
    pub max_duration: Duration,
    /// 1 to 100.
    pub quality: u32,
    /// The user's own ffmpeg, an absolute path.
    pub ffmpeg_path: Option<PathBuf>,
    /// After a recording only the newest this many are kept; 0 keeps all (RB-110-12).
    pub keep_recordings: u32,
    /// After a recording those older than this many days are deleted; 0 keeps all.
    pub keep_days: u32,
    /// The Videos panel plays a recording as soon as it is chosen.
    pub autoplay: bool,
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
            keep_recordings: lenient(project.keep_recordings, user.keep_recordings)
                .min(MAX_KEEP_RECORDINGS),
            keep_days: lenient(project.keep_days, user.keep_days).min(MAX_KEEP_DAYS),
            autoplay: merged.autoplay.unwrap_or(false),
        }
    }
}

/// A retention limit in effect (RB-110-12, security review): the user's value (its default 0,
/// keep all) and the project's, the one that keeps more - 0 above every number. A checked-in
/// project can keep more than the person asked for, never delete more.
fn lenient(project: Option<u32>, user: Option<u32>) -> u32 {
    let user = user.unwrap_or(0);
    match project {
        None => user,
        Some(0) => 0,
        Some(_) if user == 0 => 0,
        Some(project) => project.max(user),
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

/// The file of each view: `<breakpoint>.webm` (the size for a free one), made unique with the
/// view id where two views show the same breakpoint.
pub fn file_names(views: &[(ViewId, String)]) -> Vec<(ViewId, String)> {
    let ext = EXTENSION;
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
