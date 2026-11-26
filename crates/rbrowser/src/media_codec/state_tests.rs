use rb_plugin_types::Severity;
use rb_sync::{MediaMessage, MediaPlays, MediaTag};

use super::*;

const NO_STREAMS: &str = "DEMUXER_ERROR_NO_SUPPORTED_STREAMS: FFmpegDemuxer: no supported streams";

fn failed(src: &str) -> MediaMessage {
    MediaMessage {
        v: 1,
        top: true,
        frame: None,
        url: "http://localhost:5173/".to_owned(),
        tag: MediaTag::Video,
        code: Some(4),
        message: Some(NO_STREAMS.to_owned()),
        src: src.to_owned(),
        mime_type: None,
        can_play: None,
        plays: MediaPlays::default(),
        path: "main > video".to_owned(),
    }
}

const PAGE: &str = "http://localhost:5173/shop?session=abc#top";

#[test]
fn one_notice_per_page_and_format_whatever_the_views_and_rebuilds() {
    let mut state = State::default();
    let first = state.on_media(1, PAGE, &failed("http://localhost:5173/clip.mp4"));
    assert!(first.is_some());
    // The other views of the grid, a rebuilt view, the page loaded again with another query.
    for (view, page) in [
        (2, PAGE),
        (3, PAGE),
        (9, PAGE),
        (1, "http://localhost:5173/shop?session=xyz"),
    ] {
        assert_eq!(
            state.on_media(view, page, &failed("http://localhost:5173/clip.mp4")),
            None
        );
    }
    // Another file of the same format on that page: still the one notice.
    assert_eq!(
        state.on_media(1, PAGE, &failed("http://localhost:5173/other.mp4")),
        None
    );
    // Another format on that page, the same format on another page: a notice each.
    assert!(
        state
            .on_media(1, PAGE, &failed("http://localhost:5173/song.m4a"))
            .is_some()
    );
    assert!(
        state
            .on_media(1, "http://localhost:5173/about", &failed("/clip.mp4"))
            .is_some()
    );
}

#[test]
fn webm_brings_no_notice_and_no_finding() {
    let mut state = State::default();
    assert_eq!(state.on_media(1, PAGE, &failed("/clip.webm")), None);
    assert_eq!(state.findings_len(), 0);
}

#[test]
fn every_outcome_carries_all_findings_so_far_and_they_stay_bounded() {
    let mut state = State::default();
    let one = state
        .on_media(1, PAGE, &failed("/clip.mp4"))
        .expect("a notice");
    assert_eq!(one.findings.len(), 1);
    let two = state
        .on_media(2, PAGE, &failed("/song.m4a"))
        .expect("a notice");
    assert_eq!(two.findings.len(), 2);
    assert_eq!(two.findings[0], one.findings[0]);
    for index in 0..MAX_FINDINGS + 3 {
        let page = format!("http://localhost:5173/page-{index}");
        state.on_media(1, &page, &failed("/clip.mp4"));
    }
    assert_eq!(state.findings_len(), MAX_FINDINGS);
}

#[test]
fn the_pages_remembered_stay_bounded_and_the_oldest_goes_first() {
    let mut seen = Seen::default();
    assert!(seen.first("a", "mp4"));
    assert!(!seen.first("a", "mp4"));
    for index in 0..MAX_SEEN {
        assert!(seen.first(&format!("page-{index}"), "mp4"));
    }
    assert_eq!(seen.len(), MAX_SEEN);
    // `a` was the oldest and is forgotten.
    assert!(seen.first("a", "mp4"));
}

#[test]
fn a_page_is_its_address_without_credentials_query_and_fragment() {
    assert_eq!(page_key(PAGE), "http://localhost:5173/shop");
    assert_eq!(
        page_key("https://user:pw@example.test/a/b?x#y"),
        "https://example.test/a/b"
    );
}

#[test]
fn the_outcome_is_an_info_notice_and_an_info_finding() {
    let mut state = State::default();
    let outcome = state
        .on_media(4, PAGE, &failed("http://localhost:5173/clip.mp4"))
        .expect("a notice");
    assert_eq!(outcome.notification.level, rb_ipc::NotificationLevel::Info);
    let finding = &outcome.findings[0];
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.view, Some(4));
}
