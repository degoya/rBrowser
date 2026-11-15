use std::time::Duration;

use serde_json::json;

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
