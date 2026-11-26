use rb_sync::{MediaMessage, MediaPlays, MediaTag};

use super::*;

const NO_STREAMS: &str = "DEMUXER_ERROR_NO_SUPPORTED_STREAMS: FFmpegDemuxer: no supported streams";
/// What Chromium says to a media file it could not fetch (a 404).
const FORMAT_ERROR: &str = "MEDIA_ELEMENT_ERROR: Format error";

/// The standard CEF build: no H.264, AAC or HEVC.
const STANDARD: MediaPlays = MediaPlays {
    h264: false,
    aac: false,
    hevc: false,
};
/// A build with the proprietary codecs (RB-110-20).
const CODECS: MediaPlays = MediaPlays {
    h264: true,
    aac: true,
    hevc: true,
};

fn element(src: &str, code: u16, message: &str, plays: MediaPlays) -> MediaMessage {
    MediaMessage {
        v: 1,
        top: true,
        frame: None,
        url: "http://localhost:5173/".to_owned(),
        tag: MediaTag::Video,
        code: Some(code),
        message: Some(message.to_owned()),
        src: src.to_owned(),
        mime_type: None,
        can_play: None,
        plays,
        path: "main > video".to_owned(),
    }
}

fn source(
    src: &str,
    mime: Option<&str>,
    can_play: Option<&str>,
    plays: MediaPlays,
) -> MediaMessage {
    MediaMessage {
        tag: MediaTag::Source,
        code: None,
        message: None,
        mime_type: mime.map(str::to_owned),
        can_play: can_play.map(str::to_owned),
        path: "video > source".to_owned(),
        ..element(src, 0, "", plays)
    }
}

fn missing(format: Format, evidence: Evidence) -> Option<Missing> {
    Some(Missing { format, evidence })
}

#[test]
fn an_mp4_the_demuxer_finds_no_stream_in_is_an_mp4_without_its_codecs() {
    let media = element("http://localhost:5173/clip.mp4", 4, NO_STREAMS, STANDARD);
    assert_eq!(classify(&media), missing(Format::Mp4, Evidence::Decoder));
}

#[test]
fn a_source_the_build_refuses_names_the_codec_of_its_type() {
    let media = source(
        "http://localhost:5173/clip.mp4",
        Some(r#"video/mp4; codecs="avc1.42E01E, mp4a.40.2""#),
        Some(""),
        STANDARD,
    );
    assert_eq!(classify(&media), missing(Format::H264, Evidence::Refused));
    let hevc = source(
        "/clip",
        Some("video/mp4; codecs=hvc1.1.6.L93.B0"),
        Some(""),
        CODECS,
    );
    assert_eq!(classify(&hevc), missing(Format::Hevc, Evidence::Refused));
    let aac = source(
        "/a",
        Some("audio/mp4; codecs=mp4a.40.2"),
        Some(""),
        STANDARD,
    );
    assert_eq!(classify(&aac), missing(Format::Aac, Evidence::Refused));
    let dolby = source("/a", Some("audio/mp4; codecs=ec-3"), Some(""), CODECS);
    assert_eq!(classify(&dolby), missing(Format::Ac3, Evidence::Refused));
}

#[test]
fn the_container_or_the_file_name_names_the_format_without_codecs() {
    let mov = source("/clip", Some("video/quicktime"), Some(""), STANDARD);
    assert_eq!(classify(&mov), missing(Format::Mov, Evidence::Refused));
    let m4a = element("/media/song.M4A", 4, NO_STREAMS, STANDARD);
    assert_eq!(classify(&m4a), missing(Format::Aac, Evidence::Decoder));
    let unknown = source("/clip.flv", Some("video/x-flv"), Some(""), STANDARD);
    assert_eq!(
        classify(&unknown),
        missing(Format::Other("video/x-flv".to_owned()), Evidence::Refused)
    );
    let by_name = element("/clip.mkv", 4, NO_STREAMS, STANDARD);
    assert_eq!(
        classify(&by_name),
        missing(Format::Other("MKV".to_owned()), Evidence::Decoder)
    );
}

#[test]
fn an_mp4_source_that_failed_without_a_verdict_is_likely_when_the_build_lacks_h264() {
    let media = source(
        "/media/clip.mp4",
        Some("video/mp4"),
        Some("maybe"),
        STANDARD,
    );
    assert_eq!(classify(&media), missing(Format::Mp4, Evidence::Likely));
    let no_type = source("/media/clip.mp4", None, None, STANDARD);
    assert_eq!(classify(&no_type), missing(Format::Mp4, Evidence::Likely));
    // A file the build cannot fetch, with the build lacking H.264.
    let not_found = element("/media/clip.mp4", 4, FORMAT_ERROR, STANDARD);
    assert_eq!(classify(&not_found), missing(Format::Mp4, Evidence::Likely));
}

#[test]
fn a_build_with_the_codecs_is_told_nothing_without_a_verdict() {
    let media = source("/media/clip.mp4", Some("video/mp4"), Some("maybe"), CODECS);
    assert_eq!(classify(&media), None);
    let not_found = element("/media/clip.mp4", 4, FORMAT_ERROR, CODECS);
    assert_eq!(classify(&not_found), None);
}

#[test]
fn webm_vp9_av1_opus_and_the_other_open_formats_are_never_missing() {
    for src in [
        "/a.webm", "/a.ogg", "/a.ogv", "/a.opus", "/a.mp3", "/a.flac", "/a.wav",
    ] {
        assert_eq!(
            classify(&element(src, 4, NO_STREAMS, STANDARD)),
            None,
            "{src}"
        );
    }
    for mime in [
        "video/webm; codecs=\"vp9, opus\"",
        "video/webm",
        "audio/ogg; codecs=vorbis",
        r#"video/mp4; codecs="av01.0.05M.08""#,
        "video/mp4; codecs=vp09.00.10.08",
        "audio/mp4; codecs=opus",
        "audio/mpeg",
    ] {
        let media = source("/a.mp4", Some(mime), Some(""), STANDARD);
        assert_eq!(classify(&media), None, "{mime}");
    }
}

#[test]
fn an_unknown_file_without_a_verdict_and_an_unknown_message_say_nothing() {
    assert_eq!(classify(&source("/clip.mkv", None, None, STANDARD)), None);
    assert_eq!(
        classify(&element("/clip", 4, NO_STREAMS, STANDARD)),
        None,
        "neither a type nor a file name names a format"
    );
    assert_eq!(
        classify(&element("/clip.mkv", 4, FORMAT_ERROR, STANDARD)),
        None
    );
    // Only code 4 of a media element counts.
    assert_eq!(classify(&element("/clip.mp4", 3, NO_STREAMS, CODECS)), None);
}

#[test]
fn types_are_read_without_case_quotes_or_spaces() {
    assert_eq!(
        parse_type(r#" Video/MP4 ; Codecs = "AVC1.42E01E , mp4a.40.2" "#),
        (
            "video/mp4".to_owned(),
            vec!["avc1.42e01e".to_owned(), "mp4a.40.2".to_owned()]
        )
    );
    assert_eq!(
        parse_type("video/webm"),
        ("video/webm".to_owned(), Vec::new())
    );
}

#[test]
fn the_extension_is_the_path_s_not_the_host_s() {
    assert_eq!(extension("https://cdn.example.mp4/clip"), None);
    assert_eq!(
        extension("https://cdn.test/a/clip.MP4"),
        Some("mp4".to_owned())
    );
    assert_eq!(extension("data:video/mp4"), None);
    assert_eq!(extension("/a/.hidden/"), None);
}

#[test]
fn every_format_has_an_id_and_a_label() {
    let formats = [
        Format::H264,
        Format::Hevc,
        Format::Aac,
        Format::Ac3,
        Format::Mp4,
        Format::Mov,
        Format::Other("video/x-FLV".to_owned()),
    ];
    let ids: Vec<String> = formats.iter().map(Format::id).collect();
    assert_eq!(
        ids,
        [
            "h264",
            "hevc",
            "aac",
            "ac3",
            "mp4",
            "mov",
            "other:video/x-flv"
        ]
    );
    assert!(formats.iter().all(|format| !format.label().is_empty()));
    assert_eq!(Format::Mp4.label(), "MP4 (H.264, AAC)");
}
