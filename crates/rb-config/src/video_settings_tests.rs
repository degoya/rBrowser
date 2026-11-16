use crate::{ProjectConfig, UserSettings};

use super::*;

#[test]
fn the_project_reads_capture_video_and_writes_nothing_by_default() {
    let config: ProjectConfig =
        serde_json::from_str(r#"{ "version": 1, "capture": { "video": { "fps": 24 } } }"#)
            .expect("parses");
    assert_eq!(config.capture.video.fps, Some(24));
    assert_eq!(config.capture.video.quality, None);
    let bare: ProjectConfig = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    let written = serde_json::to_value(&bare).expect("serializes");
    assert!(written.get("capture").is_none(), "{written}");
}

#[test]
fn the_project_wins_per_key_over_the_user() {
    let project = VideoConfig {
        fps: Some(15),
        ..VideoConfig::default()
    };
    let user = VideoConfig {
        fps: Some(60),
        max_duration_sec: Some(10),
        keep_recordings: Some(5),
        keep_days: Some(30),
        autoplay: Some(true),
        ..VideoConfig::default()
    };
    let merged = project.over(&user);
    assert_eq!(merged.fps, Some(15));
    assert_eq!(merged.max_duration_sec, Some(10));
    assert_eq!(merged.quality, None);
    assert_eq!(merged.keep_recordings, Some(5));
    assert_eq!(merged.keep_days, Some(30));
    assert_eq!(merged.autoplay, Some(true));
}

/// The Videos panel's keys (RB-110-12): retention and autoplay, read from either file and never
/// written when not set.
#[test]
fn retention_and_autoplay_read_and_stay_out_when_unset() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "capture": { "video": { "keepRecordings": 3, "keepDays": 7, "autoplay": true } } }"#,
    )
    .expect("parses");
    assert_eq!(config.capture.video.keep_recordings, Some(3));
    assert_eq!(config.capture.video.keep_days, Some(7));
    assert_eq!(config.capture.video.autoplay, Some(true));
    let written = serde_json::to_value(VideoConfig::default()).expect("serializes");
    assert_eq!(written, serde_json::json!({}));
    let project = VideoConfig {
        keep_recordings: Some(0),
        ..VideoConfig::default()
    };
    let user = VideoConfig {
        keep_recordings: Some(9),
        autoplay: Some(false),
        ..VideoConfig::default()
    };
    let merged = project.over(&user);
    assert_eq!(
        merged.keep_recordings,
        Some(0),
        "0 in the project keeps all"
    );
    assert_eq!(merged.autoplay, Some(false));
}

/// RB-090-52: recordings are always WebM - the schema names no format any more.
#[test]
fn the_format_is_no_setting_any_more() {
    let schema = serde_json::Value::from(crate::project_schema());
    let video = schema
        .pointer("/properties/capture/properties/video/properties")
        .expect("capture.video");
    assert!(video.get("format").is_none(), "{video}");
    assert!(video.get("fps").is_some(), "{video}");
}

/// The program path lives in the user settings only: the project schema has no such key.
#[test]
fn the_ffmpeg_path_is_a_user_setting_only() {
    let user: UserSettings =
        serde_json::from_str(r#"{ "tools": { "ffmpegPath": "C:\\ffmpeg\\bin\\ffmpeg.exe" } }"#)
            .expect("parses");
    assert_eq!(
        user.tools.ffmpeg_path.as_deref(),
        Some("C:\\ffmpeg\\bin\\ffmpeg.exe")
    );
    let project = serde_json::to_value(schemars::schema_for!(ProjectConfig)).expect("schema");
    assert!(
        !project.to_string().contains("ffmpegPath"),
        "the project schema names no program path"
    );
}
