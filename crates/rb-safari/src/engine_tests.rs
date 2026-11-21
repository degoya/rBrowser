use std::sync::Arc;
use std::time::Duration;

use rb_engine::test_support::conformance::{check_engine, check_unavailable, sample_spec};
use rb_engine::{
    Emulation, Engine, EngineError, EngineHealth, EngineKind, FieldSupport, HandleEvent,
    ScreenshotRequest, ViewHandle, ViewId,
};

use super::*;
use crate::driver::Attached;
use crate::testing::{CHROME, FAKE_PNG, FakeSafari};

/// The bundle the tests boot: the fake knows a boot script by its shape, not its bundle.
const BUNDLE: &str = "globalThis.__rbAgent = { command: () => ({ status: 'done' }) };";

fn engine(fake: &FakeSafari) -> Arc<SafariEngine> {
    SafariEngine::new(Arc::new(Attached(fake.port())), BUNDLE)
}

async fn exited(mut events: rb_engine::HandleEvents) -> Option<bool> {
    let wait = async {
        loop {
            match events.recv().await {
                Some(HandleEvent::Exited { crashed }) => return Some(crashed),
                Some(_) => {}
                None => return None,
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), wait)
        .await
        .ok()
        .flatten()
}

#[tokio::test]
async fn safari_passes_the_engine_contract() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    assert_eq!(engine.health(), EngineHealth::Stopped);
    check_engine(engine.as_ref(), &sample_spec(EngineKind::Safari, 7)).await;
    assert_eq!(fake.sessions(), 0, "close ends the session");
}

#[tokio::test]
async fn switched_off_safari_opens_no_view() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    engine.set_config(SafariConfig {
        enabled: false,
        edition: Edition::Safari,
    });
    check_unavailable(engine.as_ref(), &sample_spec(EngineKind::Safari, 7)).await;
    assert!(fake.requests().is_empty(), "no driver was asked");
}

#[tokio::test]
async fn a_view_opens_the_url_at_the_breakpoint_s_viewport() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    let spec = sample_spec(EngineKind::Safari, 7);
    let view = engine.create_view(&spec).await.expect("a view");
    let id = fake.session_ids().pop().expect("a session");
    assert_eq!(fake.url_of(&id).as_deref(), Some("http://localhost/start"));
    let (x, y, width, height) = fake.rect_of(&id).expect("a window");
    // The window is the viewport and Safari's toolbar, at the place asked for.
    assert_eq!((x, y), (10, 20));
    assert_eq!((width, height), (390, 844 + CHROME.1));
    assert_eq!(engine.health(), EngineHealth::Running);
    let new_session = &fake.requests_to("/session")[0];
    assert_eq!(
        new_session.body["capabilities"]["alwaysMatch"]["browserName"],
        "safari"
    );
    assert_eq!(ViewHandle::id(view.as_ref()), ViewId(7));
}

#[tokio::test]
async fn a_window_safari_keeps_wider_is_reported_with_what_it_shows() {
    let fake = FakeSafari::start().await;
    fake.min_width(500);
    let engine = engine(&fake);
    let view = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    let emulation = Emulation {
        width: 320,
        height: 568,
        ..Emulation::default()
    };
    let report = view.apply_emulation(&emulation).await.expect("a report");
    let FieldSupport::Approximated(reason) = &report.fields[0].support else {
        panic!("approximated: {report:?}");
    };
    assert!(reason.contains("500x568"), "{reason}");
}

#[tokio::test]
async fn remote_automation_off_names_the_command_and_a_later_try_works() {
    let fake = FakeSafari::start().await;
    fake.refuse_sessions(
        "Could not create a session: You must enable the 'Allow Remote Automation' option in Safari's Develop menu to control Safari via WebDriver.",
    );
    let engine = engine(&fake);
    let spec = sample_spec(EngineKind::Safari, 7);
    let Err(EngineError::Unavailable {
        engine: kind,
        reason,
    }) = engine.create_view(&spec).await
    else {
        panic!("unavailable");
    };
    assert_eq!(kind, EngineKind::Safari);
    assert!(
        reason.contains("safaridriver --enable") || reason == "engines.safari.notEnabled",
        "{reason}"
    );
    // Not the engine's state: once the person enabled it, the restart works.
    assert_ne!(
        std::mem::discriminant(&engine.health()),
        std::mem::discriminant(&EngineHealth::Unavailable(String::new()))
    );
    fake.accept_sessions();
    engine
        .create_view(&spec)
        .await
        .expect("a view after enabling");
}

#[tokio::test]
async fn a_second_view_safari_refuses_is_unavailable_and_the_first_runs_on() {
    let fake = FakeSafari::start().await;
    fake.max_sessions(1);
    let engine = engine(&fake);
    let first = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("the first view");
    let second = engine
        .create_view(&sample_spec(EngineKind::Safari, 8))
        .await;
    assert!(matches!(
        second,
        Err(EngineError::Unavailable {
            engine: EngineKind::Safari,
            ..
        })
    ));
    first
        .navigate("http://localhost/still")
        .await
        .expect("runs on");
}

#[tokio::test]
async fn a_crashed_driver_ends_every_view_as_crashed() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    let first = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    let second = engine
        .create_view(&sample_spec(EngineKind::Safari, 8))
        .await
        .expect("a view");
    let first_events = first.events();
    let second_events = second.events();
    fake.stop();
    let mut failed = false;
    for _ in 0..50 {
        if matches!(
            first.navigate("http://localhost/after").await,
            Err(EngineError::Closed)
        ) {
            failed = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(failed, "the gone driver closes the view");
    assert_eq!(exited(first_events).await, Some(true));
    assert_eq!(exited(second_events).await, Some(true));
    assert_eq!(engine.health(), EngineHealth::Stopped);
}

#[tokio::test]
async fn a_window_closed_in_safari_ends_its_view_as_closed() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    let view = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    let events = view.events();
    let id = fake.session_ids().pop().expect("a session");
    fake.close_window(&id);
    assert!(matches!(view.reload(false).await, Err(EngineError::Closed)));
    assert_eq!(exited(events).await, Some(false));
    assert!(engine.view(ViewId(7)).is_none());
}

#[tokio::test]
async fn a_page_screenshot_takes_the_html_element_and_falls_back_to_the_viewport() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    let view = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    let page = ScreenshotRequest { full_page: true };
    assert_eq!(
        view.screenshot(page).await.expect("png").png_base64,
        FAKE_PNG
    );
    assert_eq!(fake.requests_to("/element/fake-node-1/screenshot").len(), 1);
    fake.hide("html");
    assert_eq!(
        view.screenshot(page).await.expect("png").png_base64,
        FAKE_PNG
    );
    assert_eq!(fake.requests_to("/screenshot").len(), 2);
}

#[tokio::test]
async fn shutdown_ends_every_session() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    for view in [7, 8] {
        engine
            .create_view(&sample_spec(EngineKind::Safari, view))
            .await
            .expect("a view");
    }
    assert_eq!(fake.sessions(), 2);
    engine.shutdown().await;
    assert_eq!(fake.sessions(), 0);
    assert_eq!(engine.health(), EngineHealth::Stopped);
}

#[tokio::test]
async fn the_session_s_safari_version_is_kept_for_about() {
    let fake = FakeSafari::start().await;
    let engine = engine(&fake);
    engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    assert_eq!(crate::platform::cached_version().as_deref(), Some("26.1"));
}

/// A launcher that hands out the fake and keeps the edition of every start.
struct Recording {
    port: u16,
    editions: std::sync::Mutex<Vec<Edition>>,
}

#[async_trait::async_trait]
impl Launch for Recording {
    async fn launch(&self, edition: Edition) -> Result<Launched, crate::error::SafariError> {
        rb_util::lock(&self.editions).push(edition);
        Attached(self.port).launch(edition).await
    }
}

#[tokio::test]
async fn another_edition_starts_its_own_driver_and_ends_the_old_one_s_views() {
    let fake = FakeSafari::start().await;
    let launch = Arc::new(Recording {
        port: fake.port(),
        editions: std::sync::Mutex::new(Vec::new()),
    });
    let engine = SafariEngine::new(launch.clone(), BUNDLE);
    let first = engine
        .create_view(&sample_spec(EngineKind::Safari, 7))
        .await
        .expect("a view");
    let first_events = first.events();
    engine
        .create_view(&sample_spec(EngineKind::Safari, 8))
        .await
        .expect("a second view on the same driver");
    assert_eq!(*rb_util::lock(&launch.editions), [Edition::Safari]);
    engine.set_config(SafariConfig {
        enabled: true,
        edition: Edition::TechnologyPreview,
    });
    engine
        .create_view(&sample_spec(EngineKind::Safari, 9))
        .await
        .expect("a view in Technology Preview");
    assert_eq!(
        *rb_util::lock(&launch.editions),
        [Edition::Safari, Edition::TechnologyPreview],
        "the edition's own driver"
    );
    // The old driver's views end as crashed: their restart opens them on the new driver.
    assert_eq!(exited(first_events).await, Some(true));
    assert!(engine.view(ViewId(7)).is_none());
    assert!(engine.view(ViewId(8)).is_none());
    assert!(engine.view(ViewId(9)).is_some());
    assert_eq!(fake.sessions(), 1, "the old driver's sessions ended");
}
