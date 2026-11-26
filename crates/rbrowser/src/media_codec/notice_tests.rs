use rb_plugin_types::Severity;
use rb_sync::{MediaMessage, MediaPlays, MediaTag};

use super::*;
use crate::media_codec::classify::{Evidence, Format, Missing};

fn refused_source(src: &str) -> MediaMessage {
    MediaMessage {
        v: 1,
        top: true,
        frame: None,
        url: "http://localhost:5173/".to_owned(),
        tag: MediaTag::Source,
        code: None,
        message: None,
        src: src.to_owned(),
        mime_type: Some(r#"video/mp4; codecs="avc1.42E01E""#.to_owned()),
        can_play: Some(String::new()),
        plays: MediaPlays::default(),
        path: "main > video > source".to_owned(),
    }
}

fn h264() -> Missing {
    Missing {
        format: Format::H264,
        evidence: Evidence::Refused,
    }
}

#[test]
fn the_notice_names_the_format_and_the_file_and_links_the_media_formats_page() {
    let notice = notification(&h264(), "https://cdn.test/media/clip.mp4");
    assert_eq!(notice.level, NotificationLevel::Info);
    assert_eq!(notice.message_key, NOTICE_KEY);
    assert_eq!(notice.args.get("format").map(String::as_str), Some("H.264"));
    assert_eq!(
        notice.args.get("source").map(String::as_str),
        Some("clip.mp4")
    );
    let action = action();
    assert_eq!(action.command, crate::open_link::OPEN_LINK_COMMAND);
    assert_eq!(action.label_key, ACTION_KEY);
    assert_eq!(action.args["url"], WIKI_URL);
    assert!(WIKI_URL.starts_with(crate::open_link::HANDBOOK_URL));
}

#[test]
fn the_finding_is_info_with_the_source_and_the_element() {
    let media = refused_source("https://user:pw@cdn.test/media/clip.mp4");
    let finding = finding(
        3,
        "http://localhost:5173/shop?token=abc&page=2",
        &media,
        &h264(),
    );
    assert_eq!(finding.rule, RULE);
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.message, FINDING_KEY);
    assert_eq!(finding.view, Some(3));
    // The page as every finding names it, its secrets masked.
    assert_eq!(
        finding.url.as_deref(),
        Some("http://localhost:5173/shop?token=***&page=2")
    );
    assert_eq!(finding.selector.as_deref(), Some("main > video > source"));
    assert_eq!(finding.path.as_deref(), Some("main > video > source"));
    assert_eq!(
        finding.params,
        [
            ("format".to_owned(), "H.264".to_owned()),
            (
                "source".to_owned(),
                "https://cdn.test/media/clip.mp4".to_owned()
            ),
        ]
    );
    let data: serde_json::Value =
        serde_json::from_str(finding.data.as_deref().unwrap_or_default()).expect("JSON");
    assert_eq!(data["format"], "h264");
    assert_eq!(data["evidence"], "refused");
    assert_eq!(data["type"], r#"video/mp4; codecs="avc1.42E01E""#);
}

#[test]
fn a_source_is_named_without_credentials_query_or_fragment_and_cut() {
    assert_eq!(
        source_label("https://u:p@cdn.test/a/clip.mp4?sig=1#t=3"),
        "https://cdn.test/a/clip.mp4"
    );
    assert_eq!(source_label("data:video/mp4"), "data:video/mp4");
    assert_eq!(
        source_label(&format!("https://cdn.test/{}", "a".repeat(900)))
            .chars()
            .count(),
        MAX_SOURCE
    );
}

#[test]
fn a_file_is_named_by_its_last_segment_or_its_host() {
    assert_eq!(file_name("https://cdn.test/a/clip.mp4?x=1"), "clip.mp4");
    assert_eq!(file_name("https://cdn.test/"), "cdn.test");
    assert_eq!(file_name("data:video/mp4"), "data:video/mp4");
    assert_eq!(file_name("/media/song.m4a"), "song.m4a");
}

#[test]
fn the_finding_texts_are_the_core_s() {
    let own = texts(PLUGIN);
    assert!(own.contains_key(FINDING_KEY));
    assert_eq!(own.len(), 1);
    assert!(!texts("seo").contains_key(FINDING_KEY));
}

#[test]
fn publishing_the_media_findings_writes_no_run_and_prunes_nothing() {
    let root = std::path::PathBuf::from("/project");
    assert_eq!(KEEP, crate::checks::reported::Keep::ShowOnly);
    assert_eq!(crate::checks::reported::written_to(Some(root), KEEP), None);
}
