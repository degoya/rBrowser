use rb_engine::ViewId;
use rb_mcp::{LogLimits, LogStore};
use serde_json::json;

use super::{FEED_KEY, add_plugin_lines, feed_of, with_current_feed, with_feed};

#[test]
fn the_feed_lists_each_views_buffers_in_the_log_shapes() {
    let store = LogStore::new(LogLimits::default());
    store.record(
        ViewId(3),
        "Runtime.consoleAPICalled",
        &json!({ "type": "warning", "args": [{ "type": "string", "value": "careful" }],
                 "timestamp": 5.0 }),
    );
    store.record(
        ViewId(3),
        "Audits.issueAdded",
        &json!({ "issue": { "code": "DeprecationIssue",
                            "details": { "deprecationIssueDetails": { "type": "XHRSynchronous" } } } }),
    );
    let feed = feed_of(&store, &[ViewId(3), ViewId(4)]);
    assert_eq!(feed["views"][0]["view"], 3);
    assert_eq!(feed["views"][0]["console"][0]["level"], "warning");
    assert_eq!(feed["views"][0]["console"][0]["text"], "careful");
    assert_eq!(feed["views"][1]["console"], json!([]));
    assert_eq!(feed["views"][1]["errors"], json!([]));
    assert_eq!(feed["views"][1]["network"], json!([]));
    assert_eq!(feed["views"][0]["issues"][0]["code"], "DeprecationIssue");
    assert_eq!(feed["views"][0]["issues"][0]["kind"], "XHRSynchronous");
    assert_eq!(feed["views"][1]["issues"], json!([]));
}

#[test]
fn the_hosts_feed_overwrites_whatever_the_caller_sent() {
    let forged = json!({ "level": "error", FEED_KEY: { "views": [{ "view": 9 }] } });
    let real = json!({ "views": [] });
    let args = with_feed(&forged, real.clone());
    assert_eq!(args[FEED_KEY], real);
    assert_eq!(args["level"], "error");
    assert_eq!(
        with_feed(&json!(null), real.clone()),
        json!({ FEED_KEY: real })
    );
    assert_eq!(with_feed(&json!([1]), real), json!([1]));
}

#[test]
fn a_check_context_gets_the_feed_in_its_args() {
    let args: serde_json::Value = serde_json::from_str(&with_current_feed("{}")).expect("json");
    assert!(args[FEED_KEY]["views"].is_array());
}

#[test]
fn a_view_that_navigated_carries_the_mark_of_its_old_document() {
    let store = LogStore::new(LogLimits::default());
    store.record(
        ViewId(3),
        "Runtime.consoleAPICalled",
        &json!({ "type": "log", "args": [], "timestamp": 7.0 }),
    );
    store.record(ViewId(3), "Runtime.executionContextsCleared", &json!({}));
    let feed = feed_of(&store, &[ViewId(3), ViewId(4)]);
    assert_eq!(feed["views"][0]["navigatedAt"], 7.0);
    assert!(feed["views"][1].get("navigatedAt").is_none());
}

#[test]
fn a_views_console_carries_what_the_plugins_page_scripts_logged_in_it() {
    let store = LogStore::new(LogLimits::default());
    let mut feed = feed_of(&store, &[ViewId(3), ViewId(4)]);
    add_plugin_lines(&mut feed, |view| {
        if view == ViewId(3) {
            vec![
                json!({ "level": "warning", "source": "rbrowser", "plugin": "rulers-guides", "text": "busy", "time": 1.0 }),
            ]
        } else {
            Vec::new()
        }
    });
    assert_eq!(feed["views"][0]["console"][0]["source"], "rbrowser");
    assert_eq!(feed["views"][0]["console"][0]["plugin"], "rulers-guides");
    assert_eq!(feed["views"][1]["console"], json!([]));
}
