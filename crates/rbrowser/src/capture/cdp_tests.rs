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

/// A view `width` CSS px wide whose breakpoint has no `dpr` (the screen's) and no height.
fn screen_target(width: u32) -> CaptureTarget {
    CaptureTarget {
        session: "S1".to_owned(),
        metrics: json!({
            "width": width, "height": 600, "deviceScaleFactor": 0.0, "mobile": false, "scale": 1.0
        }),
        height: 0,
        dpr: 0.0,
        hidden: false,
    }
}

/// A page `css_height` CSS px tall on a screen at `ratio`; every shot answers a 1 px wide PNG of
/// `rows` rows (a section with more rows than its plan is cut to it).
fn long_page(script: &Scripted, css_height: f64, ratio: f64, rows: u32) {
    script.answer(
        "Page.getLayoutMetrics",
        json!({
            "cssVisualViewport": { "pageX": 0.0, "pageY": 0.0 },
            "cssLayoutViewport": { "pageX": 0.0, "pageY": 0.0 },
            "cssContentSize": { "width": 100.0, "height": css_height },
            "contentSize": { "width": 100.0 * ratio, "height": css_height * ratio }
        }),
    );
    let image = rb_visual::Rgba::new(1, rows, [10, 20, 30, 255]).expect("small");
    let png = rb_visual::encode_png(&image).expect("encoded");
    script.answer(
        "Page.captureScreenshot",
        json!({ "data": base64::engine::general_purpose::STANDARD.encode(png) }),
    );
}

/// `(y, height)` of every shot's clip.
fn clips(script: &Scripted) -> Vec<(f64, f64)> {
    script
        .params("Page.captureScreenshot")
        .iter()
        .map(|shot| {
            let of = |key: &str| shot["clip"][key].as_f64().unwrap_or(-1.0);
            (of("y"), of("height"))
        })
        .collect()
}

#[tokio::test]
async fn a_long_page_without_dpr_is_taken_in_sections_at_the_screens_ratio() {
    // RB-099-11, the owner's find: no `dpr`, Windows at 150 %, a page of 40000 CSS px. One
    // shot of 16384 CSS px was 24576 device px and repeated its top from row 16384 on.
    let (script, router) = Scripted::connect();
    long_page(&script, 40_000.0, 1.5, 15_999);
    let restore = || Some(screen_target(1920).metrics);
    let png = capture(
        &router,
        &screen_target(1920),
        &Shot::FullPage,
        1.0,
        &restore,
    )
    .await
    .expect("captured");
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(
        overrides.len(),
        2,
        "one viewport for every section, then the restore"
    );
    assert_eq!(overrides[0]["height"], 10_666.0, "one section's height");
    assert_eq!(overrides[1], screen_target(1920).metrics);
    assert_eq!(
        clips(&script),
        [
            (0.0, 10_666.0),
            (10_666.0, 10_666.0),
            (21_332.0, 10_666.0),
            (31_998.0, 8002.0)
        ]
    );
    for shot in script.params("Page.captureScreenshot") {
        assert_eq!(shot["captureBeyondViewport"], true);
        assert_eq!(shot["clip"]["width"], 1920.0);
    }
    // 15999 rows three times, the last section cut to its 12003.
    assert_eq!(rb_visual::png_size(&png), Some((1, 60_000)));
    assert_eq!(rb_visual::truncation(&png), None);
}

#[tokio::test]
async fn a_page_beyond_the_image_is_cut_and_its_png_says_how_tall_it_was() {
    let (script, router) = Scripted::connect();
    long_page(&script, 100_000.0, 1.0, 16_000);
    let restore = || Some(screen_target(1).metrics);
    let png = capture(&router, &screen_target(1), &Shot::FullPage, 1.0, &restore)
        .await
        .expect("captured");
    assert_eq!(clips(&script).len(), 5);
    assert_eq!(rb_visual::png_size(&png), Some((1, 65_535)));
    assert_eq!(
        rb_visual::truncation(&png),
        Some(rb_visual::Truncated {
            page_height: 100_000,
            captured_height: 65_535,
        })
    );
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

/// A windowless view's surface that records each call with the number of CDP messages sent by
/// then, so a test sees where it came among them (RB-090-88).
struct Recording {
    script: Arc<Scripted>,
    calls: std::sync::Mutex<Vec<(String, usize)>>,
    fail_fit: bool,
}

impl Recording {
    fn new(script: &Arc<Scripted>) -> Self {
        Self {
            script: script.clone(),
            calls: std::sync::Mutex::new(Vec::new()),
            fail_fit: false,
        }
    }

    fn calls(&self) -> Vec<(String, usize)> {
        crate::cdp::lock(&self.calls).clone()
    }
}

#[async_trait::async_trait]
impl Surface for Recording {
    async fn fit(&self, height: f64) -> Result<(), EngineError> {
        let at = self.script.methods().len();
        crate::cdp::lock(&self.calls).push((format!("fit {height}"), at));
        if self.fail_fit {
            return Err(EngineError::Invalid("the view is gone".to_owned()));
        }
        Ok(())
    }

    async fn restore(&self) {
        let at = self.script.methods().len();
        crate::cdp::lock(&self.calls).push(("restore".to_owned(), at));
    }
}

#[tokio::test]
async fn full_page_grows_the_views_surface_before_the_shot_and_shrinks_it_after() {
    // A windowless render view paints only its own rectangle: the page height in the emulation
    // alone gave the top viewport repeated down the image (RB-090-88).
    let (script, router) = browser();
    let surface = Recording::new(&script);
    capture_on(
        &router,
        &target(852),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &surface,
    )
    .await
    .expect("captured");
    let methods = script.methods();
    let grown = methods
        .iter()
        .enumerate()
        .filter(|(_, method)| *method == "Emulation.setDeviceMetricsOverride")
        .nth(1)
        .map(|(at, _)| at + 1)
        .expect("the full-page override");
    let shot = methods
        .iter()
        .position(|method| method == "Page.captureScreenshot")
        .expect("a shot");
    let restored = methods
        .iter()
        .rposition(|method| method == "Emulation.setDeviceMetricsOverride")
        .expect("the restore");
    let calls = surface.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    // Grown to the page's height after its override and before the shot...
    assert_eq!(calls[0].0, "fit 4001");
    assert_eq!(calls[0].1, grown, "{methods:?}");
    assert!(calls[0].1 <= shot);
    // ...and back to its own size once the metrics are the host's again.
    assert_eq!(calls[1], ("restore".to_owned(), restored + 1));
}

#[tokio::test]
async fn a_viewport_or_element_shot_leaves_the_surface_alone() {
    let (script, router) = browser();
    script.answer("DOM.getDocument", json!({ "root": { "nodeId": 1 } }));
    script.answer("DOM.querySelector", json!({ "nodeId": 7 }));
    script.answer(
        "DOM.getBoxModel",
        json!({ "model": { "border": [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0] } }),
    );
    let surface = Recording::new(&script);
    let element = Shot::Element {
        selector: "#a".to_owned(),
        padding: 0,
    };
    for shot in [Shot::Viewport, element] {
        capture_on(&router, &target(852), &shot, 1.0, &host_metrics, &surface)
            .await
            .expect("captured");
    }
    assert!(surface.calls().is_empty());
}

#[tokio::test]
async fn a_failed_full_page_shot_still_shrinks_the_surface() {
    let (script, router) = browser();
    script.fail("Page.captureScreenshot", "boom");
    let surface = Recording::new(&script);
    let result = capture_on(
        &router,
        &target(852),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &surface,
    )
    .await;
    assert!(result.is_err());
    let calls = surface.calls();
    assert_eq!(calls.last().map(|call| call.0.as_str()), Some("restore"));
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.last(), Some(&target(852).metrics));
}

#[tokio::test]
async fn a_surface_that_cannot_grow_fails_the_shot_and_is_put_back() {
    let (script, router) = browser();
    let mut surface = Recording::new(&script);
    surface.fail_fit = true;
    let error = capture_on(
        &router,
        &target(852),
        &Shot::FullPage,
        1.0,
        &host_metrics,
        &surface,
    )
    .await
    .expect_err("no surface");
    assert!(error.to_string().contains("the view is gone"), "{error}");
    assert!(script.params("Page.captureScreenshot").is_empty());
    let calls: Vec<String> = surface.calls().into_iter().map(|call| call.0).collect();
    assert_eq!(calls, ["fit 4001", "restore"]);
    let overrides = script.params("Emulation.setDeviceMetricsOverride");
    assert_eq!(overrides.last(), Some(&target(852).metrics));
}

#[tokio::test]
async fn a_grid_views_capture_has_a_fixed_surface() {
    // A grid view is a window: it paints the whole emulated page, nothing is resized.
    let surface = FixedSurface;
    surface.fit(4001.0).await.expect("nothing to do");
    surface.restore().await;
}
