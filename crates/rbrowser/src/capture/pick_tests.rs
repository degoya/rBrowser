use std::time::Duration;

use serde_json::{Value, json};

use super::*;
use crate::capture::scripted::Scripted;

#[tokio::test]
async fn a_click_gives_the_node_and_ends_the_mode() {
    let (script, router) = Scripted::connect();
    let task = tokio::spawn({
        let router = router.clone();
        async move { pick(&router, "S1", Duration::from_secs(5)).await }
    });
    while script.params("Overlay.setInspectMode").is_empty() {
        tokio::task::yield_now().await;
    }
    // Another view's click is not this one's.
    script.emit(json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 7 }, "sessionId": "S2" }));
    script.emit(json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 42 }, "sessionId": "S1" }));
    let node = task.await.expect("joined").expect("picked");
    assert_eq!(node, 42);
    let modes = script.params("Overlay.setInspectMode");
    assert_eq!(modes[0]["mode"], "searchForNode");
    assert_eq!(modes[1]["mode"], "none");
    assert_eq!(
        script.methods().last().map(String::as_str),
        Some("Overlay.disable")
    );
}

#[tokio::test]
async fn no_click_times_out_and_still_ends_the_mode() {
    let (script, router) = Scripted::connect();
    let error = pick(&router, "S1", Duration::from_millis(20))
        .await
        .expect_err("timed out");
    assert!(error.to_string().contains("in time"));
    assert_eq!(script.params("Overlay.setInspectMode")[1]["mode"], "none");
}

/// RB-090-54: a click on one of rBrowser's own layers - a ruler's handle, a plugin's host under
/// `<rbrowser-layers>` - is no pick. The node is resolved in rBrowser's isolated world of the main
/// frame and [`OURS`] asks whether it or a host above it is rBrowser's; the mode is armed again,
/// and the next click on the page is the pick.
#[tokio::test]
async fn a_click_on_an_rbrowser_layer_is_passed_over() {
    let (script, router) = Scripted::connect();
    script.answer(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F1" } } }),
    );
    script.answer(
        "Page.createIsolatedWorld",
        json!({ "executionContextId": 7 }),
    );
    script.answer(
        "DOM.resolveNode",
        json!({ "object": { "type": "object", "subtype": "node", "objectId": "O1" } }),
    );
    script.answer(
        "Runtime.callFunctionOn",
        json!({ "result": { "type": "boolean", "value": true } }),
    );
    let task = tokio::spawn({
        let router = router.clone();
        async move { pick(&router, "S1", Duration::from_secs(5)).await }
    });
    while script.params("Overlay.setInspectMode").is_empty() {
        tokio::task::yield_now().await;
    }
    script.emit(json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 9 }, "sessionId": "S1" }));
    while script.params("Overlay.setInspectMode").len() < 2 {
        tokio::task::yield_now().await;
    }
    script.answer(
        "Runtime.callFunctionOn",
        json!({ "result": { "type": "boolean", "value": false } }),
    );
    script.emit(json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 42 }, "sessionId": "S1" }));
    let node = task.await.expect("joined").expect("picked");
    assert_eq!(node, 42);
    assert!(
        script.sent().iter().all(|sent| sent["sessionId"] == "S1"),
        "the page's session"
    );
    let resolved = script.params("DOM.resolveNode");
    assert_eq!(
        resolved[0],
        json!({ "backendNodeId": 9, "executionContextId": 7, "objectGroup": PICK_GROUP })
    );
    assert_eq!(resolved[1]["backendNodeId"], 42);
    assert_eq!(
        script.params("Page.createIsolatedWorld")[0],
        json!({ "frameId": "F1", "worldName": PICK_WORLD })
    );
    let call = &script.params("Runtime.callFunctionOn")[0];
    assert_eq!(call["objectId"], "O1");
    assert_eq!(call["functionDeclaration"], OURS);
    assert_eq!(call["returnByValue"], true);
    assert_eq!(script.params("Runtime.releaseObjectGroup").len(), 2);
    // Armed again after the layer's click, ended after the page's.
    let modes: Vec<Value> = script
        .params("Overlay.setInspectMode")
        .iter()
        .map(|mode| mode["mode"].clone())
        .collect();
    assert_eq!(modes, ["searchForNode", "searchForNode", "none"]);
}

/// A node the check cannot resolve in the main frame's world (in a child frame, gone) is not
/// rBrowser's: the layers live in the top document. The pick stands.
#[tokio::test]
async fn a_node_the_check_cannot_resolve_is_the_page_s() {
    let (script, router) = Scripted::connect();
    script.answer(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F1" } } }),
    );
    script.answer(
        "Page.createIsolatedWorld",
        json!({ "executionContextId": 7 }),
    );
    script.fail(
        "DOM.resolveNode",
        "Node with given id does not belong to the document",
    );
    let task = tokio::spawn({
        let router = router.clone();
        async move { pick(&router, "S1", Duration::from_secs(5)).await }
    });
    while script.params("Overlay.setInspectMode").is_empty() {
        tokio::task::yield_now().await;
    }
    script.emit(json!({ "method": "Overlay.inspectNodeRequested", "params": { "backendNodeId": 5 }, "sessionId": "S1" }));
    assert_eq!(task.await.expect("joined").expect("picked"), 5);
}

#[test]
fn the_check_walks_up_through_shadow_roots_to_rbrowser_s_hosts() {
    assert!(OURS.trim_start().starts_with("function ()"));
    assert!(OURS.contains("'rbrowser-'"));
    assert!(OURS.contains("data-rbrowser-plugin"));
    assert!(OURS.contains(".host"));
}
