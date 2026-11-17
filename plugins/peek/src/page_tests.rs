use serde_json::{Value, json};

use super::*;
use crate::testing::FakeHost;

fn call(host: &mut FakeHost, view: u32, message: Value) -> Value {
    let args = json!({ "view": view, "message": message }).to_string();
    serde_json::from_str(&handle(host, &args).expect("answers")).expect("JSON")
}

fn data() -> Value {
    json!({
        "tag": "a", "color": "rgb(0, 0, 0)", "backgrounds": [{ "color": "rgb(255, 255, 255)" }],
        "font": { "size": 16, "weight": 400 }, "role": "link", "name": "Home",
        "selector": "#home",
        "target": { "element": { "kind": "css", "path": "#home" }, "offset": { "x": 0.5, "y": 0.5 } },
    })
}

#[test]
fn the_page_script_asks_for_its_settings() {
    let mut host = FakeHost::default();
    host.settings
        .insert("modifier".to_owned(), "\"Ctrl\"".to_owned());
    let answer = call(&mut host, 1, json!({ "type": "config" }));
    assert_eq!(answer["config"]["modifier"], "Ctrl");
    assert_eq!(answer["config"]["enabled"], true);
    assert!(answer.get("$post").is_none());
}

#[test]
fn an_inspection_is_answered_and_the_element_posted_to_every_view() {
    let mut host = FakeHost::default();
    let answer = call(&mut host, 3, json!({ "type": "inspect", "data": data() }));
    assert_eq!(answer["inspection"]["tag"], "a");
    assert_eq!(answer["inspection"]["contrast"]["ratio"], 21.0);
    assert_eq!(
        answer["$post"],
        json!([{ "target": "all", "message": {
            "type": "highlight", "from": 3, "target": data()["target"] } }])
    );
    // The selector went to CDP (which this host lacks: the page's role stands).
    assert_eq!(host.methods(), ["Accessibility.getFullAXTree"]);
    assert_eq!(answer["inspection"]["accessibility"], "page");
}

#[test]
fn without_sync_highlight_nothing_is_posted() {
    let mut host = FakeHost::default();
    host.settings
        .insert("syncHighlight".to_owned(), "false".to_owned());
    let answer = call(&mut host, 3, json!({ "type": "inspect", "data": data() }));
    assert!(answer.get("$post").is_none());
    let answer = call(&mut host, 3, json!({ "type": "clear" }));
    assert!(answer.get("$post").is_none());
}

#[test]
fn a_huge_target_is_not_posted() {
    let mut host = FakeHost::default();
    let mut data = data();
    data["target"]["element"]["path"] = json!("p".repeat(MAX_TARGET_BYTES));
    let answer = call(&mut host, 3, json!({ "type": "inspect", "data": data }));
    assert!(answer.get("$post").is_none());
    assert_eq!(answer["inspection"]["tag"], "a");
}

#[test]
fn clear_removes_the_highlight_everywhere() {
    let mut host = FakeHost::default();
    let answer = call(&mut host, 2, json!({ "type": "clear" }));
    assert_eq!(
        answer["$post"],
        json!([{ "target": "all", "message": { "type": "clear", "from": 2 } }])
    );
}

#[test]
fn the_point_is_the_locator_when_the_page_has_no_selector() {
    let mut host = FakeHost::default();
    let mut data = data();
    data["selector"] = Value::Null;
    data["point"] = json!({ "x": 5, "y": 6 });
    call(&mut host, 1, json!({ "type": "inspect", "data": data }));
    assert_eq!(host.methods(), ["DOM.getNodeForLocation"]);
}

#[test]
fn malformed_requests_are_refused() {
    let mut host = FakeHost::default();
    for args in [
        "not json".to_owned(),
        json!({ "message": { "type": "config" } }).to_string(),
        json!({ "view": 1, "message": { "type": "launch" } }).to_string(),
        json!({ "view": 1, "message": { "type": "inspect" } }).to_string(),
    ] {
        let failure = handle(&mut host, &args).expect_err("refused");
        assert_eq!(
            failure.code.as_deref(),
            Some(rb_plugin_api::codes::INVALID),
            "{args}"
        );
    }
}

#[test]
fn locators_are_checked() {
    assert_eq!(
        locator(&json!({ "selector": "#a" })),
        Some(Locator::Selector("#a".to_owned()))
    );
    assert_eq!(locator(&json!({ "selector": "" })), None);
    assert_eq!(
        locator(&json!({ "selector": "a".repeat(MAX_SELECTOR + 1) })),
        None
    );
    assert_eq!(
        locator(&json!({ "point": { "x": 1.5, "y": 2 } })),
        Some(Locator::Point { x: 1.5, y: 2.0 })
    );
    assert_eq!(locator(&json!({ "point": { "x": -1, "y": 2 } })), None);
    assert_eq!(locator(&json!({})), None);
}

/// RB-090-54: the pointer is in viewport px, `DOM.getNodeForLocation` takes document px - the
/// page's scroll moves the point into the document, as Inspect does since RB-090-51.
#[test]
fn a_point_on_a_scrolled_page_is_moved_into_the_document() {
    assert_eq!(
        locator(&json!({ "point": { "x": 10, "y": 20 }, "scroll": { "x": 30, "y": 500 } })),
        Some(Locator::Point { x: 40.0, y: 520.0 })
    );
    // A long page: far below the viewport's bound, still a point of the document.
    assert_eq!(
        locator(&json!({ "point": { "x": 1, "y": 2 }, "scroll": { "x": 0, "y": 250_000 } })),
        Some(Locator::Point {
            x: 1.0,
            y: 250_002.0
        })
    );
    // A scroll the page shaped beyond reason, not a number, or one that leaves the document: no
    // CDP locator.
    for scroll in [
        json!({ "x": 0, "y": 1e12 }),
        json!({ "x": "0", "y": 0 }),
        json!({ "x": -50, "y": 0 }),
    ] {
        assert_eq!(
            locator(&json!({ "point": { "x": 10, "y": 20 }, "scroll": scroll })),
            None,
            "{scroll}"
        );
    }
    // A selector needs no point.
    assert_eq!(
        locator(&json!({ "selector": "#a", "scroll": { "x": "bad" } })),
        Some(Locator::Selector("#a".to_owned()))
    );
}
