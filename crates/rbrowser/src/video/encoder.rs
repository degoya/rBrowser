//! The encoder: ffmpeg reading the screencast's JPEG frames from its stdin (`-f image2pipe`)
//! and writing one WebM file (RB-090-52: always WebM, which plays in the Videos panel and every
//! browser on every platform) - VP9 (`libvpx-vp9`, `realtime`), or VP8 (`libvpx`) where the
//! ffmpeg in use has no VP9 ([`WebmCodec`], chosen from `ffmpeg -encoders`,
//! `super::ffmpeg::webm_codec`); never `libx264` (GPL). The frame is scaled to a fixed even size
//! (4:2:0 needs even sides), and an existing file is never overwritten (`-n`).
//!
//! The encoders never take the views' CPU: each runs below the app's priority
//! (`rb_process::low_priority_command`) with [`encoder_threads`] threads, so all of them
//! together use at most half the logical cores.
//!
//! [`FfmpegSink`] starts the program without a console window (`rb_process`), writes the frames
//! asynchronously - a frame not taken within [`WRITE_TIMEOUT`] stops the program, so a hung
//! ffmpeg cannot hold the recording - and at [`FrameSink::finish`] closes stdin and waits for the
//! file; the error names the end of ffmpeg's error output. [`build_info`] reads `ffmpeg -version`: a build
//! configured with `--enable-gpl` or `--enable-nonfree` is no LGPL build.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::process::{Child, ChildStdin};
use tokio::task::JoinHandle;

/// How long ffmpeg may take to finish the file after the last frame.
pub const FINISH_TIMEOUT: Duration = Duration::from_secs(120);
/// How long ffmpeg may take to accept one frame; a program that takes none for this long hangs
/// and is stopped.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// The most bytes of ffmpeg's error output kept for the error.
const STDERR_TAIL: usize = 4096;
/// The extension of every recording.
pub const EXTENSION: &str = "webm";
/// VP8's bitrate ceiling: its constant quality needs one (VP9's `-b:v 0` has none).
pub const VP8_MAX_BITRATE: &str = "12M";

/// The WebM video codec of a recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WebmCodec {
    /// VP9 (`libvpx-vp9`): smaller files at the same quality.
    Vp9,
    /// VP8 (`libvpx`): where the ffmpeg in use has no VP9.
    Vp8,
}

impl WebmCodec {
    /// ffmpeg's name of the encoder.
    #[must_use]
    pub fn encoder(self) -> &'static str {
        match self {
            Self::Vp9 => "libvpx-vp9",
            Self::Vp8 => "libvpx",
        }
    }

    /// The codec's own arguments after `-c:v`: real-time speed, `threads` threads, constant
    /// quality from `quality` (1 to 100).
    #[must_use]
    pub fn args(self, quality: u32, threads: u32) -> Vec<String> {
        let mut args = vec![
            "-c:v".to_owned(),
            self.encoder().to_owned(),
            "-deadline".to_owned(),
            "realtime".to_owned(),
            "-cpu-used".to_owned(),
            "8".to_owned(),
            "-threads".to_owned(),
            threads.max(1).to_string(),
        ];
        match self {
            Self::Vp9 => args.extend(["-row-mt", "1", "-b:v", "0"].map(str::to_owned)),
            Self::Vp8 => args.extend(["-b:v".to_owned(), VP8_MAX_BITRATE.to_owned()]),
        }
        args.extend(["-crf".to_owned(), crf(quality).to_string()]);
        args
    }
}

/// What one file is encoded with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodeSpec {
    /// The codec.
    pub codec: WebmCodec,
    /// Frames per second.
    pub fps: u32,
    /// 1 to 100.
    pub quality: u32,
    /// The frame width in device pixels.
    pub width: u32,
    /// The frame height in device pixels.
    pub height: u32,
    /// The encoder's threads ([`encoder_threads`]).
    pub threads: u32,
}

/// `value` rounded down to an even number, at least 2.
pub fn even(value: u32) -> u32 {
    (value & !1).max(2)
}

/// The VP9/VP8 constant rate factor of a quality from 1 to 100 (63 worst, 15 best).
pub fn crf(quality: u32) -> u32 {
    63 - quality.min(100) * 48 / 100
}

/// The threads of each of `encoders` encoders running at once on `logical` logical cores: all
/// of them together at most half the cores, at least one each.
#[must_use]
pub fn encoder_threads(logical: usize, encoders: usize) -> u32 {
    let each = (logical / 2) / encoders.max(1);
    u32::try_from(each).unwrap_or(u32::MAX).max(1)
}

/// The logical cores of this machine; 1 when the system does not say.
#[must_use]
pub fn logical_cores() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}

/// The arguments of one encoding into `output`.
pub fn ffmpeg_args(spec: &EncodeSpec, output: &Path) -> Vec<OsString> {
    let fps = spec.fps.to_string();
    let quality = spec.quality.clamp(1, 100);
    let filter = format!(
        "scale={}:{}:flags=bicubic,format=yuv420p",
        even(spec.width),
        even(spec.height)
    );
    let input: [&str; 16] = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-n",
        "-f",
        "image2pipe",
        "-framerate",
        fps.as_str(),
        "-c:v",
        "mjpeg",
        "-i",
        "pipe:0",
        "-vf",
        filter.as_str(),
        "-r",
        fps.as_str(),
    ];
    let mut args: Vec<OsString> = input.iter().map(OsString::from).collect();
    args.extend(
        spec.codec
            .args(quality, spec.threads)
            .into_iter()
            .map(OsString::from),
    );
    args.push(output.as_os_str().to_owned());
    args
}

/// What `ffmpeg -version` says of a build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildInfo {
    /// The version (`n9.0.2-...`); empty when the output is no ffmpeg's.
    pub version: String,
    /// Whether a configuration line was found.
    pub configured: bool,
    /// Configured with `--enable-gpl`.
    pub gpl: bool,
    /// Configured with `--enable-nonfree`.
    pub nonfree: bool,
}

impl BuildInfo {
    /// An LGPL build: configured, neither GPL nor non-free.
    pub fn is_lgpl(&self) -> bool {
        self.configured && !self.gpl && !self.nonfree
    }
}

/// Reads the output of `ffmpeg -version`.
pub fn build_info(output: &str) -> BuildInfo {
    let version = output
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("ffmpeg version "))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_default()
        .to_owned();
    let configuration = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("configuration:"));
    let flag = |name: &str| {
        configuration.is_some_and(|line| line.split_whitespace().any(|word| word == name))
    };
    BuildInfo {
        version,
        configured: configuration.is_some(),
        gpl: flag("--enable-gpl"),
        nonfree: flag("--enable-nonfree"),
    }
}

/// Where the paced frames go.
#[async_trait]
pub trait FrameSink: Send {
    /// Writes one JPEG frame.
    async fn write(&mut self, jpeg: &[u8]) -> std::io::Result<()>;
    /// No more frames: the file is finished.
    async fn finish(self: Box<Self>) -> Result<(), String>;
}

/// ffmpeg as a [`FrameSink`].
pub struct FfmpegSink {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr: JoinHandle<String>,
    write_timeout: Duration,
}

impl FfmpegSink {
    /// Starts `ffmpeg` encoding `spec` into `output`.
    ///
    /// # Errors
    ///
    /// The program could not be started.
    pub fn spawn(ffmpeg: &Path, spec: &EncodeSpec, output: &Path) -> Result<Self, String> {
        Self::spawn_program(ffmpeg, &ffmpeg_args(spec, output))
    }

    /// Starts `program` with `args` below the app's priority, its stdin the frames.
    pub(crate) fn spawn_program(program: &Path, args: &[OsString]) -> Result<Self, String> {
        let mut child = rb_process::low_priority_command(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("{} could not be started: {error}", program.display()))?;
        let stdin = child.stdin.take();
        let stderr = child.stderr.take();
        let stderr = tokio::spawn(async move {
            let mut text = Vec::new();
            if let Some(mut stderr) = stderr {
                let mut chunk = [0_u8; 1024];
                while let Ok(read) = stderr.read(&mut chunk).await {
                    if read == 0 {
                        break;
                    }
                    text.extend_from_slice(chunk.get(..read).unwrap_or_default());
                    let excess = text.len().saturating_sub(STDERR_TAIL);
                    text.drain(..excess);
                }
            }
            String::from_utf8_lossy(&text).trim().to_owned()
        });
        Ok(Self {
            child,
            stdin,
            stderr,
            write_timeout: WRITE_TIMEOUT,
        })
    }

    /// The same sink with another [`WRITE_TIMEOUT`] (the Unix tests).
    #[cfg(all(test, unix))]
    pub(crate) fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = timeout;
        self
    }
}

#[async_trait]
impl FrameSink for FfmpegSink {
    async fn write(&mut self, jpeg: &[u8]) -> std::io::Result<()> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        };
        match tokio::time::timeout(self.write_timeout, stdin.write_all(jpeg)).await {
            Ok(written) => written,
            Err(_) => {
                // A program that takes no frame hangs: stop it, so `finish` returns at once.
                self.stdin = None;
                let _ = self.child.start_kill();
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!(
                        "ffmpeg took no frame for {} s and was stopped",
                        self.write_timeout.as_secs()
                    ),
                ))
            }
        }
    }

    async fn finish(mut self: Box<Self>) -> Result<(), String> {
        if let Some(mut stdin) = self.stdin.take() {
            // A program that stopped early fails below with its own words.
            let _ = stdin.shutdown().await;
        }
        let status = match tokio::time::timeout(FINISH_TIMEOUT, self.child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(error)) => return Err(format!("ffmpeg could not be waited for: {error}")),
            Err(_) => {
                let _ = self.child.kill().await;
                return Err(format!(
                    "ffmpeg did not finish within {} s",
                    FINISH_TIMEOUT.as_secs()
                ));
            }
        };
        let stderr = (&mut self.stderr).await.unwrap_or_default();
        if status.success() {
            Ok(())
        } else {
            Err(format!("ffmpeg ended with {status}: {stderr}"))
        }
    }
}

#[cfg(test)]
#[path = "encoder_tests.rs"]
mod tests;
