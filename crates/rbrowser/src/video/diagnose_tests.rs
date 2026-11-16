use std::collections::BTreeMap;

use rb_ipc::{Notification, NotificationLevel};

use super::*;

fn report(key: &str, args: &[(&str, &str)]) -> Notification {
    Notification {
        level: NotificationLevel::Warning,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        settings_key: None,
    }
}

#[test]
fn a_video_that_does_not_play_is_logged_with_the_players_error() {
    let logged = play_report(&report(
        "videos.error.play",
        &[
            ("name", "ipad-mini.webm"),
            ("code", "4"),
            (
                "detail",
                "DEMUXER_ERROR_COULD_NOT_OPEN: FFmpegDemuxer: open context failed",
            ),
        ],
    ))
    .expect("logged");
    assert_eq!(
        logged,
        PlayReport {
            key: "videos.error.play".to_owned(),
            name: "ipad-mini.webm".to_owned(),
            code: "4".to_owned(),
            detail: "DEMUXER_ERROR_COULD_NOT_OPEN: FFmpegDemuxer: open context failed".to_owned(),
        }
    );
    let fallback = play_report(&report(
        "videos.fallback",
        &[("name", "a.webm"), ("code", "2")],
    ))
    .expect("logged");
    assert_eq!(fallback.detail, "-", "a value missing is a dash");
}

#[test]
fn other_reports_are_not_logged_here() {
    for key in ["videos.copied", "status.other", "videos.error.copy", ""] {
        assert_eq!(play_report(&report(key, &[("name", "x")])), None, "{key}");
    }
}

#[test]
fn a_value_is_cut_and_keeps_no_control_characters() {
    let long = "x".repeat(LOGGED_CHARS + 50);
    let logged = play_report(&report(
        "videos.error.play",
        &[("name", "a\nb.webm"), ("detail", long.as_str())],
    ))
    .expect("logged");
    assert_eq!(logged.name, "a?b.webm");
    assert_eq!(logged.detail.chars().count(), LOGGED_CHARS);
}
