use base64::Engine as _;
use rb_visual::{CleanOverrides, CleanSettings};
use serde_json::json;

use super::*;
use crate::capture::cdp::{CaptureTarget, Shot, capture};
use crate::capture::scripted::Scripted;

fn page() -> (Arc<Scripted>, Arc<CdpRouter>) {
    let (script, router) = Scripted::connect();
    script.answer("Animation.getPlaybackRate", json!({ "playbackRate": 0.5 }));
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
        json!({ "result": { "type": "object", "value": { "token": 3, "matched": 1 } } }),
    );
    (script, router)
}

fn plan(settle_ms: u32) -> CleanPlan {
    CleanSettings {
        hide_selectors: vec!["#own-banner".to_owned()],
        settle_ms,
        ..CleanSettings::default()
    }
    .plan(&[], CleanOverrides::default())
}

fn media() -> Value {
    json!({ "media": "", "features": [{ "name": "prefers-color-scheme", "value": "dark" }] })
}

fn session_of_every_call(script: &Scripted) -> bool {
    script
        .sent()
        .iter()
        .all(|message| message["sessionId"] == "S1")
}

#[tokio::test]
async fn prepare_freezes_and_hides_in_the_isolated_world_and_undo_puts_it_back() {
    let (script, router) = page();
    let prepared = prepare(&router, "S1", &plan(0), &media()).await;
    assert!(prepared.is_prepared());
    prepared.undo(&router).await;
    assert_eq!(
        script.methods(),
        [
            "Animation.getPlaybackRate",
            "Animation.setPlaybackRate",
            "Emulation.setEmulatedMedia",
            "Page.getFrameTree",
            "Page.createIsolatedWorld",
            "Runtime.evaluate",
            // The undo, in reverse.
            "Runtime.evaluate",
            "Emulation.setEmulatedMedia",
            "Animation.setPlaybackRate",
        ]
    );
    assert!(session_of_every_call(&script));
    let rates = script.params("Animation.setPlaybackRate");
    assert_eq!(rates[0]["playbackRate"], 0.0);
    assert_eq!(rates[1]["playbackRate"], 0.5, "the timeline's own rate");
    let media_calls = script.params("Emulation.setEmulatedMedia");
    assert_eq!(
        media_calls[0]["features"],
        json!([
            { "name": "prefers-color-scheme", "value": "dark" },
            { "name": "prefers-reduced-motion", "value": "reduce" }
        ])
    );
    assert_eq!(media_calls[1], media(), "the view's own media");
    let world = &script.params("Page.createIsolatedWorld")[0];
    assert_eq!(world["frameId"], "F1");
    assert_eq!(world["worldName"], "rbrowser-capture");
    let evaluations = script.params("Runtime.evaluate");
    for evaluation in &evaluations {
        assert_eq!(evaluation["contextId"], 42, "never the page's own world");
        assert_eq!(evaluation["awaitPromise"], true);
    }
    let prepare_call = evaluations[0]["expression"].as_str().unwrap_or_default();
    assert!(prepare_call.contains("__rbCaptureClean.prepare("));
    assert!(prepare_call.contains("#own-banner"));
    assert!(
        prepare_call.contains("#onetrust-banner-sdk"),
        "the built-in list"
    );
    let undo_call = evaluations[1]["expression"].as_str().unwrap_or_default();
    assert!(undo_call.ends_with("__rbCaptureClean.undo(3)"));
}

#[tokio::test]
async fn hiding_alone_leaves_the_timeline_and_the_media() {
    let (script, router) = page();
    let hide_only = CleanSettings::default().plan(
        &[],
        CleanOverrides {
            hide_banners: None,
            freeze_animations: Some(false),
        },
    );
    prepare(&router, "S1", &hide_only, &media())
        .await
        .undo(&router)
        .await;
    assert!(
        script
            .methods()
            .iter()
            .all(|method| !method.starts_with("Animation.") && !method.starts_with("Emulation."))
    );
    assert_eq!(script.params("Runtime.evaluate").len(), 2);
}

#[tokio::test]
async fn a_plan_that_does_nothing_sends_nothing() {
    let (script, router) = page();
    let off = CleanSettings::default().plan(
        &[],
        CleanOverrides {
            hide_banners: Some(false),
            freeze_animations: Some(false),
        },
    );
    let prepared = prepare(&router, "S1", &off, &media()).await;
    assert!(!prepared.is_prepared());
    prepared.undo(&router).await;
    assert!(script.methods().is_empty());
}

#[tokio::test]
async fn without_an_isolated_world_the_timeline_and_media_still_come_back() {
    let (script, router) = page();
    script.fail("Page.createIsolatedWorld", "no frame");
    let prepared = prepare(&router, "S1", &plan(0), &media()).await;
    assert!(!prepared.is_prepared());
    prepared.undo(&router).await;
    assert!(
        script.params("Runtime.evaluate").is_empty(),
        "no page world"
    );
    let rates = script.params("Animation.setPlaybackRate");
    assert_eq!(
        rates.last().map(|rate| rate["playbackRate"].clone()),
        Some(json!(0.5))
    );
    assert_eq!(
        script.params("Emulation.setEmulatedMedia").last(),
        Some(&media())
    );
}

#[tokio::test]
async fn a_script_that_throws_is_not_undone_but_the_rest_is() {
    let (script, router) = page();
    script.answer(
        "Runtime.evaluate",
        json!({ "exceptionDetails": { "text": "boom" } }),
    );
    let prepared = prepare(&router, "S1", &plan(0), &media()).await;
    assert!(!prepared.is_prepared());
    prepared.undo(&router).await;
    assert_eq!(script.params("Runtime.evaluate").len(), 1);
    assert_eq!(script.params("Animation.setPlaybackRate").len(), 2);
}

#[tokio::test]
async fn an_unknown_rate_is_put_back_as_one() {
    let (script, router) = page();
    script.fail("Animation.getPlaybackRate", "no animation domain");
    prepare(&router, "S1", &plan(0), &media())
        .await
        .undo(&router)
        .await;
    let rates = script.params("Animation.setPlaybackRate");
    assert_eq!(rates[1]["playbackRate"], 1.0);
}

/// A view whose animations the user stopped (RB-094-07) answers 0: the undo puts the 0 back
/// that the view holds, not a running timeline; without a held rate 0 is put back as one.
#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_view_stays_stopped_after_the_capture() {
    let (script, router) = page();
    script.answer("Animation.getPlaybackRate", json!({ "playbackRate": 0.0 }));
    let (_fake, hooks_router) = crate::cdp::fake::FakeBrowser::connect();
    let hooks = crate::time_control::hooks();
    hooks
        .attach(&hooks_router, rb_engine::ViewId(70), "S70")
        .await;
    hooks
        .set_rate(&hooks_router, &[rb_engine::ViewId(70)], 0.0)
        .await;
    prepare(&router, "S70", &plan(0), &media())
        .await
        .undo(&router)
        .await;
    let rates = script.params("Animation.setPlaybackRate");
    assert_eq!(rates[1]["playbackRate"], 0.0, "stopped as the user left it");
    prepare(&router, "S1", &plan(0), &media())
        .await
        .undo(&router)
        .await;
    let rates = script.params("Animation.setPlaybackRate");
    assert_eq!(rates[3]["playbackRate"], 1.0, "a 0 nobody holds runs again");
}

#[tokio::test]
async fn the_settle_wait_comes_after_the_preparation() {
    let (_script, router) = page();
    let before = std::time::Instant::now();
    let _prepared = prepare(&router, "S1", &plan(40), &media()).await;
    assert!(before.elapsed() >= Duration::from_millis(40));
}

fn target() -> CaptureTarget {
    CaptureTarget {
        session: "S1".to_owned(),
        metrics: json!({ "width": 400, "height": 800, "deviceScaleFactor": 1.0, "mobile": false }),
        height: 800,
        dpr: 1.0,
        hidden: false,
    }
}

#[tokio::test]
async fn the_capture_runs_between_preparation_and_undo_and_a_failure_is_still_undone() {
    for fails in [false, true] {
        let (script, router) = page();
        script.answer(
            "Page.getLayoutMetrics",
            json!({
                "cssVisualViewport": { "pageX": 0.0, "pageY": 0.0 },
                "cssLayoutViewport": { "pageX": 0.0, "pageY": 0.0 },
                "cssContentSize": { "width": 400.0, "height": 2000.0 }
            }),
        );
        if fails {
            script.fail("Page.captureScreenshot", "boom");
        } else {
            script.answer(
                "Page.captureScreenshot",
                json!({ "data": base64::engine::general_purpose::STANDARD.encode(b"png") }),
            );
        }
        let restore = || Some(target().metrics);
        let view = target();
        let shot = capture(&router, &view, &Shot::FullPage, 1.0, &restore);
        let result = around(&router, "S1", &plan(0), &media(), shot).await;
        assert_eq!(result.is_err(), fails);
        let methods = script.methods();
        let at = |name: &str| methods.iter().position(|method| method == name);
        let last = |name: &str| methods.iter().rposition(|method| method == name);
        assert!(
            at("Runtime.evaluate") < at("Page.captureScreenshot"),
            "{methods:?}"
        );
        assert!(
            at("Page.captureScreenshot") < last("Runtime.evaluate"),
            "{methods:?}"
        );
        assert_eq!(
            methods.last().map(String::as_str),
            Some("Animation.setPlaybackRate"),
            "{methods:?}"
        );
    }
}

#[tokio::test]
async fn a_capture_dropped_midway_is_still_undone() {
    let (script, router) = page();
    let never = std::future::pending::<()>();
    let dropped = tokio::time::timeout(
        Duration::from_millis(50),
        around(&router, "S1", &plan(0), &media(), never),
    )
    .await;
    assert!(dropped.is_err(), "the capture never ends");
    // The undo runs in a task of its own once the future is gone.
    for _ in 0..100 {
        if script.params("Animation.setPlaybackRate").len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let rates = script.params("Animation.setPlaybackRate");
    assert_eq!(rates.len(), 2, "{:?}", script.methods());
    assert_eq!(rates[1]["playbackRate"], 0.5, "the timeline runs again");
    assert_eq!(
        script.params("Emulation.setEmulatedMedia").last(),
        Some(&media())
    );
    let evaluations = script.params("Runtime.evaluate");
    assert!(
        evaluations
            .last()
            .and_then(|call| call["expression"].as_str())
            .is_some_and(|expression| expression.ends_with("__rbCaptureClean.undo(3)")),
        "the banners come back"
    );
}
