use super::*;
use crate::test_support::FakeEngine;
use crate::test_support::conformance::{check_engine, sample_spec};

#[test]
fn engines_have_names_for_people() {
    assert_eq!(engine_name(EngineKind::Chromium), "Chromium");
    assert_eq!(engine_name(EngineKind::Firefox), "Firefox");
    assert_eq!(engine_name(EngineKind::Webkit), "WebKit");
}

#[test]
fn health_and_input_serialize_for_mcp() {
    assert_eq!(
        serde_json::to_value(EngineHealth::Unavailable("no Firefox".into())).expect("serializes"),
        serde_json::json!({ "state": "unavailable", "reason": "no Firefox" })
    );
    assert_eq!(
        serde_json::to_value(EngineHealth::Running).expect("serializes"),
        serde_json::json!({ "state": "running" })
    );
    let wheel = InputEvent::Wheel {
        x: 1.0,
        y: 2.0,
        delta_x: 0.0,
        delta_y: 120.0,
    };
    let json = serde_json::to_value(&wheel).expect("serializes");
    assert_eq!(json["type"], "wheel");
    assert_eq!(json["deltaY"], 120.0);
}

#[tokio::test]
async fn the_host_s_events_reach_only_their_view_s_handle() {
    let (sender, receiver) = broadcast::channel(16);
    let mut events = HandleEvents::from_host(ViewId(2), receiver);
    for event in [
        ViewEvent::Navigated {
            view: ViewId(1),
            url: "http://other/".into(),
        },
        ViewEvent::LayoutChanged,
        ViewEvent::Navigated {
            view: ViewId(2),
            url: "http://mine/".into(),
        },
        ViewEvent::Updated(ViewId(2)),
        ViewEvent::Closed(ViewId(1)),
        ViewEvent::Closed(ViewId(2)),
    ] {
        sender.send(event).expect("a receiver");
    }
    drop(sender);
    assert_eq!(
        events.recv().await,
        Some(HandleEvent::Navigated {
            url: "http://mine/".into()
        })
    );
    assert_eq!(events.recv().await, Some(HandleEvent::Updated));
    assert_eq!(
        events.recv().await,
        Some(HandleEvent::Exited { crashed: false })
    );
    assert_eq!(events.recv().await, None);
}

#[tokio::test]
async fn the_fake_engine_passes_the_conformance_checks() {
    for kind in [
        EngineKind::Firefox,
        EngineKind::Webkit,
        EngineKind::Chromium,
    ] {
        let engine = FakeEngine::new(kind);
        assert_eq!(engine.health(), EngineHealth::Stopped);
        check_engine(engine.as_ref(), &sample_spec(kind, 7)).await;
    }
}

#[tokio::test]
async fn a_fake_view_crashes_and_closes_on_request() {
    let engine = FakeEngine::new(EngineKind::Firefox);
    let handle = engine
        .create_view(&sample_spec(EngineKind::Firefox, 4))
        .await
        .expect("opens");
    let fake = engine.view(ViewId(4)).expect("made");
    let mut events = handle.events();
    fake.crash();
    assert_eq!(
        events.recv().await,
        Some(HandleEvent::Exited { crashed: true })
    );
    assert!(fake.is_closed());
    // Ending twice announces once.
    fake.close_by_user();
    engine.shutdown().await;
    assert_eq!(engine.shutdowns(), 1);
    assert_eq!(engine.health(), EngineHealth::Stopped);
    engine.fail_next_start("no profile");
    assert!(matches!(
        engine.create_view(&sample_spec(EngineKind::Firefox, 5)).await,
        Err(EngineError::Unavailable { reason, .. }) if reason == "no profile"
    ));
}
