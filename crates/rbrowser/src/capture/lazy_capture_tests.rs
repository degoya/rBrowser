//! The lazy-loading pre-pass inside a capture over CDP (`crate::capture::cdp::capture_in`).

use std::sync::Arc;

use base64::Engine as _;
use serde_json::{Value, json};

use crate::capture::cdp::{CaptureTarget, FixedSurface, Shot, capture_in};
use crate::capture::scripted::Scripted;
use crate::cdp::CdpRouter;

fn target() -> CaptureTarget {
    CaptureTarget {
        session: "S1".to_owned(),
        metrics: json!({ "width": 393, "height": 852, "deviceScaleFactor": 1.0, "mobile": true }),
        height: 852,
        dpr: 1.0,
        hidden: false,
    }
}

fn host_metrics() -> Option<Value> {
    Some(target().metrics)
}

/// A page of 4000 CSS px scrolled to 120 whose every script answers the same position and
/// `pending` images still loading; shots are a 1 x 1 PNG.
fn browser(pending: u32) -> (Arc<Scripted>, Arc<CdpRouter>) {
    let (script, router) = Scripted::connect();
    script.answer(
        "Page.getFrameTree",
        json!({ "frameTree": { "frame": { "id": "F1" } } }),
    );
    script.answer(
        "Page.createIsolatedWorld",
        json!({ "executionContextId": 42 }),
    );
    script.answer(
        "Runtime.evaluate",
        json!({ "result": { "type": "object", "value": {
            "x": 0, "y": 120, "height": 4000, "viewport": 852, "pending": pending
        } } }),
    );
    script.answer(
        "Page.getLayoutMetrics",
        json!({
            "cssVisualViewport": { "pageX": 0.0, "pageY": 120.0 },
            "cssLayoutViewport": { "pageX": 0.0, "pageY": 120.0 },
            "cssContentSize": { "width": 393.0, "height": 4000.0 }
        }),
    );
    let image = rb_visual::Rgba::new(1, 1, [1, 2, 3, 255]).expect("small");
    let png = rb_visual::encode_png(&image).expect("encoded");
    script.answer(
        "Page.captureScreenshot",
        json!({ "data": base64::engine::general_purpose::STANDARD.encode(png) }),
    );
    (script, router)
}

fn expressions(script: &Scripted) -> Vec<String> {
    script
        .params("Runtime.evaluate")
        .iter()
        .filter_map(|params| params["expression"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn a_full_page_is_walked_before_it_is_measured_and_put_back_after_the_restore() {
    let (script, router) = browser(0);
    let png = capture_in(
        &router,
        &target(),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &FixedSurface,
        None,
    )
    .await
    .expect("captured");
    assert!(!rb_visual::images_pending(&png));
    let methods = script.methods();
    let first = |name: &str| methods.iter().position(|method| method == name);
    let last = |name: &str| methods.iter().rposition(|method| method == name);
    // In rBrowser's capture world, every script awaited.
    assert!(first("Page.createIsolatedWorld") < first("Runtime.evaluate"));
    for params in script.params("Runtime.evaluate") {
        assert_eq!(params["contextId"], 42);
        assert_eq!(params["awaitPromise"], true);
    }
    // Walked and waited for before the page is measured for the plan...
    let measured = first("Page.getLayoutMetrics");
    let walked = methods
        .iter()
        .enumerate()
        .filter(|(_, method)| *method == "Runtime.evaluate")
        .map(|(at, _)| at)
        .filter(|at| Some(*at) < measured)
        .count();
    // The read, the stops at 0 and 852 (the page stays at 120), the way back, the images.
    assert_eq!(walked, 5, "{methods:?}");
    // ...and scrolled back once the host's metrics are back.
    assert!(last("Emulation.setDeviceMetricsOverride") < last("Runtime.evaluate"));
    assert_eq!(methods.last().map(String::as_str), Some("Runtime.evaluate"));
    let scripts = expressions(&script);
    let back = scripts.last().expect("the put back");
    assert!(back.contains("top:120,"), "{back}");
    assert!(back.contains(r#""ms":1000}"#), "{back}");
    assert!(
        scripts[3].contains(r#""ms":10000}"#),
        "the marker over the shot"
    );
    assert!(scripts[4].contains("document.images"));
}

#[tokio::test]
async fn images_still_loading_mark_the_png_and_the_capture_goes_on() {
    let (script, router) = browser(3);
    let png = capture_in(
        &router,
        &target(),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &FixedSurface,
        None,
    )
    .await
    .expect("captured anyway");
    assert!(rb_visual::images_pending(&png));
    assert_eq!(rb_visual::png_size(&png), Some((1, 1)));
    assert_eq!(script.params("Page.captureScreenshot").len(), 1);
}

#[tokio::test]
async fn the_sync_agents_world_is_used_when_the_view_has_one() {
    let (script, router) = browser(0);
    capture_in(
        &router,
        &target(),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &FixedSurface,
        Some(("A1".to_owned(), 7)),
    )
    .await
    .expect("captured");
    assert!(script.params("Page.createIsolatedWorld").is_empty());
    let evaluations: Vec<Value> = script
        .sent()
        .into_iter()
        .filter(|message| message["method"] == "Runtime.evaluate")
        .collect();
    assert!(!evaluations.is_empty());
    for message in evaluations {
        assert_eq!(message["sessionId"], "A1");
        assert_eq!(message["params"]["contextId"], 7);
    }
}

#[tokio::test]
async fn a_stale_agent_world_falls_back_to_the_capture_world() {
    // Right after a navigation the agent's context is gone: its first script fails.
    let (script, router) = browser(0);
    script.fail_in(
        "Runtime.evaluate",
        "A1",
        "Cannot find context with specified id",
    );
    let png = capture_in(
        &router,
        &target(),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &FixedSurface,
        Some(("A1".to_owned(), 7)),
    )
    .await
    .expect("captured");
    assert!(
        !rb_visual::images_pending(&png),
        "walked in the capture world"
    );
    let evaluations: Vec<Value> = script
        .sent()
        .into_iter()
        .filter(|message| message["method"] == "Runtime.evaluate")
        .collect();
    assert_eq!(evaluations[0]["sessionId"], "A1", "the agent's world first");
    let fallback = &evaluations[1..];
    // The read, two stops, the way back, the images and the put back.
    assert_eq!(fallback.len(), 6, "{evaluations:?}");
    for message in fallback {
        assert_eq!(message["sessionId"], "S1");
        assert_eq!(message["params"]["contextId"], 42);
    }
    assert_eq!(script.params("Page.createIsolatedWorld").len(), 1);
}

#[tokio::test]
async fn a_prepass_that_cannot_run_marks_the_capture() {
    let (script, router) = browser(0);
    script.fail("Runtime.evaluate", "Execution context was destroyed");
    let png = capture_in(
        &router,
        &target(),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &FixedSurface,
        Some(("A1".to_owned(), 7)),
    )
    .await
    .expect("captured anyway");
    assert!(rb_visual::images_pending(&png));
    assert_eq!(script.params("Page.captureScreenshot").len(), 1);
    // Tried in both worlds, nothing to put back.
    assert_eq!(script.params("Runtime.evaluate").len(), 2);
    // Without any isolated world the capture is marked as well.
    let (script, router) = browser(0);
    script.fail("Page.createIsolatedWorld", "no frame");
    let png = capture_in(
        &router,
        &target(),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &FixedSurface,
        None,
    )
    .await
    .expect("captured anyway");
    assert!(rb_visual::images_pending(&png));
    assert!(script.params("Runtime.evaluate").is_empty());
}

#[tokio::test]
async fn a_viewport_or_element_shot_has_no_prepass() {
    let (script, router) = browser(0);
    script.answer("DOM.getDocument", json!({ "root": { "nodeId": 1 } }));
    script.answer("DOM.querySelector", json!({ "nodeId": 7 }));
    script.answer(
        "DOM.getBoxModel",
        json!({ "model": { "border": [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0] } }),
    );
    let element = Shot::Element {
        selector: "#a".to_owned(),
        padding: 0,
    };
    for shot in [Shot::Viewport, element] {
        capture_in(
            &router,
            &target(),
            &shot,
            1.0,
            &host_metrics,
            &FixedSurface,
            Some(("A1".to_owned(), 7)),
        )
        .await
        .expect("captured");
    }
    assert!(script.params("Runtime.evaluate").is_empty());
    assert!(script.params("Page.getFrameTree").is_empty());
}
