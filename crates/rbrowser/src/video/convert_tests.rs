use std::path::Path;

use super::*;

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

/// The H.264 encoder of each platform's LGPL ffmpeg; none on Linux, so the export is not
/// offered there (RB-090-52).
#[test]
fn mp4_export_has_an_encoder_on_windows_and_macos_only() {
    assert_eq!(mp4_encoder_for("windows"), Some("h264_mf"));
    assert_eq!(mp4_encoder_for("macos"), Some("h264_videotoolbox"));
    assert_eq!(mp4_encoder_for("linux"), None);
    assert_eq!(mp4_encoder(), mp4_encoder_for(std::env::consts::OS));
}

#[test]
fn the_target_is_the_same_breakpoint_in_the_other_format() {
    assert_eq!(
        target_name("mobile.webm", VideoTarget::Mp4).as_deref(),
        Ok("mobile.mp4")
    );
    assert_eq!(
        target_name("500x800@2.mp4", VideoTarget::Webm).as_deref(),
        Ok("500x800@2.webm")
    );
    assert_eq!(
        target_name("Tablet.MP4", VideoTarget::Webm).as_deref(),
        Ok("Tablet.webm")
    );
    for (source, to) in [
        ("mobile.mp4", VideoTarget::Mp4),
        ("mobile.webm", VideoTarget::Webm),
        ("mobile.jpg", VideoTarget::Mp4),
        ("../mobile.webm", VideoTarget::Mp4),
        ("con.webm", VideoTarget::Mp4),
        ("", VideoTarget::Webm),
    ] {
        assert!(
            matches!(
                target_name(source, to),
                Err(ConvertError::Library(LibraryError::Refused(_)))
            ),
            "{source:?} to {to:?}"
        );
    }
    assert_eq!(part_name("mobile.mp4"), "mobile.mp4.part");
    assert!(
        !crate::video::library::is_video_name(&part_name("mobile.mp4")),
        "a half-written file is never listed or served"
    );
}

/// RB-090-52: the export never uses libx264 (GPL), writes into the `.part` file with its format
/// named, never over a file already there, and reports its progress on stdout.
#[test]
fn the_mp4_export_uses_the_platform_encoder() {
    let args = text(&convert_args(
        Encoder::Mp4("h264_mf"),
        80,
        4,
        Path::new("in.webm"),
        Path::new("mobile.mp4.part"),
    ));
    assert_eq!(pair(&args, "-i").as_deref(), Some("in.webm"));
    assert_eq!(pair(&args, "-c:v").as_deref(), Some("h264_mf"));
    assert_eq!(pair(&args, "-quality").as_deref(), Some("80"));
    assert_eq!(pair(&args, "-f").as_deref(), Some("mp4"));
    assert_eq!(pair(&args, "-movflags").as_deref(), Some("+faststart"));
    assert_eq!(pair(&args, "-progress").as_deref(), Some("pipe:1"));
    assert_eq!(pair(&args, "-loglevel").as_deref(), Some("info"));
    assert!(args.iter().any(|arg| arg == "-n"), "{args:?}");
    assert!(args.iter().any(|arg| arg == "-an"), "{args:?}");
    assert!(!args.iter().any(|arg| arg.contains("x264")), "{args:?}");
    assert_eq!(args.last().map(String::as_str), Some("mobile.mp4.part"));

    let mac = text(&convert_args(
        Encoder::Mp4("h264_videotoolbox"),
        80,
        4,
        Path::new("in.webm"),
        Path::new("o.part"),
    ));
    assert_eq!(pair(&mac, "-c:v").as_deref(), Some("h264_videotoolbox"));
    assert_eq!(pair(&mac, "-b:v").as_deref(), Some(MP4_BITRATE));
    assert_eq!(pair(&mac, "-allow_sw").as_deref(), Some("1"));
}

/// RB-090-52: an old MP4 becomes WebM with the recorder's codec and its share of threads.
#[test]
fn the_webm_conversion_uses_the_recorders_codec() {
    let args = text(&convert_args(
        Encoder::Webm(WebmCodec::Vp9),
        80,
        6,
        Path::new("in.mp4"),
        Path::new("mobile.webm.part"),
    ));
    assert_eq!(pair(&args, "-c:v").as_deref(), Some("libvpx-vp9"));
    assert_eq!(pair(&args, "-threads").as_deref(), Some("6"));
    assert_eq!(pair(&args, "-f").as_deref(), Some("webm"));
    assert!(!args.iter().any(|arg| arg == "-movflags"), "{args:?}");
    let vp8 = text(&convert_args(
        Encoder::Webm(WebmCodec::Vp8),
        80,
        1,
        Path::new("in.mp4"),
        Path::new("o.part"),
    ));
    assert_eq!(pair(&vp8, "-c:v").as_deref(), Some("libvpx"));
}

#[test]
fn the_length_and_the_progress_are_read_from_ffmpeg() {
    assert_eq!(
        parse_duration("  Duration: 00:01:02.50, start: 0.000000, bitrate: 512 kb/s"),
        Some(62.5)
    );
    assert_eq!(
        parse_duration("  Duration: 01:00:00.00, start: 0"),
        Some(3600.0)
    );
    assert_eq!(parse_duration("  Duration: N/A, start: 0.000000"), None);
    assert_eq!(parse_duration("frame=  10 fps=0.0"), None);

    assert_eq!(
        parse_progress("out_time_us=1500000"),
        Some(Progress::At(1_500_000))
    );
    assert_eq!(
        parse_progress("out_time_ms=2500000"),
        Some(Progress::At(2_500_000))
    );
    assert_eq!(parse_progress("out_time_us=N/A"), None);
    assert_eq!(parse_progress("progress=end"), Some(Progress::End));
    assert_eq!(parse_progress("progress=continue"), None);
    assert_eq!(parse_progress("fps=30.0"), None);

    assert_eq!(percent(1_000_000, Some(4.0)), Some(25));
    assert_eq!(
        percent(9_000_000, Some(4.0)),
        Some(99),
        "100 only at the end"
    );
    assert_eq!(percent(0, Some(4.0)), Some(0));
    assert_eq!(percent(1_000_000, None), None, "no length, no share");
    assert_eq!(percent(1_000_000, Some(0.0)), None);
}

#[test]
fn conversion_errors_have_their_codes() {
    assert_eq!(ConvertError::Unsupported.code(), "video.unsupported");
    assert_eq!(
        ConvertError::Exists("mobile.mp4".to_owned()).code(),
        "video.exists"
    );
    assert_eq!(ConvertError::Queued.code(), "video.busy");
    assert_eq!(
        ConvertError::Library(LibraryError::NotFound).code(),
        "video.not_found"
    );
    let unsupported = ConvertError::Unsupported.to_string();
    assert!(
        unsupported.contains("Windows") && unsupported.contains("macOS"),
        "{unsupported}"
    );
}
