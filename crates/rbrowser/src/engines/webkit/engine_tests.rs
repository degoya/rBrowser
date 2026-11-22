use std::sync::Mutex;

use rb_config::{EngineKind, SyncConfig};
use rb_engine::test_support::conformance::{check_engine, check_unavailable, sample_spec};
use rb_engine::{Engine, EngineError, EngineHealth, HandleEvent, ViewId};
use rb_sync::{AgentMessage, SyncEvent};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::engines::webkit::fake::FakeSidecar;

/// A click event as the agent writes it (agent/src/protocol-fixtures.json).
pub(crate) const CLICK: &str = r#"{"kind":"event","v":1,"top":true,"seq":2,"time":1762790400001,"event":{"type":"click","button":"left","detail":1,"modifiers":8,"target":{"element":{"kind":"css","path":"main > button"},"offset":{"x":0.5,"y":0.5}}}}"#;

fn engine(fake: &Arc<FakeSidecar>) -> Arc<WebkitEngine> {
    WebkitEngine::new(Arc::new(fake.clone()), Handle::current(), "/* agent */")
}

#[tokio::test]
async fn webkit_passes_the_engine_conformance() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    assert_eq!(webkit.health(), EngineHealth::Stopped);
    check_engine(webkit.as_ref(), &sample_spec(EngineKind::Webkit, 3)).await;
    assert_eq!(fake.launches(), 1);
}

#[tokio::test]
async fn a_view_opens_with_its_options_url_agent_and_config() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    webkit.set_sync(&SyncConfig::default(), false);
    let spec = sample_spec(EngineKind::Webkit, 7);
    let handle = webkit.create_view(&spec).await.expect("opens");
    assert_eq!(handle.id(), ViewId(7));
    // The window is found by the view's own process (RB-080-39, RB-080-40); on macOS it stays
    // a window of its own (RB-097-02).
    let window = handle.native_window();
    if crate::engines::webkit::view::EMBEDDABLE {
        let window = window.expect("a window hint");
        assert_eq!(
            (window.process_id, window.window_class),
            (FakeSidecar::pid_of(7), None)
        );
    } else {
        assert!(window.is_none());
    }
    assert_eq!(
        fake.calls_of(rpc::LAUNCH),
        [json!({ "agent": "/* agent */" })]
    );
    let create = &fake.calls_of(rpc::CREATE)[0];
    assert_eq!(create["view"], 7);
    assert_eq!(create["url"], "http://localhost/start");
    assert_eq!(
        create["options"]["viewport"],
        json!({ "width": 390, "height": 844 })
    );
    assert_eq!(create["options"]["deviceScaleFactor"], 3.0);
    assert_eq!(create["options"]["hasTouch"], true);
    assert_eq!(create["agentConfig"]["active"], false);
}

#[tokio::test]
async fn without_the_webkit_build_it_says_so_until_it_is_installed() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    fake.fail_next(
        rpc::LAUNCH,
        rpc::NOT_INSTALLED,
        "WebKit 2388 is not installed",
    );
    let spec = sample_spec(EngineKind::Webkit, 1);
    let error = webkit
        .create_view(&spec)
        .await
        .err()
        .expect("not installed");
    let EngineError::Unavailable { engine, reason } = error else {
        panic!("unavailable expected");
    };
    assert_eq!(engine, EngineKind::Webkit);
    assert_eq!(reason, rb_i18n::t("engines.webkit.notInstalled"));
    assert_eq!(webkit.health(), EngineHealth::Unavailable(reason));
    check_unavailable(webkit.as_ref(), &spec).await;
    webkit.installed();
    assert_eq!(webkit.health(), EngineHealth::Stopped);
    webkit.create_view(&spec).await.expect("installed now");
}

#[tokio::test]
async fn switched_off_it_opens_nothing() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    webkit.set_enabled(false);
    check_unavailable(webkit.as_ref(), &sample_spec(EngineKind::Webkit, 1)).await;
    assert!(
        fake.calls().is_empty(),
        "no sidecar for a switched-off engine"
    );
}

#[tokio::test]
async fn a_sidecar_crash_ends_every_view_and_the_next_view_starts_again() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    let handle = webkit
        .create_view(&sample_spec(EngineKind::Webkit, 1))
        .await
        .expect("opens");
    let mut events = handle.events();
    fake.crash();
    let ended = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
    assert_eq!(
        ended.ok().flatten(),
        Some(HandleEvent::Exited { crashed: true })
    );
    assert_eq!(webkit.health(), EngineHealth::Stopped);
    assert!(matches!(
        handle.navigate("http://a/").await,
        Err(EngineError::Closed)
    ));
    webkit
        .create_view(&sample_spec(EngineKind::Webkit, 1))
        .await
        .expect("a new process");
    assert_eq!(fake.launches(), 2);
}

#[tokio::test]
async fn a_window_closed_by_the_user_ends_its_view_only() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    let one = webkit
        .create_view(&sample_spec(EngineKind::Webkit, 1))
        .await
        .expect("opens");
    let _two = webkit
        .create_view(&sample_spec(EngineKind::Webkit, 2))
        .await
        .expect("opens");
    let mut events = one.events();
    fake.notify(
        rpc::EVENT,
        json!({ "view": 1, "kind": "closed", "crashed": false }),
    );
    let ended = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
    assert_eq!(
        ended.ok().flatten(),
        Some(HandleEvent::Exited { crashed: false })
    );
    assert!(webkit.view(ViewId(1)).is_none());
    assert!(webkit.view(ViewId(2)).is_some());
}

#[tokio::test]
async fn agent_reports_reach_the_sink_only_as_a_sync_source() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    let seen: Arc<Mutex<Vec<(ViewId, AgentMessage)>>> = Arc::default();
    let sink_seen = seen.clone();
    webkit.set_sink(Some(Arc::new(move |view, message| {
        lock(&sink_seen).push((view, message));
    })));
    webkit
        .create_view(&sample_spec(EngineKind::Webkit, 4))
        .await
        .expect("opens");
    fake.notify(
        rpc::AGENT_REPORT,
        json!({ "view": 4, "top": true, "payload": CLICK }),
    );
    fake.notify(
        rpc::AGENT_REPORT,
        json!({ "view": 4, "top": true, "payload": "{\"kind\":\"forged\"}" }),
    );
    fake.notify(
        rpc::AGENT_REPORT,
        json!({ "view": 99, "top": true, "payload": CLICK }),
    );
    crate::engines::tests::until(|| !lock(&seen).is_empty()).await;
    webkit.set_sync(&SyncConfig::default(), false);
    fake.notify(
        rpc::AGENT_REPORT,
        json!({ "view": 4, "top": true, "payload": CLICK }),
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    let seen = lock(&seen).clone();
    assert_eq!(seen.len(), 1, "only the valid report of a known view");
    let (view, AgentMessage::Event(event)) = &seen[0] else {
        panic!("an event");
    };
    assert_eq!(*view, ViewId(4));
    assert!(matches!(event.event, SyncEvent::Click(_)));
}

#[tokio::test]
async fn shutdown_closes_the_windows_without_starting_a_sidecar() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    webkit.shutdown().await;
    assert!(fake.calls().is_empty(), "nothing ran, nothing starts");
    webkit
        .create_view(&sample_spec(EngineKind::Webkit, 1))
        .await
        .expect("opens");
    webkit.shutdown().await;
    assert_eq!(fake.calls_of(rpc::CLOSE), [json!({ "view": 1 })]);
    assert_eq!(fake.calls_of(rpc::SHUTDOWN).len(), 1);
    assert_eq!(webkit.health(), EngineHealth::Stopped);
}

#[tokio::test]
async fn a_view_without_its_cell_id_is_refused() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    let mut spec = sample_spec(EngineKind::Webkit, 1);
    spec.view = None;
    assert!(matches!(
        webkit.create_view(&spec).await,
        Err(EngineError::Invalid(_))
    ));
}

/// RB-090-33 review: a new sync configuration - the scroll mode `sync_set` set at runtime
/// included - reaches the agents of the views already open, not only of views opened later.
#[tokio::test]
async fn a_new_sync_configuration_reaches_the_open_views() {
    let fake = FakeSidecar::new();
    let webkit = engine(&fake);
    webkit
        .create_view(&sample_spec(EngineKind::Webkit, 7))
        .await
        .expect("opens");
    assert!(fake.calls_of(rpc::AGENT).is_empty());
    let wheel = SyncConfig {
        scroll_mode: rb_config::ScrollMode::Wheel,
        ..SyncConfig::default()
    };
    webkit.set_sync(&wheel, true);
    crate::engines::tests::until(|| !fake.calls_of(rpc::AGENT).is_empty()).await;
    let configure = &fake.calls_of(rpc::AGENT)[0];
    assert_eq!(configure["view"], 7);
    assert_eq!(configure["command"]["cmd"], "configure");
    assert_eq!(configure["command"]["config"]["scrollMode"], "wheel");
}
