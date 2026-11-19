use std::collections::BTreeMap;

use rb_ipc::{Notification, NotificationLevel};

use super::*;

fn notice(error: &str) -> Notification {
    Notification {
        level: NotificationLevel::Error,
        message_key: "sitemap.notice.failed".to_owned(),
        args: BTreeMap::from([("error".to_owned(), error.to_owned())]),
        settings_key: Some("sitemap".to_owned()),
    }
}

#[test]
fn an_error_code_in_front_of_the_text_goes() {
    // The owner's status bar: "The sitemap capture did not start: sitemap.error.busy: a
    // sitemap capture is already running".
    let mut notification = notice("sitemap.error.busy: a sitemap capture is already running");
    strip_codes(&mut notification);
    assert_eq!(
        notification.args["error"],
        "a sitemap capture is already running"
    );
    let mut notification = notice("env_compare.error.unknown_environment: no environment staging");
    strip_codes(&mut notification);
    assert_eq!(notification.args["error"], "no environment staging");
}

#[test]
fn a_text_without_a_code_stays() {
    for text in [
        "example.com: connection refused",
        "C:\\Users\\dev: access denied",
        "reserved-shortcuts.json: line 3",
        "sitemap.error.busy",
        "sitemap.error.busy: ",
        "Sitemap.error.busy: the capital is no code",
        "sitemap..error.busy: empty segment",
        "a sitemap capture is already running",
    ] {
        let mut notification = notice(text);
        strip_codes(&mut notification);
        assert_eq!(notification.args["error"], text, "{text}");
    }
}

#[test]
fn every_argument_is_read_and_the_rest_of_the_notice_stays() {
    let mut notification = notice("checks.error.busy: a check run is already in progress");
    notification
        .args
        .insert("folder".to_owned(), "C:\\captures".to_owned());
    strip_codes(&mut notification);
    assert_eq!(
        notification.args["error"],
        "a check run is already in progress"
    );
    assert_eq!(notification.args["folder"], "C:\\captures");
    assert_eq!(notification.message_key, "sitemap.notice.failed");
    assert_eq!(notification.settings_key.as_deref(), Some("sitemap"));
}
