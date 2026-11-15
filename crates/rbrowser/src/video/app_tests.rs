use super::*;

/// Acceptance: the resolution is the viewport times the DPR, bounded and even.
#[test]
fn the_frame_is_the_viewport_times_the_dpr_bounded() {
    assert_eq!(frame_size(390, 844, 3.0), (1170, 2532));
    assert_eq!(frame_size(1920, 1080, 2.0), (3840, 2160));
    assert_eq!(
        frame_size(2560, 1440, 2.0),
        (3840, 2160),
        "bounded, aspect kept"
    );
    assert_eq!(
        frame_size(375, 667, 0.0),
        (374, 666),
        "no DPR counts as 1, sides even"
    );
}

#[test]
fn a_start_request_takes_a_target_or_views_and_nothing_else() {
    let all: StartRequest = serde_json::from_str(r#"{ "target": "all" }"#).expect("all");
    assert_eq!(all.target, Some(Target::All));
    let views: StartRequest = serde_json::from_str(r#"{ "views": [1, 3] }"#).expect("views");
    assert_eq!(views.views, Some(vec![ViewId(1), ViewId(3)]));
    assert!(serde_json::from_str::<StartRequest>(r#"{ "url": "x" }"#).is_err());
}

#[test]
fn every_error_has_a_stable_code_and_the_missing_ffmpeg_points_to_the_ui() {
    assert_eq!(VideoError::FfmpegMissing.code(), "video.ffmpeg_missing");
    assert_eq!(VideoError::Busy.code(), "video.busy");
    assert_eq!(VideoError::NotRecording.code(), "video.not_recording");
    let text = VideoError::FfmpegMissing.to_string();
    assert!(
        text.contains("download ffmpeg") && text.contains("tools.ffmpegPath"),
        "{text}"
    );
}

#[test]
fn nothing_records_at_first() {
    assert!(!is_recording());
}
