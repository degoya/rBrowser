use super::*;
use crate::{AgentMessage, parse_message};

#[test]
fn parses_a_media_element_failure_and_writes_it_back_unchanged() {
    let text = r#"{"kind":"media","v":1,"top":true,"url":"http://localhost:5173/","tag":"video","code":4,"message":"DEMUXER_ERROR_NO_SUPPORTED_STREAMS: FFmpegDemuxer: no supported streams","src":"http://localhost:5173/media/clip.mp4","plays":{"h264":false,"aac":false,"hevc":false},"path":"main > video"}"#;
    let message = parse_message(text).expect("a valid message");
    let AgentMessage::Media(media) = &message else {
        panic!("not a media message: {message:?}");
    };
    assert_eq!(message.version(), 1);
    assert_eq!(media.tag, MediaTag::Video);
    assert_eq!(media.code, Some(4));
    assert_eq!(media.mime_type, None);
    assert_eq!(media.plays, MediaPlays::default());
    let back = serde_json::to_string(&message).expect("serializes");
    assert_eq!(back, text);
}

#[test]
fn parses_a_source_failure_with_its_type_and_the_build_s_answer() {
    let text = r#"{"kind":"media","v":1,"top":false,"frame":["iframe#player"],"url":"http://localhost:5173/player","tag":"source","src":"http://localhost:5173/media/clip.mp4","type":"video/mp4; codecs=\"avc1.42E01E\"","canPlay":"","plays":{"h264":false,"aac":true,"hevc":false},"path":"video > source"}"#;
    let AgentMessage::Media(media) = parse_message(text).expect("a valid message") else {
        panic!("not a media message");
    };
    assert_eq!(media.tag, MediaTag::Source);
    assert_eq!(
        media.mime_type.as_deref(),
        Some(r#"video/mp4; codecs="avc1.42E01E""#)
    );
    assert_eq!(media.can_play.as_deref(), Some(""));
    assert!(media.plays.aac);
    assert_eq!(media.frame, Some(vec!["iframe#player".to_owned()]));
}

#[test]
fn refuses_an_unknown_tag() {
    let text = r#"{"kind":"media","v":1,"top":true,"url":"u","tag":"img","src":"s","plays":{"h264":false,"aac":false,"hevc":false},"path":"p"}"#;
    assert!(parse_message(text).is_err());
}
