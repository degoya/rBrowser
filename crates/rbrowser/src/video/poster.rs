//! The poster of each recorded video (RB-110-12, after the acceptance of 2026-11-16): once a
//! recording is saved, ffmpeg - the same one that recorded it - grabs its first frame into
//! `<breakpoint>.jpg` beside it, scaled to at most [`WIDTH`] pixels wide. The Videos panel shows
//! it as the recording's thumbnail and as the player's poster, so a video shows a picture before
//! it plays - and also when it cannot play in rBrowser at all (an MP4: rBrowser's Chromium has no
//! H.264 decoder, ffmpeg's LGPL build has one).
//!
//! A poster is held to the videos' rules: registered with its video and its stamp
//! (`super::panel::register_saved`), served by the same route only while it is the file the
//! recorder left (`super::serve`), deleted with its video (`super::library::delete`). ffmpeg
//! runs with `-n`: a file already there under the poster's name is never overwritten, so
//! nothing but what ffmpeg wrote is registered. A poster that fails is logged and left out;
//! the video is listed without one.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use super::library::{breakpoint_of, is_plain, is_video_name};

/// A poster's extension, as the recorder writes it.
pub const EXTENSION: &str = "jpg";
/// A poster's MIME type on the video route.
pub const MIME: &str = "image/jpeg";
/// The widest poster in pixels; a narrower video keeps its width.
pub const WIDTH: u32 = 640;
/// How long ffmpeg may take for one poster.
const TIMEOUT: Duration = Duration::from_secs(20);
/// The most of ffmpeg's error output kept for the log.
const MAX_ERROR_CHARS: usize = 300;

/// The poster of the video file `video`: `<breakpoint>.jpg`; `None` for a name that is no video's.
#[must_use]
pub fn poster_name(video: &str) -> Option<String> {
    is_video_name(video).then(|| format!("{}.{EXTENSION}", breakpoint_of(video)))
}

/// Whether `name` has a poster's form: a plain name of the recorder's characters ending in
/// `.jpg` (lower case, as the recorder writes it).
#[must_use]
pub fn is_poster_name(name: &str) -> bool {
    name.strip_suffix(".jpg")
        .is_some_and(|stem| !stem.is_empty())
        && is_plain(name)
}

/// ffmpeg's arguments for the poster `output` of the video `video`: its first frame, scaled
/// down to [`WIDTH`] and from the video's limited range BT.709 (`super::encoder`) to a JPEG's
/// full range BT.601, never over a file already there.
#[must_use]
pub fn poster_args(video: &Path, output: &Path) -> Vec<OsString> {
    let scale = format!("scale='min({WIDTH},iw)':-2:out_range=pc:out_color_matrix=bt601");
    let mut args: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-n", "-i"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.push(video.as_os_str().to_owned());
    args.extend(
        [
            "-frames:v",
            "1",
            "-vf",
            scale.as_str(),
            // Full-range YUV, which every ffmpeg's JPEG encoder takes (the scale converts to it).
            "-pix_fmt",
            "yuvj420p",
            "-q:v",
            "4",
            "-update",
            "1",
        ]
        .into_iter()
        .map(OsString::from),
    );
    args.push(output.as_os_str().to_owned());
    args
}

/// Writes the poster of each video in `videos` (names in `folder`, the recording's) with the
/// ffmpeg at `ffmpeg`; answers the names of the posters written.
pub async fn write_all(ffmpeg: &Path, folder: &Path, videos: &[String]) -> Vec<String> {
    let mut written = Vec::new();
    for video in videos {
        let Some(name) = poster_name(video) else {
            continue;
        };
        match write_one(ffmpeg, &folder.join(video), &folder.join(&name)).await {
            Ok(()) => written.push(name),
            Err(error) => {
                tracing::warn!(%error, poster = %name, "a recorded video got no poster");
            }
        }
    }
    written
}

async fn write_one(ffmpeg: &Path, video: &Path, output: &Path) -> Result<(), String> {
    // Below the app's priority, as every encoder (RB-090-52).
    let run = rb_process::low_priority_command(ffmpeg)
        .args(poster_args(video, output))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(TIMEOUT, run)
        .await
        .map_err(|_| "ffmpeg did not finish the poster in time".to_owned())?
        .map_err(|error| format!("ffmpeg could not be started: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr: String = String::from_utf8_lossy(&output.stderr)
        .trim()
        .chars()
        .take(MAX_ERROR_CHARS)
        .collect();
    Err(format!("ffmpeg ended with {}: {stderr}", output.status))
}

#[cfg(test)]
#[path = "poster_tests.rs"]
mod tests;
