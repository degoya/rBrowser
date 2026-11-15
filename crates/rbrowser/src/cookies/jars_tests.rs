use rb_bidi::testing::FakeFirefox;
use rb_config::EngineKind;
use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{CellRect, Engine};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::cookies::mapping::{to_bidi, to_playwright};
use crate::engines::webkit::fake::FakeSidecar;
use crate::engines::webkit::rpc;
use crate::host::nav::marker_url;
use crate::host::ui::recording::RecordingUi;

const NOW: u64 = 1_800_000_000;

pub(crate) fn sid() -> ChromeCookie {
    cookies_of(&json!({ "cookies": [
        { "name": "sid", "value": "hunter2", "domain": "127.0.0.1", "path": "/",
          "httpOnly": true, "session": true, "sameSite": "Lax" }
    ]}))
    .pop()
    .expect("a cookie")
}

/// A FirefoxEngine over `fake` with a view of session `admin` open.
pub(crate) async fn firefox(fake: &FakeFirefox) -> Arc<FirefoxEngine> {
    let engine = FirefoxEngine::new(
        rb_bidi::FirefoxConfig {
            launch: fake.launcher(),
            agent_source: Arc::from(""),
            agent_config: crate::sync_agent::agent_config(&rb_config::SyncConfig::default(), false),
        },
        Handle::current(),
    );
    let mut spec = sample_spec(EngineKind::Firefox, 5);
    spec.breakpoint.session = Some("admin".to_owned());
    engine.create_view(&spec).await.expect("a Firefox view");
    engine
}

/// A WebkitEngine over `fake` with view 6 open.
pub(crate) async fn webkit(fake: &Arc<FakeSidecar>) -> Arc<WebkitEngine> {
    let engine = WebkitEngine::new(Arc::new(fake.clone()), Handle::current(), "");
    engine
        .create_view(&sample_spec(EngineKind::Webkit, 6))
        .await
        .expect("a WebKit view");
    engine
}

pub(crate) async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn a_session_is_read_over_the_cdp_session_of_one_of_its_views() {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui, router, Handle::current());
    let config = serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [{ "id": "desktop", "width": 1440, "height": 900 }]
    }))
    .expect("the test config parses");
    host.open_config(config, None).expect("opens");
    let view = host.views().first().map(|view| view.id).expect("a view");
    fake.add_target("T1", &marker_url(view));
    host.on_view_bounds(
        view,
        CellRect {
            x: 0,
            y: 0,
            width: 400,
            height: 300,
        },
    );
    host.on_load_end(view, &marker_url(view));
    until(|| host.cdp_sessions().iter().any(|(id, _)| *id == view)).await;
    fake.answer_with(
        READ_METHOD,
        json!({ "cookies": [
            { "name": "sid", "value": "hunter2", "domain": "127.0.0.1", "path": "/" }
        ]}),
    );
    let jars = HostJars { host: host.clone() };
    let cookies = jars.read(view).await.expect("the cookies");
    assert_eq!(cookies.len(), 1);
    assert_eq!(fake.sent(READ_METHOD).len(), 1);
    let error = jars.read(ViewId(99)).await.expect_err("no such view");
    assert!(error.contains("99"), "{error}");
}

#[tokio::test]
async fn firefox_takes_the_cookies_into_the_sessions_user_context() {
    let fake = FakeFirefox::start().await;
    let engine = firefox(&fake).await;
    let cookie = to_bidi(&sid(), NOW).expect("maps");
    let taken = into_firefox(Some(engine), "admin", std::slice::from_ref(&cookie))
        .await
        .expect("taken");
    assert_eq!((taken.set, taken.refused), (1, 0));
    let set = fake.calls("storage.setCookie");
    assert_eq!(set[0]["cookie"]["name"], "sid");
    assert_eq!(set[0]["partition"]["type"], "storageKey");
    let missing = into_firefox(None, "admin", &[cookie]).await;
    assert!(missing.is_err());
}

#[tokio::test]
async fn webkit_takes_the_cookies_into_the_view_and_a_closed_view_says_so() {
    let fake = FakeSidecar::new();
    let engine = webkit(&fake).await;
    let cookie = to_playwright(&sid(), NOW).expect("maps");
    let taken = into_webkit(
        Some(engine.clone()),
        ViewId(6),
        std::slice::from_ref(&cookie),
    )
    .await
    .expect("taken");
    assert_eq!((taken.set, taken.refused), (1, 0));
    let sent = &fake.calls_of(rpc::COOKIES)[0];
    assert_eq!(sent["view"], 6);
    assert_eq!(sent["cookies"][0]["sameSite"], "Lax");
    assert!(
        into_webkit(Some(engine), ViewId(7), std::slice::from_ref(&cookie))
            .await
            .is_err()
    );
    assert!(into_webkit(None, ViewId(6), &[cookie]).await.is_err());
}
