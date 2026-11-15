use crate::{ProjectConfig, UserSettings};

use super::*;

#[test]
fn the_project_reads_capture_video_and_writes_nothing_by_default() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "capture": { "video": { "format": "webm", "fps": 24 } } }"#,
    )
    .expect("parses");
    assert_eq!(config.capture.video.format, Some(VideoFormat::Webm));
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
        format: Some(VideoFormat::Webm),
        fps: Some(60),
        max_duration_sec: Some(10),
        quality: None,
    };
    let merged = project.over(&user);
    assert_eq!(merged.fps, Some(15));
    assert_eq!(merged.format, Some(VideoFormat::Webm));
    assert_eq!(merged.max_duration_sec, Some(10));
    assert_eq!(merged.quality, None);
}

#[test]
fn an_unknown_format_is_refused() {
    let bad = r#"{ "version": 1, "capture": { "video": { "format": "avi" } } }"#;
    assert!(serde_json::from_str::<ProjectConfig>(bad).is_err());
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
