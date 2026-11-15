use std::path::Path;

use super::*;

fn spec(format: VideoFormat) -> EncodeSpec {
    EncodeSpec {
        format,
        fps: 30,
        quality: 80,
        width: 1171,
        height: 2533,
    }
}

fn text(args: &[std::ffi::OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn pair(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|at| args.get(at + 1).cloned())
}

#[test]
fn mp4_encodes_with_media_foundation_never_libx264() {
    let args = text(&ffmpeg_args(&spec(VideoFormat::Mp4), Path::new("out.mp4")));
    assert_eq!(pair(&args, "-c:v").as_deref(), Some("mjpeg"), "the input");
    assert!(args.iter().any(|arg| arg == "h264_mf"), "{args:?}");
    assert!(!args.iter().any(|arg| arg.contains("x264")), "{args:?}");
    assert_eq!(pair(&args, "-f").as_deref(), Some("image2pipe"));
    assert_eq!(pair(&args, "-i").as_deref(), Some("pipe:0"));
    assert_eq!(pair(&args, "-framerate").as_deref(), Some("30"));
    assert_eq!(pair(&args, "-quality").as_deref(), Some("80"));
    assert_eq!(args.last().map(String::as_str), Some("out.mp4"));
}

#[test]
fn webm_encodes_with_vp9_and_a_crf_from_the_quality() {
    let args = text(&ffmpeg_args(
        &spec(VideoFormat::Webm),
        Path::new("out.webm"),
    ));
    assert!(args.iter().any(|arg| arg == "libvpx-vp9"), "{args:?}");
    assert_eq!(pair(&args, "-crf").as_deref(), Some("25"));
    assert_eq!(pair(&args, "-b:v").as_deref(), Some("0"));
}

/// H.264 and VP9 in 4:2:0 need even sides: the frame is scaled to the even size below.
#[test]
fn the_size_is_made_even() {
    let args = text(&ffmpeg_args(&spec(VideoFormat::Mp4), Path::new("o.mp4")));
    let filter = pair(&args, "-vf").expect("a filter");
    assert!(filter.starts_with("scale=1170:2532"), "{filter}");
    assert!(filter.contains("format=yuv420p"), "{filter}");
    assert_eq!(even(1), 2);
    assert_eq!(even(0), 2);
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let args = text(&ffmpeg_args(&spec(VideoFormat::Mp4), Path::new("o.mp4")));
    assert!(args.iter().any(|arg| arg == "-n"), "{args:?}");
    assert!(!args.iter().any(|arg| arg == "-y"), "{args:?}");
}

#[test]
fn the_crf_spans_the_quality_range() {
    assert_eq!(crf(100), 15);
    assert_eq!(crf(1), 63);
    assert_eq!(crf(0), 63);
}

#[test]
fn the_extension_follows_the_format() {
    assert_eq!(extension(VideoFormat::Mp4), "mp4");
    assert_eq!(extension(VideoFormat::Webm), "webm");
}

const LGPL_VERSION: &str = "ffmpeg version n9.0.2-49-gc179411bbb-20261111 Copyright (c) 2000-2026 the FFmpeg developers
built with gcc 15.2.0 (crosstool-NG 1.28.0.1_403899e)
configuration: --prefix=/ffbuild/prefix --pkg-config-flags=--static --enable-version3 --disable-debug --enable-libvpx --disable-libx264 --disable-libx265 --extra-version=20261111
libavutil      60. 26.100 / 60. 26.100
";

/// Acceptance: the build's configuration is read, and a GPL or non-free build is named.
#[test]
fn the_build_configuration_tells_lgpl_from_gpl() {
    let lgpl = build_info(LGPL_VERSION);
    assert_eq!(lgpl.version, "n9.0.2-49-gc179411bbb-20261111");
    assert!(lgpl.is_lgpl());
    let gpl =
        build_info(&LGPL_VERSION.replace("--enable-version3", "--enable-gpl --enable-libx264"));
    assert!(gpl.gpl && !gpl.is_lgpl());
    let nonfree = build_info(&LGPL_VERSION.replace("--disable-debug", "--enable-nonfree"));
    assert!(nonfree.nonfree && !nonfree.is_lgpl());
    let unknown = build_info("not ffmpeg");
    assert!(!unknown.is_lgpl(), "no configuration line is no proof");
}

/// A program that reads its stdin to the end stands in for ffmpeg: every frame reaches it, and
/// `finish` waits for it.
#[cfg(unix)]
#[tokio::test]
async fn a_sink_writes_the_frames_to_the_program_and_waits_for_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("frames.bin");
    let script = format!("cat > '{}'", out.display());
    let mut sink = FfmpegSink::spawn_program(Path::new("/bin/sh"), &["-c".into(), script.into()])
        .expect("spawned");
    sink.write(b"one").await.expect("write");
    sink.write(b"two").await.expect("write");
    Box::new(sink).finish().await.expect("finished");
    assert_eq!(std::fs::read(&out).expect("output"), b"onetwo");
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_program_is_reported_with_its_error_output() {
    let mut sink = FfmpegSink::spawn_program(
        Path::new("/bin/sh"),
        &[
            "-c".into(),
            "cat > /dev/null; echo broken >&2; exit 3".into(),
        ],
    )
    .expect("spawned");
    sink.write(b"frame").await.expect("write");
    let error = Box::new(sink).finish().await.expect_err("failed");
    assert!(error.contains("broken"), "{error}");
}

/// Charge 9 review M1: a program that takes no frames (a hung ffmpeg, its pipe full) times the
/// write out and is stopped, so `finish` returns at once instead of the recording hanging.
#[cfg(unix)]
#[tokio::test]
async fn a_program_that_takes_no_frames_times_out_and_is_stopped() {
    let sink =
        FfmpegSink::spawn_program(Path::new("/bin/sh"), &["-c".into(), "exec sleep 30".into()])
            .expect("spawned");
    let mut sink = sink.with_write_timeout(Duration::from_millis(300));
    let frame = vec![0_u8; 4 * 1024 * 1024];
    let error = sink.write(&frame).await.expect_err("timed out");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    // The give-up deadline: a `finish` that waited for the program's 30 s would not return.
    let finished = tokio::time::timeout(Duration::from_secs(10), Box::new(sink).finish())
        .await
        .expect("finish returns");
    assert!(finished.is_err());
}
