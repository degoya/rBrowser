use serde_json::json;

use super::*;
use crate::browser::{Browser, view};
use crate::columns::CONFIG_PATH;

const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";

/// Three views - two in `default`, one in `admin` - and a WebKit view.
fn browser() -> Browser {
    let mut browser = Browser::default();
    browser.project.insert(
        CONFIG_PATH.into(),
        br#"{"breakpoints":[{"id":"a"},{"id":"b"},{"id":"c","session":"admin"},{"id":"w"}]}"#
            .to_vec(),
    );
    browser.add_view(view(1, "a", "chromium", PAGE), "default");
    browser.add_view(view(2, "b", "chromium", PAGE), "default");
    browser.add_view(view(3, "c", "chromium", PAGE), "admin");
    browser.add_view(view(4, "w", "webkit", PAGE), "webkit");
    browser.set_cookie(
        1,
        json!({ "name": "sid", "value": "TOPSECRET", "domain": "127.0.0.1", "path": "/",
                "httpOnly": true }),
    );
    browser.set_local(1, "featureFlag", "off");
    browser.set_local(3, "featureFlag", "on");
    browser
}

#[test]
fn storage_get_answers_every_view_without_values_by_default() {
    let mut browser = browser();
    let answer = get(&mut browser, &json!({ "target": "all" })).expect("answers");
    assert_eq!(answer["values"], "hidden");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 4);
    assert_eq!(views[0]["cookies"][0]["name"], "sid");
    assert_eq!(views[0]["cookies"][0]["httpOnly"], true);
    assert_eq!(views[0]["cookies"][0]["length"], 9);
    assert_eq!(views[0]["localStorage"][0]["key"], "featureFlag");
    assert_eq!(views[2]["session"], "admin");
    assert_eq!(views[3]["status"], "unsupported");
    assert!(!answer.to_string().contains("TOPSECRET"));
    assert!(!answer.to_string().contains("\"off\""));

    let differences = answer["differences"].as_array().expect("differences");
    let summary: Vec<(&str, &str, &str)> = differences
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap_or(""),
                d["name"].as_str().unwrap_or(""),
                d["state"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("cookies", "sid", "missing"),
            ("localStorage", "featureFlag", "differs")
        ]
    );
}

#[test]
fn storage_get_answers_values_once_a_person_allows_it() {
    let mut browser = browser();
    browser.settings.insert("mcpValues".into(), "true".into());
    let answer = get(&mut browser, &json!({ "target": 1, "kinds": ["cookies"] })).expect("answers");
    assert_eq!(answer["values"], "shown");
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0]["cookies"][0]["value"], "TOPSECRET");
    assert!(views[0].get("localStorage").is_none());
}

#[test]
fn storage_get_filters_by_origin_and_name_and_hides_httponly_on_request() {
    let mut browser = browser();
    browser
        .settings
        .insert("showHttpOnly".into(), "false".into());
    let answer = get(&mut browser, &json!({ "search": "FEATURE" })).expect("answers");
    assert_eq!(answer["views"][0]["cookies"], json!([]));
    assert_eq!(answer["views"][0]["localStorage"][0]["key"], "featureFlag");
    let none = get(&mut browser, &json!({ "origin": "example.com" })).expect("answers");
    assert_eq!(none["views"][0]["localStorage"], json!([]));
    assert_eq!(none["differences"], json!([]));
    for args in [
        json!({ "kinds": ["indexedDB"] }),
        json!({ "kinds": "cookies" }),
        json!({ "search": 3 }),
        json!({ "target": "active" }),
    ] {
        assert!(get(&mut browser, &args).is_err(), "{args}");
    }
}

#[test]
fn storage_set_is_refused_until_a_person_allows_values() {
    let mut browser = browser();
    let args = json!({ "kind": "localStorage", "name": "featureFlag", "value": "on",
                       "target": "all" });
    let failure = set(&mut browser, &args).expect_err("refused");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.storage-panel.errors.mcp_values_off")
    );
    assert!(browser.evaluated.is_empty());

    browser.settings.insert("mcpValues".into(), "true".into());
    let answer = set(&mut browser, &args).expect("written");
    let results = answer["results"].as_array().expect("results");
    assert_eq!(results.len(), 4);
    assert!(results[..3].iter().all(|r| r["ok"] == true));
    assert_eq!(
        results[3]["error"],
        "plugin.storage-panel.errors.unsupported_engine"
    );
    assert_eq!(browser.local_item(3, "featureFlag").as_deref(), Some("on"));
    assert_eq!(answer["$post"][0]["target"], "panel");
}

#[test]
fn nothing_is_logged() {
    let mut browser = browser();
    browser.settings.insert("mcpValues".into(), "true".into());
    get(&mut browser, &json!({})).expect("answers");
    set(
        &mut browser,
        &json!({ "kind": "cookie", "name": "sid", "value": "NEWSECRET" }),
    )
    .expect("written");
    assert!(browser.logs.is_empty());
}
