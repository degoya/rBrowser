use serde_json::json;

use super::*;
use crate::capture::scripted::Scripted;

const PNG: &[u8] = b"\x89PNG fake";

fn target(height: u32) -> CaptureTarget {
    CaptureTarget {
        session: "S1".to_owned(),
        metrics: json!({
            "width": 393, "height": 600, "deviceScaleFactor": 3.0, "mobile": true, "scale": 0.5,
            "screenOrientation": { "type": "portraitPrimary", "angle": 0 }
        }),
        height,
        dpr: 3.0,
        hidden: false,
    }
}

fn browser() -> (Arc<Scripted>, Arc<CdpRouter>) {
    let (script, router) = Scripted::connect();
    script.answer(
        "Page.getLayoutMetrics",
        json!({
            "cssVisualViewport": { "pageX": 0.0, "pageY": 120.0 },
            "cssLayoutViewport": { "pageX": 0.0, "pageY": 120.0 },
            "cssContentSize": { "width": 393.0, "height": 4000.5 }
        }),
    );
    script.answer(
        "Page.captureScreenshot",
        json!({ "data": base64::engine::general_purpose::STANDARD.encode(PNG) }),
    );
    (script, router)
}

fn host_metrics() -> Option<Value> {
    Some(target(852).metrics)
}

#[tokio::test]
async fn viewport_renders_the_breakpoint_height_and_restores_the_host_metrics() {
    let (script, router) = browser();
    let png = capture(&router, &target(852), &Shot::Viewport, 1.0, &host_metrics)
        .await
        .expect("captured");
    assert_eq!(png, PNG);
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.len(), 2);
    assert_eq!(overrides[0]["height"], 852.0);
    assert_eq!(overrides[0]["width"], 393);
    assert_eq!(overrides[0]["screenOrientation"]["type"], "portraitPrimary");
    assert_eq!(overrides[1], target(852).metrics, "restored exactly");
    let shot = &script.params("Page.captureScreenshot")[0];
    assert_eq!(
        shot["clip"],
        json!({ "x": 0.0, "y": 120.0, "width": 393.0, "height": 852.0, "scale": 1.0 })
    );
    assert_eq!(shot["captureBeyondViewport"], false);
    assert_eq!(shot["format"], "png");
}

#[tokio::test]
async fn viewport_at_the_cell_height_sends_no_override() {
    let (script, router) = browser();
    capture(&router, &target(600), &Shot::Viewport, 1.0, &host_metrics)
        .await
        .expect("captured");
    assert!(
        script
            .params("Emulation.setDeviceMetricsOverride")
            .is_empty()
    );
}

#[tokio::test]
async fn full_page_grows_to_the_content_and_restores_afterwards() {
    let (script, router) = browser();
    capture(&router, &target(852), &Shot::FullPage, 1.0, &host_metrics)
        .await
        .expect("captured");
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.len(), 3);
    assert_eq!(overrides[1]["height"], 4001.0);
    assert_eq!(overrides[2], target(852).metrics);
    let methods = script.methods();
    let shot_at = methods.iter().position(|m| m == "Page.captureScreenshot");
    let last_override = methods
        .iter()
        .rposition(|m| m == "Emulation.setDeviceMetricsOverride");
    assert!(
        shot_at < last_override,
        "restored after the capture: {methods:?}"
    );
    let shot = &script.params("Page.captureScreenshot")[0];
    assert_eq!(shot["clip"]["height"], 4001.0);
    assert_eq!(shot["clip"]["y"], 0.0);
    assert_eq!(shot["captureBeyondViewport"], true);
}

#[tokio::test]
async fn a_failed_capture_still_restores_the_metrics() {
    let (script, router) = browser();
    script.fail("Page.captureScreenshot", "boom");
    let result = capture(&router, &target(852), &Shot::FullPage, 1.0, &host_metrics).await;
    assert!(result.is_err());
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.last(), Some(&target(852).metrics));
}

#[tokio::test]
async fn an_element_is_its_border_box_plus_padding_in_page_coordinates() {
    let (script, router) = browser();
    script.answer("DOM.getDocument", json!({ "root": { "nodeId": 1 } }));
    script.answer("DOM.querySelector", json!({ "nodeId": 7 }));
    script.answer(
        "DOM.getBoxModel",
        json!({ "model": { "border": [10.0, 900.0, 110.0, 900.0, 110.0, 950.0, 10.0, 950.0] } }),
    );
    let shot = Shot::Element {
        selector: "#hero".to_owned(),
        padding: 16,
    };
    capture(&router, &target(600), &shot, 1.0, &host_metrics)
        .await
        .expect("captured");
    assert_eq!(script.params("DOM.querySelector")[0]["selector"], "#hero");
    let clip = &script.params("Page.captureScreenshot")[0]["clip"];
    // 100 x 50 plus 16 on each side; y moved by the scroll offset 120.
    assert_eq!(clip["width"], 132.0);
    assert_eq!(clip["height"], 82.0);
    assert_eq!(clip["x"], 0.0);
    assert_eq!(clip["y"], 1004.0);
    assert_eq!(
        script.params("Page.captureScreenshot")[0]["captureBeyondViewport"],
        true
    );
}

#[tokio::test]
async fn no_matching_element_is_an_error() {
    let (script, router) = browser();
    script.answer("DOM.getDocument", json!({ "root": { "nodeId": 1 } }));
    script.answer("DOM.querySelector", json!({ "nodeId": 0 }));
    let shot = Shot::Element {
        selector: ".missing".to_owned(),
        padding: 0,
    };
    let error = capture(&router, &target(600), &shot, 1.0, &host_metrics)
        .await
        .expect_err("nothing matches");
    assert!(error.to_string().contains(".missing"));
    assert!(script.params("Page.captureScreenshot").is_empty());
}

#[test]
fn element_clip_keeps_its_size_at_the_page_edge() {
    let clip = element_clip(
        &[2.0, 3.0, 52.0, 3.0, 52.0, 23.0, 2.0, 23.0],
        16,
        (0.0, 0.0),
        1.0,
    )
    .expect("a box");
    assert_eq!(
        (clip.x, clip.y, clip.width, clip.height),
        (0.0, 0.0, 82.0, 52.0)
    );
    assert!(element_clip(&[1.0; 6], 0, (0.0, 0.0), 1.0).is_none());
}

#[test]
fn full_page_height_is_capped_in_device_pixels() {
    assert_eq!(full_page_height(4000.2, 852.0, 3.0, 1.0), 4001.0);
    assert_eq!(full_page_height(100.0, 852.0, 3.0, 1.0), 852.0);
    assert_eq!(full_page_height(100_000.0, 852.0, 2.0, 1.0), 8192.0);
    assert_eq!(full_page_height(100_000.0, 852.0, 0.0, 1.0), 16_384.0);
}

#[tokio::test]
async fn a_failed_full_page_override_is_still_restored() {
    let (script, router) = browser();
    script.fail("Emulation.setDeviceMetricsOverride", "boom");
    // At the cell height: only the full-page override changes the metrics.
    let result = capture(&router, &target(600), &Shot::FullPage, 1.0, &host_metrics).await;
    assert!(result.is_err());
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.len(), 2, "the override, then the restore");
    assert_eq!(overrides[1], target(852).metrics);
    assert!(script.params("Page.captureScreenshot").is_empty());
}

#[tokio::test]
async fn a_failed_viewport_override_is_still_restored() {
    let (script, router) = browser();
    script.fail("Emulation.setDeviceMetricsOverride", "boom");
    let result = capture(&router, &target(852), &Shot::Viewport, 1.0, &host_metrics).await;
    assert!(result.is_err());
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.len(), 2, "the override, then the restore");
    assert_eq!(overrides[1], target(852).metrics);
}
