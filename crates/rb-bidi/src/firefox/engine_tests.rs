use std::sync::Arc;
use std::time::Duration;

use rb_engine::test_support::conformance::{check_engine, sample_spec};
use rb_engine::{Engine, EngineError, EngineHealth, EngineKind, HandleEvent, ViewHandle, ViewId};
use rb_sync::AgentMessage;
use serde_json::json;

use crate::client::{BidiClient, DEFAULT_TIMEOUT};
use crate::firefox::test_util::engine;
use crate::firefox::view::{FirefoxView, ViewState};
use crate::testing::FakeFirefox;

const WAIT: Duration = Duration::from_secs(2);

async fn exited(view: &Arc<dyn ViewHandle>) -> Option<bool> {
    let mut events = view.events();
    let wait = async {
        loop {
            match events.recv().await {
                Some(HandleEvent::Exited { crashed }) => return Some(crashed),
                Some(_) => {}
                None => return None,
            }
        }
    };
    tokio::time::timeout(WAIT, wait).await.ok().flatten()
}

async fn eventually(check: impl Fn() -> bool) {
    let wait = async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(WAIT, wait).await.expect("in time");
}

#[tokio::test]
async fn firefox_passes_the_engine_conformance() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "BUNDLE();");
    assert_eq!(
        engine.health(),
        EngineHealth::Stopped,
        "starts with its first view"
    );
    check_engine(engine.as_ref(), &sample_spec(EngineKind::Firefox, 7)).await;
}

#[tokio::test]
async fn a_phone_breakpoint_gets_its_viewport_even_when_firefox_never_answers() {
    let fake = FakeFirefox::start().await;
    fake.never_answer("browsingContext.setViewport");
    let engine = engine(&fake, "");
    let mut spec = sample_spec(EngineKind::Firefox, 3);
    spec.emulation.width = 393;
    spec.emulation.height = 852;
    spec.emulation.dpr = 3.0;
    let started = tokio::time::timeout(WAIT, engine.create_view(&spec))
        .await
        .expect("the missing ack does not hang the start");
    let view = started.expect("a view");
    let context = engine.view_state(ViewId(3)).expect("state").context.clone();
    assert_eq!(
        fake.calls("browsingContext.setViewport"),
        vec![
            json!({ "context": context, "viewport": { "width": 393, "height": 852 }, "devicePixelRatio": 3.0 })
        ]
    );
    let probe = fake.calls("script.evaluate").into_iter().any(|call| {
        call["expression"]
            .as_str()
            .is_some_and(|e| e.contains("innerWidth"))
    });
    assert!(probe, "the size is read back instead");
    let report = view.apply_emulation(&spec.emulation).await.expect("report");
    let marked = report.unsupported();
    for field in ["touch", "reducedMotion", "mobile"] {
        assert!(
            marked.contains(&field.to_owned()),
            "{field} is marked: {marked:?}"
        );
    }
}

/// A start that fails after the window opened closes it again (RB-080-38: every failed start
/// used to leave a blank Firefox window behind, one per rebuild).
#[tokio::test]
async fn a_start_that_fails_after_the_window_opened_closes_it() {
    let fake = FakeFirefox::start().await;
    fake.refuse(
        "browsingContext.setViewport",
        "Expected \"devicePixelRatio\" to be greater than 0",
    );
    let engine = engine(&fake, "");
    let error = match engine
        .create_view(&sample_spec(EngineKind::Firefox, 6))
        .await
    {
        Ok(_) => panic!("the view does not start"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("devicePixelRatio"), "{error}");
    let created = fake.calls("browsingContext.create");
    assert_eq!(created.len(), 1);
    let closed: Vec<String> = fake
        .calls("browsingContext.close")
        .iter()
        .filter_map(|call| call["context"].as_str().map(str::to_owned))
        .collect();
    assert!(
        closed.iter().any(|context| context.starts_with("ctx-")),
        "the new window is closed again: {closed:?}"
    );
    assert!(engine.view_state(ViewId(6)).is_none());
    assert_eq!(engine.live_views().len(), 0);
}

#[tokio::test]
async fn an_override_firefox_lacks_is_marked_not_failed() {
    let fake = FakeFirefox::start().await;
    fake.refuse("emulation.setLocaleOverride", "unknown command");
    let engine = engine(&fake, "");
    let mut spec = sample_spec(EngineKind::Firefox, 4);
    spec.emulation.locale = Some("fr-FR".to_owned());
    let view = engine.create_view(&spec).await.expect("view");
    let report = view.apply_emulation(&spec.emulation).await.expect("report");
    assert!(report.unsupported().contains(&"locale".to_owned()));
}

#[tokio::test]
async fn the_agent_is_preloaded_in_the_sandbox_on_the_views_channel() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "BUNDLE();");
    let _view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 7))
        .await
        .expect("view");
    let context = engine.view_state(ViewId(7)).expect("state").context.clone();
    let preload = fake
        .calls("script.addPreloadScript")
        .pop()
        .expect("preloaded");
    assert_eq!(preload["sandbox"], "rbrowser");
    assert_eq!(preload["contexts"], json!([context]));
    assert_eq!(
        preload["arguments"],
        json!([{ "type": "channel", "value": { "channel": "rb-agent-7" } }])
    );
    assert!(
        preload["functionDeclaration"]
            .as_str()
            .is_some_and(|f| f.contains("BUNDLE();"))
    );
    assert_eq!(
        fake.calls("browsingContext.close"),
        vec![json!({ "context": "initial", "promptUnload": false })],
        "the start window gives way to the view's"
    );

    let empty = FakeFirefox::start().await;
    let without = crate::firefox::test_util::engine(&empty, "");
    let _view = without
        .create_view(&sample_spec(EngineKind::Firefox, 8))
        .await
        .expect("view");
    assert!(
        empty.calls("script.addPreloadScript").is_empty(),
        "no bundle, no agent"
    );
}

#[tokio::test]
async fn agents_are_configured_when_ready_and_their_events_are_reported() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "BUNDLE();");
    let _view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 7))
        .await
        .expect("view");
    let context = engine.view_state(ViewId(7)).expect("state").context.clone();
    let mut reports = engine.reports();
    let before = fake.calls("script.evaluate").len();
    fake.agent_says(
        "rb-agent-7",
        &context,
        r#"{"kind":"ready","v":1,"top":true,"url":"http://localhost/"}"#,
    );
    let ready = tokio::time::timeout(WAIT, reports.recv())
        .await
        .expect("in time")
        .expect("report");
    assert_eq!(ready.view, ViewId(7));
    assert!(matches!(ready.message, AgentMessage::Ready(_)));
    eventually(|| {
        fake.calls("script.evaluate")[before..].iter().any(|call| {
            call["expression"]
                .as_str()
                .is_some_and(|e| e.contains("\"cmd\":\"configure\""))
        })
    })
    .await;

    let click = r##"{"kind":"event","v":1,"top":true,"seq":1,"time":1,"event":{"type":"click","button":"left","detail":1,"modifiers":0,"target":{"element":{"kind":"css","path":"#go"},"offset":{"x":0.5,"y":0.5}}}}"##;
    fake.agent_says("rb-agent-7", &context, click);
    let event = tokio::time::timeout(WAIT, reports.recv())
        .await
        .expect("in time")
        .expect("report");
    assert!(matches!(event.message, AgentMessage::Event(_)));
    assert!(event.top);

    // Untrusted: garbage and unknown channels are dropped.
    fake.agent_says("rb-agent-7", &context, "{ not json");
    fake.agent_says("rb-agent-99", &context, click);
    fake.agent_says("rb-agent-7", &context, click);
    let next = tokio::time::timeout(WAIT, reports.recv())
        .await
        .expect("in time")
        .expect("report");
    assert_eq!(next.view, ViewId(7));
}

#[tokio::test]
async fn a_new_agent_configuration_reaches_every_view() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "BUNDLE();");
    let _view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 7))
        .await
        .expect("view");
    let config = rb_sync::AgentConfig {
        active: false,
        ..crate::firefox::test_util::agent_config()
    };
    engine.set_agent_config(config);
    eventually(|| {
        fake.calls("script.evaluate").iter().any(|call| {
            call["expression"]
                .as_str()
                .is_some_and(|e| e.contains("\"active\":false"))
        })
    })
    .await;
}

#[tokio::test]
async fn a_crash_ends_every_view_and_the_next_view_starts_firefox_again() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 1))
        .await
        .expect("view");
    let ended = tokio::spawn({
        let view = view.clone();
        async move { exited(&view).await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    fake.crash();
    assert_eq!(ended.await.expect("join"), Some(true), "crashed");
    eventually(|| engine.health() == EngineHealth::Stopped).await;
    assert!(matches!(
        view.navigate("http://x/").await,
        Err(EngineError::Closed)
    ));
    let again = engine
        .create_view(&sample_spec(EngineKind::Firefox, 1))
        .await
        .expect("started again");
    assert_eq!(engine.health(), EngineHealth::Running);
    assert_eq!(fake.calls("session.new").len(), 2);
    drop(again);
}

#[tokio::test]
async fn closing_the_window_by_hand_ends_the_view() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 2))
        .await
        .expect("view");
    let context = engine.view_state(ViewId(2)).expect("state").context.clone();
    let ended = tokio::spawn({
        let view = view.clone();
        async move { exited(&view).await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    fake.emit(
        "browsingContext.contextDestroyed",
        json!({ "context": context, "parent": null }),
    );
    assert_eq!(ended.await.expect("join"), Some(false));
    assert!(engine.view_state(ViewId(2)).is_none());
}

#[tokio::test]
async fn an_old_firefox_is_refused_with_its_version() {
    let fake = FakeFirefox::start().await;
    fake.set_version("115.0");
    let engine = engine(&fake, "");
    let error = engine
        .create_view(&sample_spec(EngineKind::Firefox, 1))
        .await
        .err()
        .expect("too old");
    match error {
        EngineError::Unavailable { engine, reason } => {
            assert_eq!(engine, EngineKind::Firefox);
            assert!(reason.contains("115.0"), "{reason}");
        }
        other => panic!("{other}"),
    }
    assert_eq!(
        fake.calls("browser.close").len(),
        1,
        "the old Firefox is quit"
    );
}

#[tokio::test]
async fn shutdown_quits_firefox_and_ends_the_views() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 5))
        .await
        .expect("view");
    engine.shutdown().await;
    assert_eq!(engine.health(), EngineHealth::Stopped);
    assert_eq!(fake.calls("browser.close").len(), 1);
    assert!(matches!(view.reload(false).await, Err(EngineError::Closed)));
}

#[tokio::test]
async fn navigations_are_announced_engine_wide_once_per_url() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 6))
        .await
        .expect("view");
    let mut navigations = engine.navigations();
    view.navigate("http://localhost/next")
        .await
        .expect("navigate");
    let (id, url) = tokio::time::timeout(WAIT, navigations.recv())
        .await
        .expect("in time")
        .expect("announced");
    assert_eq!((id, url.as_str()), (ViewId(6), "http://localhost/next"));
    view.reload(false).await.expect("reload");
    view.navigate("http://localhost/last")
        .await
        .expect("navigate");
    let (_, url) = tokio::time::timeout(WAIT, navigations.recv())
        .await
        .expect("in time")
        .expect("announced");
    assert_eq!(
        url, "http://localhost/last",
        "a reload of the same URL is no navigation"
    );
    assert_eq!(
        engine.view_state(ViewId(6)).expect("state").url(),
        "http://localhost/last"
    );
}

/// The first load keeps the URL Firefox ended at (a redirect), not the one asked for: the
/// stand-in compares with it, and a stale one would load the page again.
#[tokio::test]
async fn the_first_load_keeps_the_url_firefox_ended_at() {
    let fake = FakeFirefox::start().await;
    fake.redirect("http://localhost/old", "http://localhost/new");
    let engine = engine(&fake, "");
    let mut spec = sample_spec(EngineKind::Firefox, 7);
    spec.url = "http://localhost/old".to_owned();
    engine.create_view(&spec).await.expect("view");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        engine.view_state(ViewId(7)).expect("state").url(),
        "http://localhost/new"
    );
}

/// An engine that replaces another on the same profile starts Firefox only after the old one
/// quit: two Firefox on one profile refuse each other.
#[tokio::test]
async fn a_successor_starts_firefox_only_after_its_predecessor_quit() {
    let fake = FakeFirefox::start().await;
    let old = engine(&fake, "");
    old.create_view(&sample_spec(EngineKind::Firefox, 7))
        .await
        .expect("old view");
    let new = engine(&fake, "");
    new.after(old.clone());
    new.create_view(&sample_spec(EngineKind::Firefox, 7))
        .await
        .expect("new view");
    let methods: Vec<String> = fake.commands().into_iter().map(|(m, _)| m).collect();
    let quit = methods
        .iter()
        .position(|m| m == "browser.close")
        .expect("the old Firefox quit");
    let started = methods
        .iter()
        .enumerate()
        .filter(|(_, m)| *m == "session.new")
        .nth(1)
        .map(|(at, _)| at)
        .expect("a second session");
    assert!(quit < started, "{methods:?}");
}

/// The engine's filter decides what reaches Firefox: here no headers, as in an untrusted
/// workspace; the view remembers what it sent.
#[tokio::test]
async fn every_emulation_passes_the_filter() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    engine.set_emulation_filter(Arc::new(|_, emulation: &rb_engine::Emulation| {
        let mut emulation = emulation.clone();
        emulation.headers.clear();
        emulation
    }));
    let mut spec = sample_spec(EngineKind::Firefox, 7);
    spec.emulation
        .headers
        .insert("Authorization".to_owned(), "Bearer project".to_owned());
    engine.create_view(&spec).await.expect("view");
    let headers = fake
        .calls("network.setExtraHeaders")
        .pop()
        .expect("headers");
    assert_eq!(headers["headers"], json!([]));
    let applied = engine
        .view_state(ViewId(7))
        .and_then(|state| state.applied_emulation())
        .expect("applied");
    assert!(applied.headers.is_empty());
}

/// The app finds a view's window by the Firefox process (RB-080-39); a Firefox rBrowser did
/// not start (the fake here) names none, so such a view stays a window of its own.
#[tokio::test]
async fn a_view_names_its_window_by_the_started_firefox_only() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let view = engine
        .create_view(&sample_spec(EngineKind::Firefox, 6))
        .await
        .expect("opens");
    assert!(view.is_external());
    assert_eq!(view.native_window(), None);
    let state = ViewState::new(
        ViewId(6),
        "ctx".to_owned(),
        BidiClient::connect(&fake.endpoint(), DEFAULT_TIMEOUT)
            .await
            .expect("connects"),
        tokio::sync::broadcast::channel(1).0,
        Some(4242),
    );
    let view = FirefoxView {
        state: Arc::new(state),
        engine: Arc::downgrade(&engine),
    };
    let window = view.native_window().expect("a window hint");
    assert_eq!(window.process_id, 4242);
    assert_eq!(window.window_class.as_deref(), Some("MozillaWindowClass"));
}
