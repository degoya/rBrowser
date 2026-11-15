use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::*;
use crate::external::{CASCADE_STEP, ExternalStatus, GAP};
use crate::test_support::FakeEngine;
use crate::test_support::conformance::sample_spec;
use crate::unsupported::UnsupportedEngine;

/// Fails the test when `done` does not hold within two seconds (the badge's budget).
pub(super) async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached within 2 s");
}

pub(super) struct Setup {
    pub(super) windows: Arc<ExternalWindows>,
    pub(super) firefox: Arc<FakeEngine>,
    pub(super) changes: Arc<AtomicUsize>,
}

pub(super) fn setup(restart_on_crash: bool) -> Setup {
    let changes = Arc::new(AtomicUsize::new(0));
    let counted = changes.clone();
    let windows = ExternalWindows::new(
        Handle::current(),
        Backoff {
            max_attempts: 3,
            base: Duration::from_millis(10),
        },
        restart_on_crash,
        Arc::new(move || {
            counted.fetch_add(1, Ordering::AcqRel);
        }),
    );
    let firefox = FakeEngine::new(EngineKind::Firefox);
    windows.set_engine(firefox.clone());
    Setup {
        windows,
        firefox,
        changes,
    }
}

pub(super) fn status(windows: &ExternalWindows, view: u32) -> Option<ExternalStatus> {
    windows.view(ViewId(view)).map(|known| known.status)
}

fn screen() -> Screen {
    Screen {
        main: WindowRect {
            x: 0,
            y: 0,
            width: 1000,
            height: 1000,
        },
        monitors: vec![WindowRect {
            x: 0,
            y: 0,
            width: 3000,
            height: 1200,
        }],
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_starts_on_demand_with_its_url_and_window() {
    let setup = setup(true);
    setup.windows.set_screen(screen());
    let mut spec = sample_spec(EngineKind::Firefox, 4);
    spec.window = None;
    assert_eq!(setup.firefox.health(), EngineHealth::Stopped);
    setup.windows.sync(vec![spec]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let fake = setup.firefox.view(ViewId(4)).expect("opened");
    assert_eq!(fake.url(), "http://localhost/start");
    assert_eq!(
        fake.window(),
        Some(WindowRect {
            x: 1000 + GAP,
            y: 0,
            width: 390,
            height: 844,
        })
    );
    let known = setup.windows.view(ViewId(4)).expect("known");
    assert_eq!(known.label, "Phone");
    assert_eq!(known.unsupported, ["mobile", "touch", "reducedMotion"]);
    assert!(setup.windows.handle(ViewId(4)).is_some());
    assert!(setup.changes.load(Ordering::Acquire) >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_engine_that_has_not_landed_shows_not_available_yet() {
    let setup = setup(true);
    setup
        .windows
        .set_engine(Arc::new(UnsupportedEngine::not_yet(EngineKind::Webkit)));
    setup.windows.sync(vec![
        sample_spec(EngineKind::Webkit, 5),
        sample_spec(EngineKind::Firefox, 4),
    ]);
    until(|| status(&setup.windows, 5) == Some(ExternalStatus::Unavailable)).await;
    let known = setup.windows.view(ViewId(5)).expect("known");
    assert_eq!(known.reason.as_deref(), Some("WebKit is not available yet"));
    // The other engine is not held up.
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    assert_eq!(
        setup.windows.restart(EngineKind::Webkit, None),
        Err(RestartError::Unavailable {
            engine: EngineKind::Webkit,
            reason: "WebKit is not available yet".into(),
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_names_what_is_wrong() {
    let setup = setup(true);
    let error = setup
        .windows
        .restart(EngineKind::Firefox, None)
        .expect_err("nothing runs");
    assert_eq!(error, RestartError::NotRunning(EngineKind::Firefox));
    assert_eq!(
        error.to_string(),
        "no Firefox view is open; add a breakpoint with this engine to the layout first"
    );
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    assert_eq!(
        setup.windows.restart(EngineKind::Firefox, Some(ViewId(9))),
        Err(RestartError::UnknownView {
            engine: EngineKind::Firefox,
            view: ViewId(9),
        })
    );
    assert_eq!(
        setup.windows.restart(EngineKind::Webkit, Some(ViewId(4))),
        Err(RestartError::UnknownView {
            engine: EngineKind::Webkit,
            view: ViewId(4),
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_start_shows_the_engine_s_reason() {
    let setup = setup(true);
    setup.firefox.fail_next_start("no Firefox found");
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Failed)).await;
    assert_eq!(
        setup.windows.view(ViewId(4)).and_then(|known| known.reason),
        Some("no Firefox found".into())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_the_layout_drops_closes() {
    let setup = setup(true);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let fake = setup.firefox.view(ViewId(4)).expect("opened");
    setup.windows.sync(Vec::new());
    until(|| fake.is_closed()).await;
    assert!(setup.windows.list().is_empty());
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        setup.firefox.views().len(),
        1,
        "no restart of a dropped view"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn arrange_cascades_and_raise_reaches_every_window() {
    let setup = setup(true);
    setup.windows.sync(vec![
        sample_spec(EngineKind::Firefox, 4),
        sample_spec(EngineKind::Firefox, 6),
    ]);
    until(|| {
        setup.windows.handle(ViewId(6)).is_some() && setup.windows.handle(ViewId(4)).is_some()
    })
    .await;
    // Without a known screen nothing moves.
    assert!(setup.windows.arrange().is_empty());
    setup.windows.set_screen(screen());
    assert_eq!(setup.windows.arrange(), [ViewId(4), ViewId(6)]);
    let second = setup.firefox.view(ViewId(6)).expect("opened");
    until(|| {
        second
            .window()
            .is_some_and(|rect| rect.x == 1000 + GAP + CASCADE_STEP)
    })
    .await;
    assert_eq!(setup.windows.raise_all(), 2);
    until(|| second.raised() == 1).await;
    setup.windows.set_placement(Placement::Monitor(1));
    setup.windows.arrange();
    until(|| {
        second
            .window()
            .is_some_and(|rect| rect.x == 2 * CASCADE_STEP)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn remember_reopens_a_window_where_it_was() {
    let setup = setup(true);
    setup.windows.set_screen(screen());
    setup.windows.set_placement(Placement::Remember);
    let last = WindowRect {
        x: 2000,
        y: 100,
        width: 390,
        height: 844,
    };
    let mut spec = sample_spec(EngineKind::Firefox, 4);
    spec.window = None;
    setup
        .windows
        .set_remembered(BTreeMap::from([(spec.breakpoint.id.clone(), last)]));
    setup.windows.sync(vec![spec]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    assert_eq!(
        setup.firefox.view(ViewId(4)).and_then(|fake| fake.window()),
        Some(last)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_closes_every_window_and_stops_the_engines() {
    let setup = setup(true);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    setup.windows.shutdown().await;
    assert!(setup.firefox.views().iter().all(|fake| fake.is_closed()));
    assert_eq!(setup.firefox.shutdowns(), 1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(setup.firefox.views().len(), 1, "no restart after shutdown");
}

/// A breakpoint moved to another engine: the old window closes and the view runs on the new
/// engine, on the page it showed (RB-040-04 review, finding 6).
#[tokio::test(flavor = "multi_thread")]
async fn a_view_that_changes_its_engine_moves_to_the_new_one() {
    let setup = setup(true);
    let webkit = FakeEngine::new(EngineKind::Webkit);
    setup.windows.set_engine(webkit.clone());
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let old = setup.firefox.view(ViewId(4)).expect("opened");
    old.navigate("http://localhost/kept")
        .await
        .expect("navigates");
    until(|| {
        setup
            .windows
            .view(ViewId(4))
            .is_some_and(|known| known.url == "http://localhost/kept")
    })
    .await;
    setup.windows.sync(vec![sample_spec(EngineKind::Webkit, 4)]);
    until(|| webkit.view(ViewId(4)).is_some()).await;
    // The old window is closed before the new one opens (RB-080-38): never two per view.
    assert!(old.is_closed(), "closed first, then started");
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let known = setup.windows.view(ViewId(4)).expect("still there");
    assert_eq!(known.engine, EngineKind::Webkit);
    assert_eq!(
        webkit.view(ViewId(4)).expect("on WebKit").url(),
        "http://localhost/kept"
    );
    assert_eq!(
        setup
            .windows
            .handle(ViewId(4))
            .map(|handle| handle.engine()),
        Some(EngineKind::Webkit)
    );
}

#[tokio::test]
async fn engine_health_answers_the_plugged_engine_or_not_available_yet() {
    let setup = setup(false);
    assert_eq!(
        setup.windows.engine_health(EngineKind::Firefox),
        setup.firefox.health()
    );
    assert_eq!(
        setup.windows.engine_health(EngineKind::Webkit),
        EngineHealth::Unavailable(not_available_yet(EngineKind::Webkit))
    );
    setup
        .windows
        .set_engine(Arc::new(UnsupportedEngine::because(
            EngineKind::Webkit,
            "no sidecar".to_owned(),
        )));
    assert_eq!(
        setup.windows.engine_health(EngineKind::Webkit),
        EngineHealth::Unavailable("no sidecar".to_owned())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_embedded_window_keeps_its_cell_when_the_others_are_arranged() {
    let setup = setup(true);
    setup.windows.set_screen(screen());
    setup.windows.sync(vec![
        sample_spec(EngineKind::Firefox, 4),
        sample_spec(EngineKind::Firefox, 6),
    ]);
    until(|| {
        status(&setup.windows, 4) == Some(ExternalStatus::Running)
            && status(&setup.windows, 6) == Some(ExternalStatus::Running)
    })
    .await;
    // Both as windows of their own: view 6 is the second of the cascade.
    assert_eq!(
        setup.windows.placement_rect(ViewId(6)).map(|rect| rect.x),
        Some(1000 + GAP + CASCADE_STEP)
    );
    assert!(setup.windows.set_embedded(ViewId(4), true));
    assert!(setup.windows.view(ViewId(4)).expect("view 4").embedded);
    // View 4 in its cell: view 6 takes the first slot, the one arrange gives it too, so a
    // release and the arrange after it move the window once.
    assert_eq!(
        setup.windows.placement_rect(ViewId(6)).map(|rect| rect.x),
        Some(1000 + GAP)
    );
    assert_eq!(setup.windows.placement_rect(ViewId(4)), None, "in its cell");
    assert_eq!(setup.windows.arrange(), [ViewId(6)]);
    let second = setup.firefox.view(ViewId(6)).expect("opened");
    until(|| second.window().is_some_and(|rect| rect.x == 1000 + GAP)).await;
    assert!(!setup.windows.set_embedded(ViewId(4), true), "unchanged");
    assert!(setup.windows.set_embedded(ViewId(4), false));
    assert_eq!(setup.windows.arrange(), [ViewId(4), ViewId(6)]);
}

/// A view whose engine built its window anew (a WebKit context at a new factor, RB-080-40)
/// says so through its events: the view leaves its cell and the app hears which views need
/// binding again.
#[tokio::test]
async fn a_replaced_window_leaves_its_cell_and_is_handed_to_the_app_once() {
    let setup = setup(true);
    setup.windows.set_screen(screen());
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    assert!(setup.windows.set_embedded(ViewId(4), true));
    let before = setup.changes.load(Ordering::Acquire);
    let view = setup.firefox.view(ViewId(4)).expect("opened");
    view.replace_window();
    until(|| !setup.windows.view(ViewId(4)).expect("view 4").embedded).await;
    assert!(setup.changes.load(Ordering::Acquire) > before, "announced");
    assert_eq!(setup.windows.take_replaced(), vec![ViewId(4)]);
    assert_eq!(setup.windows.take_replaced(), Vec::<ViewId>::new());
    assert_eq!(status(&setup.windows, 4), Some(ExternalStatus::Running));
}
