use serde_json::json;

use super::*;
use crate::capture::scripted::Scripted;

#[test]
fn a_point_of_the_widget_becomes_css_px_of_the_page_through_the_scale() {
    let point = Point { x: 100.0, y: 60.0 };
    assert_eq!(point.in_page(0.5), (200, 120));
    assert_eq!(point.in_page(1.0), (100, 60));
    assert_eq!(point.in_page(0.0), (100, 60), "an unusable scale is 1");
    assert_eq!(point.in_page(f64::NAN), (100, 60));
    assert_eq!(Point { x: 10.4, y: 10.6 }.in_page(1.0), (10, 11));
}

#[test]
fn the_queue_keeps_the_newest_request_per_view_until_taken() {
    let queue = InspectQueue::default();
    assert_eq!(queue.take(ViewId(1)), None);
    queue.request(ViewId(1), 7);
    queue.request(ViewId(2), 8);
    queue.request(ViewId(1), 9);
    assert_eq!(queue.pending(ViewId(1)), Some(9), "the newest replaces");
    assert_eq!(queue.take(ViewId(1)), Some(9));
    assert_eq!(queue.take(ViewId(1)), None, "taken once");
    queue.clear(ViewId(2));
    assert_eq!(queue.pending(ViewId(2)), None);
}

/// A connection peeks, delivers, then takes: a newer request that came meanwhile stays.
#[test]
fn take_if_removes_only_the_node_that_was_delivered() {
    let queue = InspectQueue::default();
    queue.request(ViewId(1), 7);
    assert!(!queue.take_if(ViewId(1), 8), "not the delivered node");
    assert_eq!(queue.pending(ViewId(1)), Some(7));
    queue.request(ViewId(1), 9);
    assert!(!queue.take_if(ViewId(1), 7), "replaced before the take");
    assert_eq!(queue.pending(ViewId(1)), Some(9));
    assert!(queue.take_if(ViewId(1), 9));
    assert_eq!(queue.pending(ViewId(1)), None);
    assert!(!queue.take_if(ViewId(1), 9));
}

#[tokio::test]
async fn a_request_wakes_a_waiting_connection() {
    let queue = Arc::new(InspectQueue::default());
    let waiter = tokio::spawn({
        let queue = queue.clone();
        async move {
            queue.changed().await;
            queue.take(ViewId(3))
        }
    });
    queue.request(ViewId(3), 11);
    assert_eq!(waiter.await.expect("joined"), Some(11));
}

/// The page as Chromium answers the resolution: the main frame `F1`, its isolated world 7, the
/// element the hit test found (`O1`) and its backend node `node`.
fn page_with(script: &Scripted, node: i64) {
    script.answer(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F1" } } }),
    );
    script.answer(
        "Page.createIsolatedWorld",
        json!({ "executionContextId": 7 }),
    );
    script.answer(
        "Runtime.callFunctionOn",
        json!({ "result": { "type": "object", "subtype": "node", "objectId": "O1" } }),
    );
    script.answer(
        "DOM.describeNode",
        json!({ "node": { "nodeId": 0, "backendNodeId": node, "nodeName": "MAIN" } }),
    );
}

/// RB-090-51: the element comes from the hit test of [`ELEMENT_AT`] in rBrowser's own world -
/// viewport px, the scroll left out (`DOM.getNodeForLocation` took document px), rBrowser's
/// hosts passed over - then its backend node; the objects are released.
#[tokio::test]
async fn the_node_comes_from_the_page_at_the_scaled_point() {
    let (script, router) = Scripted::connect();
    page_with(&script, 42);
    let node = node_at(&router, "S1", Point { x: 50.0, y: 20.0 }, 0.5)
        .await
        .expect("a node");
    assert_eq!(node, 42);
    assert!(
        script.sent().iter().all(|sent| sent["sessionId"] == "S1"),
        "the page's session"
    );
    assert_eq!(
        script.methods(),
        [
            "Page.getFrameTree",
            "Page.createIsolatedWorld",
            "Runtime.callFunctionOn",
            "DOM.describeNode",
            "Runtime.releaseObjectGroup",
        ]
    );
    assert_eq!(
        script.params("Page.createIsolatedWorld")[0],
        json!({ "frameId": "F1", "worldName": INSPECT_WORLD })
    );
    let call = &script.params("Runtime.callFunctionOn")[0];
    assert_eq!(call["executionContextId"], 7);
    assert_eq!(
        call["arguments"],
        json!([{ "value": 100 }, { "value": 40 }])
    );
    assert_eq!(call["returnByValue"], false);
    assert_eq!(call["objectGroup"], OBJECT_GROUP);
    assert_eq!(call["functionDeclaration"], ELEMENT_AT);
    assert_eq!(
        script.params("DOM.describeNode")[0],
        json!({ "objectId": "O1" })
    );
    assert_eq!(
        script.params("Runtime.releaseObjectGroup")[0],
        json!({ "objectGroup": OBJECT_GROUP })
    );
}

/// The hit test passes over every rBrowser host: the plugins' `rbrowser-*` elements and the ones
/// marked `data-rbrowser-plugin` (its behaviour: `agent/src/inspect-point.test.ts`).
#[test]
fn the_hit_test_passes_over_rbrowsers_own_hosts() {
    assert!(ELEMENT_AT.trim_start().starts_with("function (x, y)"));
    assert!(ELEMENT_AT.contains("'rbrowser-'"));
    assert!(ELEMENT_AT.contains("data-rbrowser-plugin"));
    assert!(ELEMENT_AT.contains("elementsFromPoint"));
}

#[tokio::test]
async fn no_node_at_the_point_is_an_error() {
    // Only rBrowser (or nothing) under the point: the script answers null.
    let (script, router) = Scripted::connect();
    page_with(&script, 42);
    script.answer(
        "Runtime.callFunctionOn",
        json!({ "result": { "type": "object", "subtype": "null", "value": null } }),
    );
    let error = node_at(&router, "S1", Point { x: 1.0, y: 1.0 }, 1.0)
        .await
        .expect_err("no element");
    assert!(error.to_string().contains("no element"), "{error}");
    assert!(script.params("DOM.describeNode").is_empty());
    assert_eq!(
        script.methods().last().map(String::as_str),
        Some("Runtime.releaseObjectGroup")
    );
    // The script threw.
    let (script, router) = Scripted::connect();
    page_with(&script, 42);
    script.answer(
        "Runtime.callFunctionOn",
        json!({ "result": { "type": "object" }, "exceptionDetails": { "text": "Uncaught" } }),
    );
    let error = node_at(&router, "S1", Point { x: 1.0, y: 1.0 }, 1.0)
        .await
        .expect_err("threw");
    assert!(error.to_string().contains("threw"), "{error}");
    // The page answers no world.
    let (script, router) = Scripted::connect();
    page_with(&script, 42);
    script.fail("Page.createIsolatedWorld", "No frame for given id found");
    let error = node_at(&router, "S1", Point { x: 1.0, y: 1.0 }, 1.0)
        .await
        .expect_err("no world");
    assert!(error.to_string().contains("No frame"), "{error}");
    assert!(script.params("Runtime.callFunctionOn").is_empty());
    // A node without a backend id.
    let (script, router) = Scripted::connect();
    page_with(&script, 42);
    script.answer("DOM.describeNode", json!({ "node": { "nodeId": 0 } }));
    let error = node_at(&router, "S1", Point { x: 1.0, y: 1.0 }, 1.0)
        .await
        .expect_err("no id");
    assert!(error.to_string().contains("no element"), "{error}");
}
