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

#[tokio::test]
async fn the_node_comes_from_the_page_at_the_scaled_point() {
    let (script, router) = Scripted::connect();
    script.answer(
        "DOM.getNodeForLocation",
        json!({ "backendNodeId": 42, "frameId": "F1" }),
    );
    let node = node_at(&router, "S1", Point { x: 50.0, y: 20.0 }, 0.5)
        .await
        .expect("a node");
    assert_eq!(node, 42);
    let sent = &script.sent()[0];
    assert_eq!(sent["sessionId"], "S1");
    assert_eq!(
        sent["params"],
        json!({ "x": 100, "y": 40, "includeUserAgentShadowDOM": false })
    );
}

#[tokio::test]
async fn no_node_at_the_point_is_an_error() {
    let (script, router) = Scripted::connect();
    script.fail("DOM.getNodeForLocation", "No node at given location");
    let error = node_at(&router, "S1", Point { x: 1.0, y: 1.0 }, 1.0)
        .await
        .expect_err("fails");
    assert!(error.to_string().contains("No node"), "{error}");
    let (script, router) = Scripted::connect();
    script.answer("DOM.getNodeForLocation", json!({ "frameId": "F1" }));
    let error = node_at(&router, "S1", Point { x: 1.0, y: 1.0 }, 1.0)
        .await
        .expect_err("no id");
    assert!(error.to_string().contains("no element"), "{error}");
}
