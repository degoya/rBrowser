use std::collections::BTreeMap;

use super::*;

fn notice(key: &str, level: NotificationLevel) -> Notification {
    Notification {
        level,
        message_key: key.to_owned(),
        args: BTreeMap::new(),
        settings_key: None,
    }
}

#[test]
fn messages_are_numbered_from_one_and_kept_in_order() {
    let log = StatusLog::new(10);
    let first = log.push(notice("a.one", NotificationLevel::Info), 100);
    let second = log.push(notice("a.two", NotificationLevel::Error), 200);
    let third = log.push(notice("a.three", NotificationLevel::Warning), 300);
    assert_eq!((first.id, second.id, third.id), (1, 2, 3));
    assert_eq!(third.at, 300);
    let keys: Vec<String> = log.items().into_iter().map(|m| m.message_key).collect();
    assert_eq!(keys, ["a.one", "a.two", "a.three"]);
}

#[test]
fn the_oldest_messages_go_once_the_log_is_full() {
    let log = StatusLog::new(2);
    for key in ["a.one", "a.two", "a.three"] {
        log.push(notice(key, NotificationLevel::Info), 0);
    }
    let ids: Vec<u64> = log.items().into_iter().map(|m| m.id).collect();
    assert_eq!(ids, [2, 3]);
    log.set_capacity(1);
    let ids: Vec<u64> = log.items().into_iter().map(|m| m.id).collect();
    assert_eq!(ids, [3]);
    // A new message keeps counting: ids are never reused.
    assert_eq!(log.push(notice("a.four", NotificationLevel::Info), 0).id, 4);
}

#[test]
fn a_log_keeps_at_least_one_message() {
    let log = StatusLog::new(0);
    log.push(notice("a.one", NotificationLevel::Info), 0);
    assert_eq!(log.items().len(), 1);
}

#[test]
fn recent_answers_the_newest_first_filtered_by_level() {
    let log = StatusLog::new(10);
    log.push(notice("a.one", NotificationLevel::Error), 0);
    log.push(notice("a.two", NotificationLevel::Info), 0);
    log.push(notice("a.three", NotificationLevel::Error), 0);
    let keys = |items: Vec<StatusMessage>| -> Vec<String> {
        items.into_iter().map(|m| m.message_key).collect()
    };
    assert_eq!(keys(log.recent(2, None)), ["a.three", "a.two"]);
    assert_eq!(
        keys(log.recent(10, Some(NotificationLevel::Error))),
        ["a.three", "a.one"]
    );
    assert!(log.recent(0, None).is_empty());
}

#[test]
fn a_report_of_the_shell_is_checked() {
    let good = notice("shell.error.request_failed", NotificationLevel::Error);
    assert_eq!(checked_report(&good), Ok(good.clone()));
    for key in ["", "has space", "x<script>", &"k".repeat(MAX_KEY_CHARS + 1)] {
        assert!(
            checked_report(&notice(key, NotificationLevel::Info)).is_err(),
            "{key:?}"
        );
    }
    let mut bad_setting = good.clone();
    bad_setting.settings_key = Some("ui theme".to_owned());
    assert!(checked_report(&bad_setting).is_err());
    let mut many = good.clone();
    many.args = (0..=MAX_ARGS)
        .map(|n| (format!("a{n}"), "x".to_owned()))
        .collect();
    assert!(checked_report(&many).is_err());
    let mut bad_name = good.clone();
    bad_name.args = [("na me".to_owned(), "x".to_owned())].into();
    assert!(checked_report(&bad_name).is_err());
}

#[test]
fn a_long_value_of_a_report_is_cut() {
    let mut long = notice("shell.error.request_failed", NotificationLevel::Error);
    long.args = [("message".to_owned(), "\u{e9}".repeat(MAX_ARG_CHARS + 50))].into();
    let checked = checked_report(&long).expect("a long value is cut, not refused");
    let value = &checked.args["message"];
    assert_eq!(value.chars().count(), MAX_ARG_CHARS + 3);
    assert!(value.ends_with("..."));
}

#[test]
fn the_log_takes_its_settings_and_notes_a_change_of_the_bar() {
    let settings = rb_config::StatusBarSettings {
        visible: false,
        history_size: 10,
        ..rb_config::StatusBarSettings::default()
    };
    let log = StatusLog::for_settings(&settings);
    for _ in 0..12 {
        log.push(notice("a.one", NotificationLevel::Info), 0);
    }
    assert_eq!(log.items().len(), 10);
    assert!(!log.set_shown(false), "no change");
    assert!(log.set_shown(true));
    assert!(!log.set_shown(true));
}

#[test]
fn now_is_after_the_year_2020() {
    assert!(now_ms() > 1_577_836_800_000);
}

/// Charge 10 review S2: a message of the core is cut like a report, so no long value fills
/// the log.
#[test]
fn a_long_value_of_a_core_message_is_cut() {
    let log = StatusLog::new(10);
    let mut long = notice("diagnostics.created", NotificationLevel::Info);
    long.args
        .insert("path".to_owned(), "x".repeat(MAX_ARG_CHARS + 50));
    let message = log.push(long, 1);
    let kept = &message.args["path"];
    assert_eq!(kept.chars().count(), MAX_ARG_CHARS + 3);
    assert!(kept.ends_with("..."));
}
