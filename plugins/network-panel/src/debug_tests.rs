use serde_json::json;

use super::*;
use crate::fake_host::FakeHost;

fn with_state(mut args: Value) -> Value {
    args["$network"] = json!({ "session": "s1", "trusted": true,
        "held": [{ "id": "h1", "url": "https://a.test/?sid=***", "view": 1 }],
        "answers": { "a9": { "done": true } }, "composed": [],
        "breakpoints": { "enabled": true, "active": 1 } });
    args
}

#[test]
fn breakpoints_are_added_switched_and_removed() {
    let mut host = FakeHost::default();
    let added = breakpoint_add(
        &mut host,
        &with_state(json!({ "urlPattern": "*/api/*", "method": "post", "stage": "response", "views": [1, 2] })),
    )
    .expect("added");
    assert_eq!(added["id"], "bp-1");
    assert_eq!(added["breakpoint"]["method"], "POST");
    assert_eq!(added["breakpoint"]["stage"], "response");
    assert_eq!(added["breakpoint"]["views"], json!([1, 2]));
    assert_eq!(added["$post"][0]["message"]["breakpointsChanged"], true);
    let again = breakpoint_add(
        &mut host,
        &with_state(json!({ "id": "bp-1", "urlPattern": "*" })),
    )
    .expect("replaced");
    assert_eq!(again["breakpoint"]["stage"], "request");
    let shown = state(&mut host, &with_state(json!({})));
    assert_eq!(shown["breakpoints"].as_array().map(Vec::len), Some(1));
    assert_eq!(shown["breakpoints"][0]["urlPattern"], "*");
    assert_eq!(shown["held"][0]["id"], "h1");
    assert_eq!(shown["breakpointsEnabled"], true);
    assert_eq!(shown["timeoutMs"], 30000);
    breakpoint_enable(
        &mut host,
        &with_state(json!({ "id": "bp-1", "enabled": false })),
    )
    .expect("off");
    assert_eq!(
        store::read(&mut host, "s1").breakpoints[0]["enabled"],
        false
    );
    let off = breakpoints_toggle(&mut host, &with_state(json!({}))).expect("toggled");
    assert_eq!(off["breakpointsEnabled"], false);
    assert_eq!(
        state(&mut host, &with_state(json!({})))["breakpointsEnabled"],
        false
    );
    breakpoint_remove(&mut host, &with_state(json!({ "id": "bp-1" }))).expect("removed");
    assert!(store::read(&mut host, "s1").breakpoints.is_empty());
    assert!(breakpoint_remove(&mut host, &with_state(json!({ "id": "bp-1" }))).is_err());
    for bad in [
        json!({}),
        json!({ "urlPattern": "*", "stage": "sideways" }),
        json!({ "urlPattern": "*", "views": ["a"] }),
        json!({ "urlPattern": "*", "method": "G-T" }),
    ] {
        assert!(
            breakpoint_add(&mut host, &with_state(bad.clone())).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_asks_queue_with_their_ids_and_shapes() {
    let mut host = FakeHost::default();
    let first = continue_held(
        &mut host,
        &with_state(json!({ "id": "h1", "allViews": true, "edits": { "status": 500 } })),
    )
    .expect("queued");
    assert_eq!(first["ask"], "a1");
    abort(&mut host, &with_state(json!({ "id": "h2" }))).expect("queued");
    compose(
        &mut host,
        &with_state(json!({ "view": 1, "times": 3, "request": { "method": "GET", "url": "https://a.test/" } })),
    )
    .expect("queued");
    repeat(
        &mut host,
        &with_state(json!({ "view": 2, "requestId": "2.7" })),
    )
    .expect("queued");
    let asks = store::read(&mut host, "s1").asks;
    assert_eq!(
        asks,
        vec![
            json!({ "id": "a1", "kind": "continue", "held": "h1", "allViews": true, "edits": { "status": 500 } }),
            json!({ "id": "a2", "kind": "abort", "held": "h2", "allViews": false }),
            json!({ "id": "a3", "kind": "compose", "view": 1, "times": 3,
                    "request": { "method": "GET", "url": "https://a.test/" } }),
            json!({ "id": "a4", "kind": "repeat", "view": 2, "requestId": "2.7", "edits": {}, "times": 1 }),
        ]
    );
    assert_eq!(state(&mut host, &with_state(json!({})))["pending"], 4);
    for bad in [
        continue_held(&mut host, &with_state(json!({ "id": "h1", "edits": 5 }))),
        compose(&mut host, &with_state(json!({ "request": "GET /" }))),
        compose(
            &mut host,
            &with_state(json!({ "request": { "url": "https://a.test/" }, "times": 99 })),
        ),
        repeat(&mut host, &with_state(json!({ "view": 1 }))),
    ] {
        assert!(bad.is_err());
    }
}
