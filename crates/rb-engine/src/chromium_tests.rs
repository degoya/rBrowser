use serde_json::json;

use super::*;
use crate::test_support::conformance::{check_engine, sample_spec};
use crate::test_support::{FAKE_PNG, FakeHost};

#[tokio::test]
async fn chromium_over_the_host_passes_the_conformance_checks() {
    let host = FakeHost::new();
    let engine = ChromiumEngine::new(host.clone());
    check_engine(&engine, &sample_spec(EngineKind::Chromium, 1)).await;
    // Everything went through the host the CEF app implements.
    let calls = host.calls();
    assert!(
        calls.contains(&"navigate 1 http://localhost/start".to_owned()),
        "{calls:?}"
    );
    assert!(calls.contains(&"navigate 1 http://localhost/conformance".to_owned()));
    assert!(calls.contains(&"reload 1 soft".to_owned()));
    assert!(calls.contains(&"go_back 1 -".to_owned()));
    assert!(calls.contains(&"go_forward 1 -".to_owned()));
    assert!(calls.contains(&"apply_emulation 1 390x844".to_owned()));
    let methods: Vec<String> = host
        .cdp_calls()
        .into_iter()
        .map(|call| call.method)
        .collect();
    assert_eq!(
        methods,
        [
            "Input.dispatchMouseEvent",
            "Input.dispatchMouseEvent",
            "Page.captureScreenshot"
        ]
    );
}

#[tokio::test]
async fn a_handle_wraps_an_existing_grid_view() {
    let host = FakeHost::new();
    let view = host.add_view("Desktop", EngineKind::Chromium);
    let engine = ChromiumEngine::new(host.clone());
    assert!(matches!(
        engine.handle(ViewId(99)),
        Err(EngineError::UnknownView(ViewId(99)))
    ));
    let handle = engine.handle(view).expect("a grid view");
    assert_eq!(handle.id(), view);
    let shot = handle
        .screenshot(ScreenshotRequest { full_page: true })
        .await
        .expect("a screenshot");
    assert_eq!(shot.png_base64, FAKE_PNG);
    assert_eq!(
        host.cdp_calls()[0].params,
        json!({ "format": "png", "captureBeyondViewport": true })
    );
    // Closing lets the handle go; the grid view stays.
    handle.close().await.expect("closes");
    assert!(host.view(view).is_some());
    assert!(matches!(
        handle.screenshot(ScreenshotRequest::default()).await,
        Err(EngineError::Closed)
    ));
}

#[test]
fn input_becomes_cdp_input() {
    let (method, params) = cdp_input(&InputEvent::Mouse {
        phase: Phase::Down,
        x: 12.5,
        y: 30.0,
        button: Button::Right,
        clicks: 2,
        modifiers: 2,
    })
    .expect("maps");
    assert_eq!(method, "Input.dispatchMouseEvent");
    assert_eq!(
        params,
        json!({ "type": "mousePressed", "x": 12.5, "y": 30.0, "button": "right", "clickCount": 2, "modifiers": 2 })
    );
    let (_, wheel) = cdp_input(&InputEvent::Wheel {
        x: 1.0,
        y: 2.0,
        delta_x: 0.0,
        delta_y: -40.0,
    })
    .expect("maps");
    assert_eq!(wheel["type"], "mouseWheel");
    assert_eq!(wheel["deltaY"], -40.0);
    let key = |phase, text: Option<&str>| {
        cdp_input(&InputEvent::Key {
            phase,
            key: "a".into(),
            code: "KeyA".into(),
            text: text.map(str::to_owned),
            modifiers: 8,
        })
    };
    let (method, down) = key(Phase::Down, Some("A")).expect("maps");
    assert_eq!(method, "Input.dispatchKeyEvent");
    assert_eq!(down["type"], "keyDown");
    assert_eq!(down["text"], "A");
    assert_eq!(
        key(Phase::Down, None).expect("maps").1["type"],
        "rawKeyDown"
    );
    let (_, up) = key(Phase::Up, Some("A")).expect("maps");
    assert_eq!(up["type"], "keyUp");
    assert!(up.get("text").is_none());
    assert!(matches!(
        key(Phase::Move, None),
        Err(EngineError::Invalid(_))
    ));
    let (method, text) = cdp_input(&InputEvent::Text { text: "hi".into() }).expect("maps");
    assert_eq!(
        (method, text),
        ("Input.insertText", json!({ "text": "hi" }))
    );
}

#[tokio::test]
async fn create_view_opens_a_view_outside_the_grid_order() {
    let host = FakeHost::new();
    host.add_view("Desktop", EngineKind::Chromium);
    let engine = ChromiumEngine::new(host.clone());
    let mut spec = sample_spec(EngineKind::Chromium, 1);
    spec.view = None;
    let handle = engine.create_view(&spec).await.expect("opens");
    assert_eq!(handle.id(), ViewId(2));
    assert_eq!(engine.health(), EngineHealth::Running);
    engine.shutdown().await;
    assert!(host.view(ViewId(2)).is_some());
}
