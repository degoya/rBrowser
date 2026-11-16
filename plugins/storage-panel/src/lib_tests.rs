//! The job's acceptance, through the plugin's `feature` export as the host calls it.

use rb_plugin_api::ViewId;
use serde_json::{Value, json};

use super::*;
use crate::browser::{Browser, view};
use crate::columns::CONFIG_PATH;

const ORIGIN: &str = "http://127.0.0.1:18767";
const PAGE: &str = "http://127.0.0.1:18767/storage/index.html";
const OTHER: &str = "http://127.0.0.1:18767/storage/other.html";

/// The fixture: two views in `default`, one in `admin`, a Firefox and a WebKit view.
fn browser() -> Browser {
    let mut browser = Browser::default();
    browser.project.insert(
        CONFIG_PATH.into(),
        br#"{"sessions":["admin"],"breakpoints":[{"id":"mobile"},{"id":"tablet"},
            {"id":"desktop","session":"admin"},{"id":"fx","engine":"firefox"},
            {"id":"wk","engine":"webkit"}]}"#
            .to_vec(),
    );
    browser.add_view(view(1, "mobile", "chromium", PAGE), "default");
    browser.add_view(view(2, "tablet", "chromium", PAGE), "default");
    browser.add_view(view(3, "desktop", "chromium", PAGE), "admin");
    browser.add_view(view(4, "fx", "firefox", PAGE), "firefox");
    browser.add_view(view(5, "wk", "webkit", PAGE), "webkit");
    // What the fixture's server and page set in the default session.
    browser.set_cookie(
        1,
        json!({ "name": "consent", "value": "yes", "domain": "127.0.0.1", "path": "/" }),
    );
    browser.set_cookie(
        1,
        json!({ "name": "session_id", "value": "abc123", "domain": "127.0.0.1", "path": "/",
                "httpOnly": true }),
    );
    browser
}

fn command(browser: &mut Browser, args: &Value) -> Value {
    let answer = StoragePanel
        .run_command(browser, PANEL, &args.to_string())
        .expect("the command answers");
    serde_json::from_str(&answer).expect("JSON")
}

fn tool(browser: &mut Browser, tool: &str, args: &Value) -> Result<Value, Failure> {
    StoragePanel
        .call_tool(browser, tool, &args.to_string())
        .map(|answer| serde_json::from_str(&answer).expect("JSON"))
}

/// A reload as the view runs it: page/pins.js at document start, the page's first inline
/// script (it logs `featureFlag`, as the fixture does), then the answer to `loaded`.
fn reload(browser: &mut Browser, view: ViewId) -> Option<String> {
    browser.run_page_script(view);
    let first_script_saw = browser.local_item(view, "featureFlag");
    // The page itself turns the flag off again; the pin must win at the next start.
    browser.set_local(view, "featureFlag", "off");
    let message = json!({ "view": view, "message": { "type": "loaded",
                                                      "origin": browser.origin(view) } });
    let answer = tool(browser, PAGE_TOOL, &message).expect("answers");
    browser.take_page_answer(view, &answer["record"]);
    first_script_saw
}

#[test]
fn the_consent_cookie_shows_in_default_and_is_missing_in_admin() {
    let mut browser = browser();
    let list = command(&mut browser, &json!({ "op": "list", "kind": "cookies" }));
    let consent = list["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["name"] == "consent")
        .expect("listed");
    assert_eq!(consent["state"], "missing");
    assert_eq!(consent["cells"]["1"]["value"], "yes");
    assert_eq!(consent["cells"]["2"]["value"], "yes");
    assert_eq!(consent["cells"]["3"], Value::Null);
}

#[test]
fn a_local_storage_value_edited_in_the_panel_reads_back_and_applies_to_all() {
    let mut browser = browser();
    let one = json!({ "op": "set", "kind": "local", "target": 3, "name": "featureFlag",
                      "value": "beta" });
    command(&mut browser, &one);
    assert_eq!(
        browser.local_item(3, "featureFlag").as_deref(),
        Some("beta")
    );
    assert_eq!(browser.local_item(1, "featureFlag"), None);

    let all = json!({ "op": "set", "kind": "local", "target": "all", "name": "featureFlag",
                      "value": "on" });
    let answer = command(&mut browser, &all);
    for id in 1..=3 {
        assert_eq!(browser.local_item(id, "featureFlag").as_deref(), Some("on"));
    }
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(5));
}

#[test]
fn a_pinned_value_is_set_before_the_first_script_after_reload_and_navigation() {
    let mut browser = browser();
    browser.set_local(1, "featureFlag", "on");
    command(
        &mut browser,
        &json!({ "op": "pin", "kind": "local", "view": 1, "scope": ORIGIN,
                 "name": "featureFlag" }),
    );
    assert_eq!(reload(&mut browser, 1).as_deref(), Some("on"));
    assert_eq!(reload(&mut browser, 1).as_deref(), Some("on"));
    browser.navigate(1, OTHER);
    assert_eq!(reload(&mut browser, 1).as_deref(), Some("on"));
    // The admin view got the pin when it was made.
    assert_eq!(reload(&mut browser, 3).as_deref(), Some("on"));
}

#[test]
fn an_httponly_cookie_is_shown_and_deleted() {
    let mut browser = browser();
    let list = command(&mut browser, &json!({ "op": "list", "kind": "cookies" }));
    let sid = list["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["name"] == "session_id")
        .expect("listed")
        .clone();
    assert_eq!(sid["cells"]["1"]["httpOnly"], true);
    assert_eq!(sid["cells"]["1"]["value"], "abc123");
    command(
        &mut browser,
        &json!({ "op": "delete", "kind": "cookies", "target": 1, "name": "session_id",
                 "cookie": { "domain": sid["scope"], "path": sid["path"] } }),
    );
    assert!(
        browser
            .cookies_of(2)
            .iter()
            .all(|cookie| cookie["name"] != "session_id")
    );
}

#[test]
fn firefox_and_webkit_views_are_columns_marked_unsupported() {
    let mut browser = browser();
    let list = command(&mut browser, &json!({ "op": "list", "kind": "local" }));
    let status: Vec<(&str, &str)> = list["views"]
        .as_array()
        .expect("views")
        .iter()
        .map(|c| {
            (
                c["engine"].as_str().unwrap_or(""),
                c["status"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(
        status,
        [
            ("chromium", "ok"),
            ("chromium", "ok"),
            ("chromium", "ok"),
            ("firefox", "unsupported"),
            ("webkit", "unsupported"),
        ]
    );
}

#[test]
fn storage_get_answers_per_view_and_storage_set_reports_per_view() {
    let mut browser = browser();
    browser.settings.insert("mcpValues".into(), "true".into());
    let got = tool(&mut browser, GET, &json!({ "target": "all" })).expect("answers");
    let views = got["views"].as_array().expect("views");
    assert_eq!(views.len(), 5);
    assert!(views[0]["cookies"].is_array());
    assert!(views[0]["localStorage"].is_array());
    assert!(views[0]["sessionStorage"].is_array());

    let set = tool(
        &mut browser,
        SET,
        &json!({ "kind": "sessionStorage", "name": "step", "value": "2", "target": "all" }),
    )
    .expect("written");
    let ok: Vec<bool> = set["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|r| r["ok"] == true)
        .collect();
    assert_eq!(ok, [true, true, true, false, false]);
    assert!(set.get("$post").is_some(), "the host takes the posts out");
}

#[test]
fn unknown_contributions_and_broken_arguments_are_refused() {
    let mut browser = browser();
    let unknown = StoragePanel.call_tool(&mut browser, "drop", "{}");
    assert_eq!(
        unknown.expect_err("refused").code.as_deref(),
        Some(codes::UNKNOWN_CONTRIBUTION)
    );
    assert!(StoragePanel.run_command(&mut browser, "x", "{}").is_err());
    let broken = StoragePanel.call_tool(&mut browser, GET, "[1]");
    assert_eq!(
        broken.expect_err("refused").code.as_deref(),
        Some("plugin.storage-panel.errors.bad_args")
    );
    assert_eq!(parse(" ").expect("empty is {}"), json!({}));
}
