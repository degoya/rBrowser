//! Tests of `cdp_policy` for the page-level methods the newer first-party plugins send (color-picker,
//! pwa-check, simulators, coverage); the helpers live in the parent test module.

use serde_json::json;

use super::super::{Pending, allowed, check, domain_allowed};
use super::refused_reason;

#[test]
fn a_screenshot_reads_the_views_own_pixels_only() {
    // color-picker: a few pixels around the cursor; the view's own page, nothing beyond it.
    let params = json!({ "format": "png", "clip": { "x": 10, "y": 20, "width": 11, "height": 11, "scale": 1 } });
    assert_eq!(
        check("color-picker", "Page.captureScreenshot", &params).expect("clip"),
        Pending::None
    );
    let answer = check("color-picker", "Page.captureScreenshot", &json!([]));
    assert_eq!(refused_reason(answer), "params");
}

#[test]
fn a_screenshot_passes_as_color_picker_sends_it() {
    // plugins/color-picker/src/sample.rs: the loupe's clip at scale 1, inside the viewport.
    let params = json!({
        "format": "png",
        "clip": { "x": 1234.5, "y": 20000.25, "width": 15, "height": 15, "scale": 1 },
        "captureBeyondViewport": false,
        "fromSurface": true,
    });
    assert_eq!(
        check("color-picker", "Page.captureScreenshot", &params).expect("loupe"),
        Pending::None
    );
    // The largest area the rule lets through: 1 Mpx at scale 1, or a quarter of it at scale 2.
    for clip in [
        json!({ "x": 0, "y": 0, "width": 1024, "height": 1024, "scale": 1 }),
        json!({ "x": 0, "y": 0, "width": 512, "height": 512, "scale": 2 }),
        json!({ "x": 0, "y": 0, "width": 10, "height": 10, "scale": 4 }),
        json!({ "x": 0, "y": 0, "width": 10, "height": 10 }),
    ] {
        let params = json!({ "clip": clip });
        assert_eq!(
            check("color-picker", "Page.captureScreenshot", &params).expect("in the cap"),
            Pending::None,
            "{params}"
        );
    }
}

#[test]
fn a_screenshot_of_what_the_window_shows_needs_no_clip() {
    // RB-080-64: color-picker reads the view as its window shows it - the compositor's picture,
    // never rendered anew (a clip, or the surface under rBrowser's emulation, made the view
    // flicker) and never larger than the view on the screen.
    // Only with the manifest's own permission (review S2): the policy names it, the context
    // checks the grant.
    let params = json!({ "format": "png", "captureBeyondViewport": false, "fromSurface": false });
    assert_eq!(
        check("color-picker", "Page.captureScreenshot", &params).expect("window"),
        Pending::WindowImage
    );
    assert_eq!(
        check(
            "color-picker",
            "Page.captureScreenshot",
            &json!({ "fromSurface": false })
        )
        .expect("window"),
        Pending::WindowImage
    );
    // The surface without a clip stays refused: Chromium renders it anew at the emulated size
    // and DPR (Charge 8 review M1); a clip of the window's image is not offered.
    let clip = json!({ "x": 0, "y": 0, "width": 10, "height": 10, "scale": 1 });
    for params in [
        json!({ "format": "png", "fromSurface": true }),
        json!({ "fromSurface": false, "captureBeyondViewport": true }),
        json!({ "fromSurface": false, "clip": clip }),
        json!({ "fromSurface": "false" }),
    ] {
        let answer = check("color-picker", "Page.captureScreenshot", &params);
        assert_eq!(refused_reason(answer), "clip", "{params}");
    }
}

#[test]
fn a_screenshot_is_refused_beyond_a_small_clip_of_the_viewport() {
    // Charge 8 review M1: a whole-page shot at 16384 px and scale 4 would push hundreds of MiB
    // of PNG through the view's pipe before the 4 MiB answer cap refuses it.
    let clip = |width: f64, height: f64, scale: f64| json!({ "x": 0, "y": 0, "width": width, "height": height, "scale": scale });
    for params in [
        json!({ "format": "png" }),
        json!({ "clip": null }),
        json!({ "clip": "0,0,10,10" }),
        json!({ "clip": { "x": 0, "y": 0, "width": 10 } }),
        json!({ "clip": { "x": 0, "y": 0, "width": "10", "height": 10, "scale": 1 } }),
        json!({ "clip": clip(0.0, 10.0, 1.0) }),
        json!({ "clip": clip(-5.0, 10.0, 1.0) }),
        json!({ "clip": clip(10.0, 10.0, 0.0) }),
        json!({ "clip": clip(10.0, 10.0, 4.5) }),
        json!({ "clip": clip(1025.0, 1024.0, 1.0) }),
        json!({ "clip": clip(513.0, 512.0, 2.0) }),
        json!({ "clip": clip(16384.0, 16384.0, 4.0) }),
        json!({ "clip": clip(10.0, 10.0, 1.0), "captureBeyondViewport": true }),
        json!({ "clip": clip(10.0, 10.0, 1.0), "captureBeyondViewport": "false" }),
        json!({ "clip": clip(10.0, 10.0, 1.0), "fromSurface": false }),
        json!({ "clip": clip(10.0, 10.0, 1.0), "fromSurface": 0 }),
    ] {
        let answer = check("color-picker", "Page.captureScreenshot", &params);
        assert_eq!(refused_reason(answer), "clip", "{params}");
    }
}

#[test]
fn the_installability_verdict_is_read_without_parameters() {
    // pwa-check: Chromium's installability errors of the view's own page; it reads, changes
    // nothing and reaches no other page.
    assert_eq!(
        check("pwa-check", "Page.getInstallabilityErrors", &json!({})).expect("read"),
        Pending::None
    );
    let answer = check("pwa-check", "Page.getInstallabilityErrors", &json!(null));
    assert_eq!(refused_reason(answer), "params");
    for method in [
        "Page.getAppManifest",
        "Page.getManifestIcons",
        "ServiceWorker.enable",
    ] {
        assert!(!allowed(method), "{method}");
    }
}

#[test]
fn a_vision_deficiency_is_emulated_in_the_views_page_alone() {
    // simulators (RB-070-08): one enum parameter that changes how this page renders, nothing else.
    for kind in ["deuteranopia", "blurredVision", "none"] {
        let answer = check(
            "simulators",
            "Emulation.setEmulatedVisionDeficiency",
            &json!({ "type": kind }),
        );
        assert_eq!(answer.expect("allowed"), Pending::None, "{kind}");
    }
}

#[test]
fn coverage_tracks_rule_usage_and_precise_coverage_of_the_views_page_alone() {
    // coverage (RB-080-07): CSS rule usage and V8 precise coverage of the view's own page, the
    // sheets' text by the ids the usage names; no parameter reaches beyond the page.
    for (method, params) in [
        ("CSS.startRuleUsageTracking", json!({})),
        ("CSS.takeCoverageDelta", json!({})),
        ("CSS.stopRuleUsageTracking", json!({})),
        (
            "CSS.getStyleSheetText",
            json!({ "styleSheetId": "style-sheet-1-2" }),
        ),
        ("Profiler.enable", json!({})),
        (
            "Profiler.startPreciseCoverage",
            json!({ "callCount": true, "detailed": true }),
        ),
        ("Profiler.takePreciseCoverage", json!({})),
        ("Profiler.stopPreciseCoverage", json!({})),
    ] {
        assert_eq!(
            check("coverage", method, &params).expect(method),
            Pending::None
        );
    }
    assert!(domain_allowed("Profiler"));
    assert!(!domain_allowed("Debugger"));
}

#[test]
fn a_resource_is_read_from_a_frame_by_a_web_url_only() {
    // coverage: the text of a style sheet the page linked, by its URL.
    let params = json!({ "frameId": "F1", "url": "https://cdn.example/site.css" });
    assert_eq!(
        check("coverage", "Page.getResourceContent", &params).expect("web URL"),
        Pending::None
    );
    for params in [
        json!({ "frameId": "F1", "url": "file:///C:/Users/x/secret.txt" }),
        json!({ "frameId": "F1", "url": "chrome://settings" }),
        json!({ "frameId": "F1" }),
        json!({ "frameId": "F1", "url": 5 }),
    ] {
        let answer = check("coverage", "Page.getResourceContent", &params);
        assert_eq!(refused_reason(answer), "url", "{params}");
    }
    for params in [
        json!({ "url": "https://cdn.example/site.css" }),
        json!({ "frameId": "", "url": "https://cdn.example/site.css" }),
        json!({ "frameId": "x".repeat(200), "url": "https://cdn.example/site.css" }),
    ] {
        let answer = check("coverage", "Page.getResourceContent", &params);
        assert_eq!(refused_reason(answer), "params", "{params}");
    }
}
