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

fn view(width: u32, height: u32, dpr: f64, scale: f64) -> ViewInfo {
    ViewInfo {
        id: ViewId(1),
        label: "md".to_owned(),
        breakpoint: Some("md".to_owned()),
        cell_breakpoint: Some("md".to_owned()),
        engine: EngineKind::Chromium,
        width,
        height,
        dpr,
        scale,
        url: String::new(),
        title: String::new(),
        loading: false,
        session: "default".to_owned(),
        cell: None,
    }
}

/// RB-090-52, acceptance 2026-11-17: a view is recorded at the viewport it shows - the height its
/// cell gives it, not the breakpoint's - so the frames are never stretched; on a zoomed-out
/// canvas its page fills the top-left share of the frame its scale (the owner's `md`: 768 CSS px
/// wide, 814 high in its cell, at 15 %).
#[test]
fn a_view_is_planned_at_the_viewport_it_shows_and_its_page_share() {
    let (width, height, share) = plan_frame(&view(768, 814, 1.0, 0.15));
    assert_eq!((width, height), (768, 814));
    assert!((share - 0.15).abs() < 1e-12);
    let (width, height, share) = plan_frame(&view(390, 844, 3.0, 1.6));
    assert_eq!((width, height), (1170, 2532));
    assert!(
        (share - 1.0).abs() < 1e-12,
        "past 100 % the page fills the frame"
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
