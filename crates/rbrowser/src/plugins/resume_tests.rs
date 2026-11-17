use rb_commands::CommandError;
use rb_ipc::NotificationLevel;
use rb_plugin_host::PluginHealths;
use serde_json::json;

use super::{NONE_PAUSED_KEY, RESUMED_KEY, notice, resume};

/// A book with `rulers-guides` and `todo` paused and `lighthouse` running.
fn book() -> PluginHealths {
    let book = PluginHealths::new(1);
    for id in ["rulers-guides", "todo"] {
        assert!(book.of(id).record_failure());
    }
    assert!(book.of("lighthouse").check().is_ok());
    book
}

#[test]
fn an_id_resumes_that_plugin_alone() {
    let book = book();
    let answer = resume(&book, &json!({ "id": "rulers-guides" })).expect("resumes");
    assert_eq!(answer, json!({ "resumed": ["rulers-guides"] }));
    assert!(!book.of("rulers-guides").is_suspended());
    assert!(book.of("todo").is_suspended(), "only the one plugin");
}

#[test]
fn an_id_that_is_not_paused_resumes_nothing() {
    let book = book();
    for id in ["lighthouse", "unknown"] {
        let answer = resume(&book, &json!({ "id": id })).expect("answers");
        assert_eq!(answer, json!({ "resumed": [] }));
    }
    assert_eq!(book.suspended(), ["rulers-guides", "todo"]);
}

#[test]
fn without_an_id_every_paused_plugin_resumes() {
    for args in [json!(null), json!({})] {
        let book = book();
        let answer = resume(&book, &args).expect("resumes");
        assert_eq!(answer, json!({ "resumed": ["rulers-guides", "todo"] }));
        assert!(book.suspended().is_empty());
    }
}

#[test]
fn an_id_that_is_no_text_is_refused() {
    let book = book();
    let refused = resume(&book, &json!({ "id": 7 })).expect_err("no id");
    assert!(
        matches!(refused, CommandError::InvalidArgs(_)),
        "{refused:?}"
    );
    assert_eq!(book.suspended().len(), 2, "nothing resumed");
}

#[test]
fn the_notice_names_the_resumed_plugins_or_says_none_was_paused() {
    let resumed = notice(&["rulers-guides".to_owned(), "todo".to_owned()]);
    assert_eq!(resumed.level, NotificationLevel::Info);
    assert_eq!(resumed.message_key, RESUMED_KEY);
    assert_eq!(resumed.args["plugins"], "rulers-guides, todo");
    let none = notice(&[]);
    assert_eq!(none.message_key, NONE_PAUSED_KEY);
    assert!(none.args.is_empty());
}
