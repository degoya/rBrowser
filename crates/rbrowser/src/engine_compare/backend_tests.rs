use std::sync::Arc;
use std::time::Duration;

use rb_config::{Breakpoint, EngineKind};
use rb_engine::chromium::hidden::{RenderBackend, RenderSpec, WaitSpec};
use rb_engine::test_support::{FAKE_PNG, FakeEngine};
use rb_engine::{Emulation, ViewId};

use super::*;

fn desktop() -> Breakpoint {
    serde_json::from_str(r#"{ "id": "desktop", "width": 1280, "height": 800 }"#)
        .expect("a breakpoint")
}

fn spec() -> RenderSpec {
    RenderSpec {
        breakpoint: "desktop".to_owned(),
        emulation: Emulation {
            width: 1280,
            height: 800,
            dpr: 2.0,
            ..Emulation::default()
        },
        session: "default".to_owned(),
    }
}

fn backend(engine: &Arc<FakeEngine>) -> EngineBackend {
    EngineBackend::new(
        engine.clone(),
        desktop(),
        "rBrowser - Desktop - Firefox".to_owned(),
    )
}

#[tokio::test]
async fn a_view_opens_in_the_reserved_range_with_the_spec_and_no_cell() {
    let engine = FakeEngine::new(EngineKind::Firefox);
    let backend = backend(&engine);
    let first = backend.open(&spec()).await.expect("opens");
    let second = backend.open(&spec()).await.expect("opens");
    assert_eq!(first.0, FIRST_VIEW_ID);
    assert_eq!(second.0, FIRST_VIEW_ID + 1, "never the same view twice");
    let view = engine.view(ViewId(first.0)).expect("the engine made it");
    let made = view.spec();
    assert_eq!(made.breakpoint.id, "desktop");
    assert_eq!(made.emulation.width, 1280);
    assert!((made.emulation.dpr - 2.0).abs() < f64::EPSILON);
    assert_eq!(
        made.cell, None,
        "the breakpoint's own size, not a grid cell's"
    );
    assert_eq!(made.title, "rBrowser - Desktop - Firefox");
}

#[tokio::test]
async fn a_page_loads_and_its_viewport_shot_is_the_decoded_png() {
    let engine = FakeEngine::new(EngineKind::Webkit);
    let backend = backend(&engine);
    let view = backend.open(&spec()).await.expect("opens");
    backend
        .navigate(view, "https://example.test/")
        .await
        .expect("loads");
    assert_eq!(
        engine.view(ViewId(view.0)).expect("view").url(),
        "https://example.test/"
    );
    let png = backend.screenshot(view, false).await.expect("a shot");
    let expected = base64::engine::general_purpose::STANDARD
        .decode(FAKE_PNG)
        .expect("the fake's PNG");
    assert_eq!(png, expected);
}

#[tokio::test(start_paused = true)]
async fn the_wait_is_the_settle_pause() {
    let engine = FakeEngine::new(EngineKind::Firefox);
    let backend = backend(&engine);
    let view = backend.open(&spec()).await.expect("opens");
    let started = tokio::time::Instant::now();
    let wait = WaitSpec {
        delay: Duration::from_millis(750),
        ..WaitSpec::default()
    };
    backend.wait(view, &wait).await.expect("waits");
    assert!(started.elapsed() >= Duration::from_millis(750));
}

#[tokio::test]
async fn a_closed_view_is_gone_and_its_steps_fail() {
    let engine = FakeEngine::new(EngineKind::Firefox);
    let backend = backend(&engine);
    let view = backend.open(&spec()).await.expect("opens");
    backend.close(view).await;
    assert!(engine.view(ViewId(view.0)).expect("view").is_closed());
    assert!(backend.screenshot(view, false).await.is_err());
    backend.close(view).await;
}

#[tokio::test]
async fn an_engine_that_cannot_start_fails_the_open() {
    let engine = FakeEngine::new(EngineKind::Firefox);
    engine.fail_next_start("Firefox is not installed");
    let error = backend(&engine).open(&spec()).await.expect_err("no view");
    assert!(error.to_string().contains("not installed"), "{error}");
}
