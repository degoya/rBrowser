use serde_json::json;

use super::*;

fn options() -> FeedOptions {
    FeedOptions {
        buffer_size: 1000,
        clear_on_navigate: true,
        network: NetworkLevel::Errors,
        cleared_at: 0.0,
        rbrowser: false,
    }
}

fn capture(views: serde_json::Value) -> serde_json::Value {
    json!({ FEED_KEY: { "views": views } })
}

#[test]
fn no_capture_is_an_empty_console_that_says_so() {
    let feed = read(&json!({}), &options()).expect("no capture reads");
    assert!(!feed.captured);
    assert!(feed.entries.is_empty());
}

#[test]
fn a_malformed_capture_is_refused() {
    let args = json!({ FEED_KEY: { "views": [{ "view": "three" }] } });
    assert!(read(&args, &options()).is_err());
}

#[test]
fn console_errors_and_exceptions_become_entries_oldest_first() {
    let args = capture(json!([{
        "view": 1,
        "console": [
            { "level": "warning", "source": "console-api", "text": "w", "url": "a.js", "line": 3, "time": 20.0 },
            { "level": "error", "source": "network", "text": "Failed to load resource", "time": 10.0 },
            { "level": "verbose", "source": "console-api", "text": "v", "time": 30.0 },
        ],
        "errors": [{ "text": "Uncaught TypeError: x", "url": "a.js", "line": 9, "column": 2, "stack": "at f (a.js:9:2)", "time": 15.0 }],
    }]));
    let feed = read(&args, &options()).expect("reads");
    assert!(feed.captured);
    let seen: Vec<(Level, Kind, &str)> = feed
        .entries
        .iter()
        .map(|e| (e.level, e.kind, e.text.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            (Level::Error, Kind::Browser, "Failed to load resource"),
            (Level::Error, Kind::Exception, "Uncaught TypeError: x"),
            (Level::Warning, Kind::Console, "w"),
            (Level::Debug, Kind::Console, "v"),
        ]
    );
    assert_eq!(feed.entries[1].stack.as_deref(), Some("at f (a.js:9:2)"));
    assert_eq!(feed.newest, 30.0);
}

#[test]
fn levels_follow_the_engines_names() {
    assert_eq!(Level::of_console("warn"), Level::Warning);
    assert_eq!(Level::of_console("assert"), Level::Error);
    assert_eq!(Level::of_console("table"), Level::Log);
    assert_eq!(Level::of_console("DEBUG"), Level::Debug);
    assert_eq!(Level::parse("network"), Some(Level::Network));
    assert_eq!(Level::parse("fatal"), None);
}

fn requests() -> serde_json::Value {
    capture(json!([{ "view": 2, "network": [
        { "url": "http://x.test/api/missing.json", "method": "GET", "resourceType": "Fetch", "status": 404, "time": 1.0 },
        { "url": "http://x.test/api/ok.json", "method": "POST", "resourceType": "XHR", "status": 200, "time": 2.0 },
        { "url": "http://x.test/api/down", "method": "GET", "resourceType": "Fetch", "failed": "net::ERR_CONNECTION_REFUSED", "time": 3.0 },
        { "url": "http://x.test/missing.png", "method": "GET", "resourceType": "Image", "status": 404, "time": 4.0 },
    ]}]))
}

#[test]
fn failed_fetch_and_xhr_are_the_level_network() {
    let feed = read(&requests(), &options()).expect("reads");
    let texts: Vec<&str> = feed.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "GET http://x.test/api/missing.json 404",
            "GET http://x.test/api/down net::ERR_CONNECTION_REFUSED",
        ]
    );
    assert!(feed.entries.iter().all(|e| e.level == Level::Network));
}

#[test]
fn network_level_all_and_off() {
    let all = FeedOptions {
        network: NetworkLevel::All,
        ..options()
    };
    assert_eq!(read(&requests(), &all).expect("reads").entries.len(), 3);
    let off = FeedOptions {
        network: NetworkLevel::Off,
        ..options()
    };
    assert!(read(&requests(), &off).expect("reads").entries.is_empty());
    assert_eq!(NetworkLevel::parse("bogus"), NetworkLevel::Errors);
}

fn three_logs() -> serde_json::Value {
    capture(json!([{
        "view": 1,
        // The newest time of the document before: what came after it is the current one's.
        "navigatedAt": 15.0,
        "console": [
            { "level": "log", "text": "a", "time": 10.0 },
            { "level": "log", "text": "b", "time": 20.0 },
            { "level": "log", "text": "c", "time": 30.0 },
        ],
    }]))
}

#[test]
fn messages_before_the_last_navigation_leave_unless_switched_off() {
    let feed = read(&three_logs(), &options()).expect("reads");
    let texts: Vec<&str> = feed.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["b", "c"]);
    // A message at the mark itself is the old document's.
    let mut at_mark = three_logs();
    at_mark[FEED_KEY]["views"][0]["navigatedAt"] = json!(20.0);
    let feed = read(&at_mark, &options()).expect("reads");
    let texts: Vec<&str> = feed.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["c"]);
    let keep = FeedOptions {
        clear_on_navigate: false,
        ..options()
    };
    assert_eq!(read(&three_logs(), &keep).expect("reads").entries.len(), 3);
}

#[test]
fn the_clear_mark_and_the_buffer_size_cut_the_old_messages() {
    let cleared = FeedOptions {
        clear_on_navigate: false,
        cleared_at: 10.0,
        ..options()
    };
    let feed = read(&three_logs(), &cleared).expect("reads");
    assert_eq!(feed.entries.len(), 2);
    // The newest of the capture counts cleared messages too: a clear marks all of it.
    assert_eq!(feed.newest, 30.0);
    let small = FeedOptions {
        clear_on_navigate: false,
        buffer_size: 1,
        ..options()
    };
    let feed = read(&three_logs(), &small).expect("reads");
    assert_eq!(feed.entries.len(), 1);
    assert_eq!(feed.entries[0].text, "c");
}

#[test]
fn long_texts_are_cut() {
    let long = "x".repeat(MAX_TEXT_CHARS + 50);
    let args =
        capture(json!([{ "view": 1, "console": [{ "level": "log", "text": long, "time": 1.0 }] }]));
    let feed = read(&args, &options()).expect("reads");
    assert_eq!(feed.entries[0].text.chars().count(), MAX_TEXT_CHARS);
}

// RB-080-66: what rBrowser's plugins logged (source `rbrowser`, from the host's page-script
// log) is no message of the page: its own kind, hidden unless `console.showRbrowserMessages`.
fn plugin_line() -> serde_json::Value {
    capture(json!([{
        "view": 2,
        "console": [
            { "level": "warning", "source": "rbrowser", "plugin": "rulers-guides",
              "text": "The plugin is still answering earlier requests.",
              "url": "rbrowser://plugin/rulers-guides", "time": 5.0 },
            { "level": "warning", "source": "console-api", "text": "the page's own", "time": 6.0 },
        ],
    }]))
}

#[test]
fn rbrowsers_own_messages_are_hidden_unless_asked_for() {
    let feed = read(&plugin_line(), &options()).expect("reads");
    let texts: Vec<&str> = feed.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["the page's own"]);
    let shown = FeedOptions {
        rbrowser: true,
        ..options()
    };
    let feed = read(&plugin_line(), &shown).expect("reads");
    assert_eq!(feed.entries.len(), 2);
    assert_eq!(feed.entries[0].kind, Kind::Rbrowser);
    assert_eq!(
        feed.entries[0].url.as_deref(),
        Some("rbrowser://plugin/rulers-guides")
    );
    assert_eq!(Kind::Rbrowser.as_str(), "rbrowser");
}

// Review of RB-080-66, 4: a plugin's page script that failed (a stack) is rBrowser's own failure,
// not noise: it shows even with `console.showRbrowserMessages` off.
#[test]
fn a_failed_page_script_shows_even_with_rbrowsers_messages_off() {
    let args = capture(json!([{
        "view": 2,
        "console": [
            { "level": "error", "source": "rbrowser", "plugin": "demo", "text": "boom",
              "stack": "Error: boom\n    at run", "url": "rbrowser://plugin/demo", "time": 5.0 },
            { "level": "warning", "source": "rbrowser", "plugin": "demo", "text": "busy", "time": 6.0 },
        ],
    }]));
    let feed = read(&args, &options()).expect("reads");
    let texts: Vec<&str> = feed.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["boom"]);
    assert_eq!(feed.entries[0].kind, Kind::Rbrowser);
    assert!(
        feed.entries[0]
            .stack
            .as_deref()
            .is_some_and(|s| s.contains("at run"))
    );
}
