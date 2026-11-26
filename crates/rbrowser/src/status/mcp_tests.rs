use std::collections::BTreeMap;

use rb_ipc::{Notification, NotificationLevel};
use serde_json::json;

use super::*;

fn log() -> StatusLog {
    let log = StatusLog::new(10);
    for (key, level) in [
        ("diagnostics.created", NotificationLevel::Info),
        ("shell.error.no_answer", NotificationLevel::Error),
        ("diagnostics.created", NotificationLevel::Success),
    ] {
        log.push(
            Notification {
                level,
                message_key: key.to_owned(),
                args: BTreeMap::from([("path".to_owned(), "C:/x.zip".to_owned())]),
                settings_key: None,
            },
            42,
        );
    }
    log
}

#[test]
fn the_newest_messages_come_first_with_their_text() {
    let answer = answer(&log(), &json!({})).expect("answers");
    let messages = answer["messages"].as_array().expect("a list");
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["id"], 3);
    assert_eq!(messages[0]["level"], "success");
    assert_eq!(messages[0]["at"], 42);
    assert_eq!(messages[0]["messageKey"], "diagnostics.created");
    let text = messages[0]["text"].as_str().expect("a text");
    assert!(text.contains("C:/x.zip"), "{text}");
    assert!(messages[0].get("settingsKey").is_none());
    assert_eq!(answer["kept"], 3);
}

#[test]
fn limit_and_level_narrow_the_answer() {
    let answer = answer(&log(), &json!({ "limit": 1 })).expect("answers");
    assert_eq!(answer["messages"].as_array().map(Vec::len), Some(1));
    let answer = answer_level(&json!({ "level": "error" }));
    assert_eq!(answer["messages"][0]["messageKey"], "shell.error.no_answer");
    assert_eq!(answer["messages"].as_array().map(Vec::len), Some(1));
}

fn answer_level(args: &serde_json::Value) -> serde_json::Value {
    answer(&log(), args).expect("answers")
}

#[test]
fn bad_arguments_are_refused() {
    for args in [
        json!({ "limit": 0 }),
        json!({ "limit": MAX_LIMIT + 1 }),
        json!({ "level": "fatal" }),
        json!({ "other": true }),
    ] {
        let failure = answer(&log(), &args).expect_err("refused");
        assert_eq!(failure.code, "args.invalid", "{args}");
    }
}

/// Charge 10 review S1: a registered secret (the CDP proxy's token) is masked in the text,
/// which only patterns would not find in an address path.
#[test]
fn a_registered_secret_in_a_message_is_masked() {
    // Built from parts, so the secret scanner does not take the fixture for a key.
    let token = ["c10s1", "0123456789abcdef", "0123456789"].concat();
    let token = token.as_str();
    rb_diag::redact::register_secret(token);
    let log = StatusLog::new(10);
    log.push(
        Notification {
            level: NotificationLevel::Info,
            message_key: "diagnostics.created".to_owned(),
            args: BTreeMap::from([(
                "path".to_owned(),
                format!("http://127.0.0.1:9333/t/{token}"),
            )]),
            settings_key: None,
        },
        1,
    );
    let answer = answer(&log, &json!({})).expect("answers");
    rb_diag::redact::unregister_secret(token);
    let text = answer["messages"][0]["text"].as_str().expect("a text");
    assert!(!text.contains(token), "{text}");
}
