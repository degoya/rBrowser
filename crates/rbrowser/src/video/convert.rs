//! Exports and conversions of recorded videos (RB-090-52): what a file is made into, the name
//! it gets, ffmpeg's arguments and how far it is. The queue that runs them one at a time is
//! [`super::convert_queue`], one run [`super::convert_run`].
//!
//! - **Export as MP4** (a WebM to share - QuickTime, messengers): H.264 from the platform's
//!   encoder in the LGPL ffmpeg, Windows `h264_mf`, macOS `h264_videotoolbox`; never `libx264`
//!   (GPL). Linux has none, so the panel does not offer it and says why ([`mp4_encoder`]).
//! - **Convert to WebM** (an MP4 recorded before RB-090-52, which rBrowser's Chromium cannot
//!   play): the recorder's own codec and real-time settings ([`WebmCodec`]).
//!
//! Either is the standard stream a recording is (`super::encoder`, [`convert_filter`]): square
//! pixels at the source's display aspect, even sides, limited range, BT.709, 4:2:0, tagged.
//!
//! The result lies beside its source, `<breakpoint>.<mp4|webm>`, and is registered like a
//! recording's videos (`super::registry`), so it is served and deleted under the same rules;
//! the source stays. ffmpeg writes `<target>.part` - never a name the panel lists or the route
//! serves - with `-n`, and the file is renamed only once complete.

use std::ffi::OsString;
use std::path::Path;

use rb_ipc::VideoTarget;

use super::encoder::{STANDARD_SCALE, STANDARD_TAGS, STANDARD_TAIL, WebmCodec};
use super::library::{LibraryError, breakpoint_of, is_video_name};

/// The bitrate of an export with VideoToolbox, whose constant quality not every Mac has.
pub const MP4_BITRATE: &str = "8M";
/// The extension of a half-written result.
const PART: &str = "part";

/// The filters of an export or conversion: square pixels at the display aspect (a recording made
/// before RB-090-52's fix of 2026-11-17 has a non-square sample aspect ratio), even sides, the
/// source's range (from its tags) to limited and BT.709, then [`STANDARD_TAIL`].
#[must_use]
pub fn convert_filter() -> String {
    format!(
        "scale='trunc(iw*sar/2)*2':'trunc(ih/2)*2':flags=bicubic:{STANDARD_SCALE},{STANDARD_TAIL}"
    )
}

/// Why an export or conversion did not start or did not finish.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// The source: not a registered video of the project, a link, gone, being recorded.
    #[error(transparent)]
    Library(#[from] LibraryError),
    /// The result is there already.
    #[error("{0} is there already")]
    Exists(String),
    /// The same file is being made already.
    #[error("this video is being converted already")]
    Queued,
    /// No H.264 encoder an LGPL ffmpeg has on this platform.
    #[error(
        "exporting MP4 needs an H.264 encoder of the LGPL ffmpeg, which Windows (h264_mf) and \
         macOS (h264_videotoolbox) have and Linux has not; share the WebM, which every browser \
         plays"
    )]
    Unsupported,
    /// ffmpeg is missing, unusable or failed.
    #[error("{0}")]
    Ffmpeg(String),
}

impl ConvertError {
    /// The stable code of a panel failure.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Library(error) => error.code(),
            Self::Exists(_) => "video.exists",
            Self::Queued => "video.busy",
            Self::Unsupported => "video.unsupported",
            Self::Ffmpeg(_) => "video.failed",
        }
    }
}

/// The H.264 encoder of the LGPL ffmpeg on operating system `os` (`std::env::consts::OS`).
#[must_use]
pub fn mp4_encoder_for(os: &str) -> Option<&'static str> {
    match os {
        "windows" => Some("h264_mf"),
        "macos" => Some("h264_videotoolbox"),
        _ => None,
    }
}

/// The H.264 encoder of this platform; `None` where MP4 export is not offered (Linux).
#[must_use]
pub fn mp4_encoder() -> Option<&'static str> {
    mp4_encoder_for(std::env::consts::OS)
}

/// The extension a target is written with.
#[must_use]
pub fn extension_of(to: VideoTarget) -> &'static str {
    match to {
        VideoTarget::Mp4 => "mp4",
        VideoTarget::Webm => "webm",
    }
}

/// The name of `source` made into `to`: the same breakpoint, the other extension.
///
/// # Errors
///
/// `source` is no video's name, or already is of `to`'s format.
pub fn target_name(source: &str, to: VideoTarget) -> Result<String, ConvertError> {
    let refused = || ConvertError::Library(LibraryError::Refused(source.to_owned()));
    if !is_video_name(source) {
        return Err(refused());
    }
    let from = match to {
        VideoTarget::Mp4 => "webm",
        VideoTarget::Webm => "mp4",
    };
    let is_from = source
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(from));
    if !is_from {
        return Err(refused());
    }
    let target = format!("{}.{}", breakpoint_of(source), extension_of(to));
    if is_video_name(&target) {
        Ok(target)
    } else {
        Err(refused())
    }
}

/// The name ffmpeg writes `target` under until it is complete.
#[must_use]
pub fn part_name(target: &str) -> String {
    format!("{target}.{PART}")
}

/// What a target is encoded with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoder {
    /// MP4 with this H.264 encoder ([`mp4_encoder`]).
    Mp4(&'static str),
    /// WebM with the recorder's codec.
    Webm(WebmCodec),
}

/// ffmpeg's arguments to make `source` into `output` (the `.part` file): the first video stream
/// only, no audio, `quality` from 1 to 100, `threads` threads, progress on stdout and the
/// input's length on stderr.
#[must_use]
pub fn convert_args(
    encoder: Encoder,
    quality: u32,
    threads: u32,
    source: &Path,
    output: &Path,
) -> Vec<OsString> {
    let quality = quality.clamp(1, 100);
    let mut all: Vec<OsString> = [
        "-hide_banner",
        "-nostdin",
        "-nostats",
        // `info` for the input's `Duration:` line, the share done.
        "-loglevel",
        "info",
        "-progress",
        "pipe:1",
        "-n",
        "-i",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    all.push(source.as_os_str().to_owned());
    let mut tail: Vec<String> = ["-map", "0:v:0", "-an", "-sn", "-vf"]
        .map(str::to_owned)
        .to_vec();
    tail.push(convert_filter());
    match encoder {
        Encoder::Mp4("h264_videotoolbox") => tail.extend(
            [
                "-c:v",
                "h264_videotoolbox",
                "-allow_sw",
                "1",
                "-b:v",
                MP4_BITRATE,
                "-movflags",
                "+faststart",
                "-f",
                "mp4",
            ]
            .map(str::to_owned),
        ),
        Encoder::Mp4(name) => {
            tail.extend(["-c:v", name, "-rate_control", "quality", "-quality"].map(str::to_owned));
            tail.push(quality.to_string());
            tail.extend(["-movflags", "+faststart", "-f", "mp4"].map(str::to_owned));
        }
        Encoder::Webm(codec) => {
            tail.extend(codec.args(quality, threads));
            tail.extend(["-f", "webm"].map(str::to_owned));
        }
    }
    all.extend(tail.into_iter().map(OsString::from));
    all.extend(STANDARD_TAGS.iter().map(OsString::from));
    all.push(output.as_os_str().to_owned());
    all
}

/// The input's length in seconds from ffmpeg's `  Duration: 00:01:02.50, start: ...` line;
/// `None` for another line or `N/A`.
#[must_use]
pub fn parse_duration(line: &str) -> Option<f64> {
    let rest = line.trim_start().strip_prefix("Duration:")?.trim_start();
    let time = rest.split(',').next()?.trim();
    let mut parts = time.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// One line of ffmpeg's `-progress` output that counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Written up to this time of the video, in microseconds.
    At(u64),
    /// Done.
    End,
}

/// Reads one `-progress` line: `out_time_us=` (`out_time_ms=` counts microseconds too, as
/// ffmpeg writes it) and `progress=end`.
#[must_use]
pub fn parse_progress(line: &str) -> Option<Progress> {
    let (key, value) = line.trim().split_once('=')?;
    match key {
        "out_time_us" | "out_time_ms" => value.parse().ok().map(Progress::At),
        "progress" if value == "end" => Some(Progress::End),
        _ => None,
    }
}

/// The share done at `at_us` of a video `duration` seconds long, 0 to 99 (100 is the end);
/// `None` without a length.
#[must_use]
pub fn percent(at_us: u64, duration: Option<f64>) -> Option<u8> {
    let duration = duration.filter(|seconds| seconds.is_finite() && *seconds > 0.0)?;
    let share = (at_us as f64 / 1_000_000.0) / duration * 100.0;
    Some(share.clamp(0.0, 99.0) as u8)
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;
