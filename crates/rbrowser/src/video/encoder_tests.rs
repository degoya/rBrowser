use std::path::Path;

use super::*;

fn spec(codec: WebmCodec) -> EncodeSpec {
    EncodeSpec {
        codec,
        fps: 30,
        quality: 80,
        width: 1171,
        height: 2533,
        threads: 3,
        page_share: 1.0,
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

/// RB-090-52: every recording is WebM with VP9 in real time, never H.264 or libx264.
#[test]
fn a_recording_encodes_webm_with_vp9_and_a_crf_from_the_quality() {
    let args = text(&ffmpeg_args(&spec(WebmCodec::Vp9), Path::new("out.webm")));
    assert_eq!(pair(&args, "-c:v").as_deref(), Some("mjpeg"), "the input");
    assert!(args.iter().any(|arg| arg == "libvpx-vp9"), "{args:?}");
    assert!(
        !args
            .iter()
            .any(|arg| arg.contains("264") || arg.contains("x265")),
        "{args:?}"
    );
    assert_eq!(pair(&args, "-f").as_deref(), Some("image2pipe"));
    assert_eq!(pair(&args, "-i").as_deref(), Some("pipe:0"));
    assert_eq!(pair(&args, "-framerate").as_deref(), Some("30"));
    assert_eq!(pair(&args, "-deadline").as_deref(), Some("realtime"));
    assert_eq!(pair(&args, "-row-mt").as_deref(), Some("1"));
    assert_eq!(pair(&args, "-crf").as_deref(), Some("25"));
    assert_eq!(pair(&args, "-b:v").as_deref(), Some("0"));
    assert_eq!(args.last().map(String::as_str), Some("out.webm"));
    assert_eq!(EXTENSION, "webm");
}

/// RB-090-52: an ffmpeg without VP9 records VP8 - with a bitrate ceiling, which VP8's constant
/// quality needs.
#[test]
fn the_vp8_fallback_encodes_with_libvpx() {
    let args = text(&ffmpeg_args(&spec(WebmCodec::Vp8), Path::new("out.webm")));
    let codecs: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(at, _)| *at > 0 && args[at - 1] == "-c:v")
        .map(|(_, codec)| codec)
        .collect();
    assert_eq!(codecs, ["mjpeg", "libvpx"], "{args:?}");
    assert_eq!(pair(&args, "-b:v").as_deref(), Some(VP8_MAX_BITRATE));
    assert_eq!(pair(&args, "-crf").as_deref(), Some("25"));
    assert!(!args.iter().any(|arg| arg == "-row-mt"), "{args:?}");
}

/// RB-090-52: each encoder is capped to its share of threads.
#[test]
fn the_encoder_runs_with_its_threads() {
    let args = text(&ffmpeg_args(&spec(WebmCodec::Vp9), Path::new("o.webm")));
    assert_eq!(pair(&args, "-threads").as_deref(), Some("3"));
    let mut none = spec(WebmCodec::Vp8);
    none.threads = 0;
    let args = text(&ffmpeg_args(&none, Path::new("o.webm")));
    assert_eq!(
        pair(&args, "-threads").as_deref(),
        Some("1"),
        "never 0 (auto)"
    );
}

/// RB-090-52: all encoders together take at most half the logical cores, at least one thread
/// each.
#[test]
fn the_encoders_share_half_the_cores() {
    assert_eq!(encoder_threads(16, 1), 8);
    assert_eq!(encoder_threads(16, 2), 4);
    assert_eq!(encoder_threads(16, 6), 1);
    assert_eq!(encoder_threads(8, 3), 1);
    assert_eq!(encoder_threads(32, 8), 2);
    assert_eq!(encoder_threads(2, 8), 1, "at least one");
    assert_eq!(encoder_threads(1, 1), 1);
    assert_eq!(encoder_threads(12, 0), 6, "no encoders counts as one");
    for logical in 2..=64 {
        for encoders in 1..=8 {
            let each = encoder_threads(logical, encoders) as usize;
            assert!(each * encoders <= (logical / 2).max(encoders));
        }
    }
    assert!(logical_cores() >= 1);
}

/// VP9 and VP8 in 4:2:0 need even sides: the frame is scaled to the even size below.
#[test]
fn the_size_is_made_even() {
    let args = text(&ffmpeg_args(&spec(WebmCodec::Vp9), Path::new("o.webm")));
    let filter = pair(&args, "-vf").expect("a filter");
    assert!(filter.starts_with("scale=1170:2532"), "{filter}");
    assert!(filter.contains("format=yuv420p"), "{filter}");
    assert_eq!(even(1), 2);
    assert_eq!(even(0), 2);
}

/// RB-090-52, acceptance 2026-11-17 (`PIPELINE_ERROR_DECODE` on Windows): the screencast's
/// JPEG frames are full range with BT.601 colour; the file is written as the standard stream
/// every decoder takes - limited range, BT.709, square pixels, 4:2:0 - and says so in its tags.
#[test]
fn a_recording_is_written_as_a_standard_stream() {
    for codec in [WebmCodec::Vp9, WebmCodec::Vp8] {
        let args = text(&ffmpeg_args(&spec(codec), Path::new("o.webm")));
        assert_eq!(
            pair(&args, "-vf").as_deref(),
            Some(
                "scale=1170:2532:flags=bicubic:in_range=pc:in_color_matrix=bt601:\
                 out_range=tv:out_color_matrix=bt709,setsar=1,format=yuv420p,\
                 setparams=range=tv:colorspace=bt709:color_primaries=bt709:color_trc=bt709"
            ),
            "{args:?}"
        );
        assert_eq!(pair(&args, "-color_range").as_deref(), Some("tv"));
        assert_eq!(pair(&args, "-colorspace").as_deref(), Some("bt709"));
        assert_eq!(pair(&args, "-color_primaries").as_deref(), Some("bt709"));
        assert_eq!(pair(&args, "-color_trc").as_deref(), Some("bt709"));
        let tags = args
            .iter()
            .position(|arg| arg == "-color_range")
            .expect("tags");
        let codec_at = args
            .iter()
            .rposition(|arg| arg == "-c:v")
            .expect("the codec");
        assert!(tags > codec_at, "output options, after the input: {args:?}");
        assert_eq!(args.last().map(String::as_str), Some("o.webm"));
    }
}

/// RB-090-52, acceptance 2026-11-17 (the page small in the top left on white): below a scale of
/// 1 the page fills only the top-left share of the screencast frame; that share is cut out and
/// scaled to the planned size, so the file shows the page and nothing else.
#[test]
fn the_page_is_cut_out_of_the_frame_below_a_scale_of_one() {
    let mut zoomed_out = spec(WebmCodec::Vp9);
    zoomed_out.page_share = 0.15;
    let args = text(&ffmpeg_args(&zoomed_out, Path::new("o.webm")));
    let filter = pair(&args, "-vf").expect("a filter");
    assert!(
        filter.starts_with("crop=w=iw*0.150000:h=ih*0.150000:x=0:y=0,scale=1170:2532:"),
        "{filter}"
    );
    assert!(filter.ends_with(STANDARD_TAIL), "{filter}");
    for whole in [1.0, 1.7, f64::NAN, f64::INFINITY, 0.0, -1.0] {
        let mut full = spec(WebmCodec::Vp9);
        full.page_share = whole;
        let filter =
            pair(&text(&ffmpeg_args(&full, Path::new("o.webm"))), "-vf").expect("a filter");
        assert!(filter.starts_with("scale=1170:2532:"), "{whole}: {filter}");
    }
    let mut tiny = spec(WebmCodec::Vp9);
    tiny.page_share = 0.0001;
    let filter = pair(&text(&ffmpeg_args(&tiny, Path::new("o.webm"))), "-vf").expect("a filter");
    assert!(
        filter.starts_with("crop=w=iw*0.010000:h=ih*0.010000:"),
        "never below the smallest scale: {filter}"
    );
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let args = text(&ffmpeg_args(&spec(WebmCodec::Vp9), Path::new("o.webm")));
    assert!(args.iter().any(|arg| arg == "-n"), "{args:?}");
    assert!(!args.iter().any(|arg| arg == "-y"), "{args:?}");
}

#[test]
fn the_crf_spans_the_quality_range() {
    assert_eq!(crf(100), 15);
    assert_eq!(crf(1), 63);
    assert_eq!(crf(0), 63);
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
