use serde_json::json;

use super::*;
use crate::har::tests::{assert_har_1_2, capture, host};

fn with_state(mut args: Value) -> Value {
    args["$network"] = json!({ "session": "s1", "trusted": true });
    args
}

#[test]
fn a_session_is_saved_as_har_with_the_views_marks_and_masked_urls() {
    let mut host = host();
    let mut args = capture();
    args["$console"]["views"][0]["network"][0]["url"] =
        json!("http://127.0.0.1:8080/api/user?id=7&token=abc");
    args["$console"]["views"][1]["network"][0]["url"] =
        json!("http://127.0.0.1:8080/api/user?id=7&token=abc");
    let saved = save(&mut host, &with_state(args)).expect("saved");
    assert_eq!(saved["path"], format!("{PATH}/2024-11-11T04-40-00Z.har"));
    assert_eq!(saved["entries"], 3);
    let text = host
        .out("sessions/2024-11-11T04-40-00Z.har")
        .expect("written");
    let har: Value = serde_json::from_str(&text).expect("JSON");
    assert_har_1_2(&har);
    let entries = har["log"]["entries"].as_array().expect("entries");
    let user = entries
        .iter()
        .find(|entry| {
            entry["_view"] == 1
                && entry["request"]["url"]
                    .as_str()
                    .is_some_and(|url| url.contains("/api/user"))
        })
        .expect("the user request of the first view");
    assert_eq!(
        user["request"]["url"],
        "http://127.0.0.1:8080/api/user?id=7&token=***"
    );
    assert_eq!(
        user["request"]["queryString"][1],
        json!({ "name": "token", "value": "***" })
    );
    assert_eq!(user["_breakpoint"], "mobile");
    assert_eq!(user["_partial"], false);
    let tracking = entries
        .iter()
        .find(|entry| {
            entry["request"]["url"]
                .as_str()
                .is_some_and(|url| url.contains("tracking"))
        })
        .expect("the tracking request");
    assert_eq!(tracking["_partial"], true);
    assert_eq!(har["log"]["_rbrowser"]["kind"], "session");
    assert!(text.contains("\"_viewLabel\":\"Mobile\""));
    assert!(save(&mut host, &with_state(json!({ "name": "../x" }))).is_err());
    let named = save(&mut host, &with_state(json!({ "name": "login-flow" }))).expect("saved");
    assert_eq!(named["path"], format!("{PATH}/login-flow.har"));
    assert_eq!(
        list(&mut host)["sessions"].as_array().map(Vec::len),
        Some(2)
    );
}

#[test]
fn a_saved_session_opens_as_the_lists_rows_and_nothing_else() {
    let mut host = host();
    let saved = save(&mut host, &with_state(capture())).expect("saved");
    let path = saved["path"].as_str().expect("a path").to_owned();
    let opened = open(&mut host, &json!({ "path": path })).expect("opened");
    assert_eq!(opened["readOnly"], true);
    assert_eq!(opened["total"], 2);
    assert_eq!(
        opened["views"],
        json!([{ "id": 1, "label": "Mobile" }, { "id": 2, "label": "Desktop" }])
    );
    let rows = opened["rows"].as_array().expect("rows");
    let tracking = rows
        .iter()
        .find(|row| row["url"].as_str().is_some_and(|u| u.contains("tracking")))
        .expect("row");
    assert_eq!(tracking["partial"], true);
    assert_eq!(
        tracking["views"]["1"]["failed"],
        "net::ERR_BLOCKED_BY_CLIENT"
    );
    let user = rows
        .iter()
        .find(|row| row["url"].as_str().is_some_and(|u| u.contains("user")))
        .expect("row");
    assert_eq!(user["views"]["2"]["status"], 200);
    for bad in ["x.har", ".rbrowser/../x.har", ".rbrowser/out/x.json"] {
        assert!(open(&mut host, &json!({ "path": bad })).is_err(), "{bad}");
    }
    host.file(".rbrowser/out/network-panel/sessions/bad.har", "{}");
    assert!(
        open(
            &mut host,
            &json!({ "path": ".rbrowser/out/network-panel/sessions/bad.har" })
        )
        .is_err()
    );
}

#[test]
fn the_headers_of_a_saved_entry_follow_the_apps_list() {
    let mut har = json!({ "log": { "entries": [{ "_view": 1, "request": { "method": "GET", "url": "http://x.test/",
        "headers": [{ "name": "X-CSRF-Token", "value": "t1" }, { "name": "Accept", "value": "*/*" }] },
        "response": { "headers": [{ "name": "WWW-Authenticate", "value": "Basic realm=x" }] } }] } });
    annotate(&mut har, &requests::capture(&json!({})), &BTreeMap::new());
    let entry = &har["log"]["entries"][0];
    assert_eq!(entry["request"]["headers"][0]["value"], "***");
    assert_eq!(entry["request"]["headers"][1]["value"], "*/*");
    assert_eq!(entry["response"]["headers"][0]["value"], "***");
}

#[test]
fn urls_are_masked_case_blind_and_the_rest_kept() {
    assert_eq!(
        mask_url("https://a.test/p?q=1&Token=abc#f"),
        "https://a.test/p?q=1&Token=***#f"
    );
    assert_eq!(mask_url("https://a.test/p"), "https://a.test/p");
    // RB-094-10: a signed URL's credentials and the fragment's tokens too.
    assert_eq!(
        mask_url("https://b.s3.test/k?X-Amz-Signature=ab&x=1#id_token=e.y.j"),
        "https://b.s3.test/k?X-Amz-Signature=***&x=1#id_token=***"
    );
}

#[test]
fn stamps_read_back_into_milliseconds() {
    assert_eq!(
        time_of(&har::iso8601(1_731_300_000_123.0)),
        1_731_300_000_123.0
    );
    assert_eq!(time_of("nonsense"), 0.0);
}
