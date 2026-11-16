use serde_json::json;

use super::*;
use crate::fake_host::{FakeHost, view};
use crate::har;

fn with_state(mut args: Value, trusted: bool) -> Value {
    args["$network"] = json!({ "session": "s1", "trusted": trusted, "applied": 1,
        "views": [{ "view": 1, "supported": true, "intercepting": true },
                  { "view": 2, "supported": false, "intercepting": false }] });
    args
}

#[test]
fn the_list_carries_rows_views_and_the_switches() {
    let mut host = FakeHost::with_views(vec![view(1, "Mobile", 390), view(2, "Firefox", 1440)]);
    let answer = list(&mut host, &with_state(har::tests::capture(), true)).expect("a list");
    assert_eq!(answer["total"], 2);
    assert_eq!(answer["captured"], true);
    assert_eq!(answer["views"][0]["intercepting"], true);
    assert_eq!(answer["views"][1]["supported"], false);
    assert_eq!(answer["mocksEnabled"], true);
    assert_eq!(answer["rulesRunning"], true);
    assert!(answer.to_string().len() < 64 * 1024);
}

#[test]
fn bad_list_arguments_are_refused() {
    let mut host = FakeHost::default();
    for args in [
        json!({ "status": "weird" }),
        json!({ "limit": 0 }),
        json!({ "types": "Image" }),
    ] {
        assert!(list(&mut host, &args).is_err(), "{args}");
    }
}

#[test]
fn the_switches_flip_for_the_session_and_post_to_the_panel() {
    let mut host = FakeHost::default();
    let args = with_state(json!({}), true);
    let off = mocks_toggle(&mut host, &args).expect("toggled");
    assert_eq!(off["mocksEnabled"], false);
    assert_eq!(off["$post"][0]["message"]["mocksEnabled"], false);
    let on = mocks_toggle(&mut host, &args).expect("toggled");
    assert_eq!(on["mocksEnabled"], true);
    let cache =
        cache_toggle(&mut host, &with_state(json!({ "disabled": true }), true)).expect("set");
    assert_eq!(cache["cacheDisabled"], true);
    assert_eq!(store::read(&mut host, "s1").cache_disabled, Some(true));
}

#[test]
fn clearing_site_data_waits_for_the_panels_confirmation() {
    let mut host = FakeHost::default();
    let asked = site_data_clear(&mut host, &with_state(json!({}), true)).expect("asks");
    assert_eq!(asked["confirm"], true);
    assert_eq!(asked["$post"][0]["message"]["confirmClear"], true);
    assert_eq!(store::read(&mut host, "s1").clear_site_data, 0);
    site_data_clear(&mut host, &with_state(json!({ "confirmed": true }), true)).expect("cleared");
    assert_eq!(store::read(&mut host, "s1").clear_site_data, 1);
}

#[test]
fn the_body_preview_answers_or_says_there_is_none() {
    let mut host = har::tests::host();
    let body = body(&mut host, &json!({ "view": 1, "id": "1.1" })).expect("a body");
    assert_eq!(body["text"], "{\"name\":\"Ada\"}");
    assert_eq!(body["truncated"], false);
    let none = super::body(&mut host, &json!({ "view": 2, "id": "2.1" })).expect_err("gone");
    assert_eq!(
        none.code.as_deref(),
        Some("plugin.network-panel.errors.no_body")
    );
}

#[test]
fn har_import_needs_trust_and_har_export_takes_a_target_view() {
    let mut host = har::tests::host();
    let refused =
        har_import(&mut host, &json!({ "path": ".rbrowser/x.har" })).expect_err("untrusted");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.network-panel.errors.untrusted")
    );
    let mut args = har::tests::capture();
    args["target"] = json!(2);
    let answer = har_export(&mut host, &args).expect("written");
    assert_eq!(answer["entries"], 1);
    assert!(
        answer["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("-view2.har"))
    );
}
