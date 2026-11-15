//! The life of one external view: a crash and its restart, a window closed by hand, a rebuilt
//! engine, a kept view, a failed start (RB-080-38). The fixture is `manager_tests`.

use std::sync::Arc;
use std::time::Duration;

use rb_config::EngineKind;

use super::super::tests::{setup, status, until};
use crate::ViewId;
use crate::engine::{EngineHealth, ViewHandle};
use crate::external::ExternalStatus;
use crate::test_support::conformance::sample_spec;

#[tokio::test(flavor = "multi_thread")]
async fn a_crash_shows_at_once_and_the_restart_brings_back_the_last_url() {
    let setup = setup(true);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let first = setup.firefox.view(ViewId(4)).expect("opened");
    let handle = setup.windows.handle(ViewId(4)).expect("running");
    handle
        .navigate("http://localhost/deep/page")
        .await
        .expect("navigates");
    until(|| {
        setup
            .windows
            .view(ViewId(4))
            .is_some_and(|known| known.url == "http://localhost/deep/page")
    })
    .await;
    first.crash();
    until(|| {
        matches!(
            status(&setup.windows, 4),
            Some(ExternalStatus::Restarting | ExternalStatus::Exited)
        )
    })
    .await;
    until(|| setup.firefox.views().len() == 2).await;
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let second = setup.firefox.view(ViewId(4)).expect("restarted");
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(second.url(), "http://localhost/deep/page");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_restart_on_crash_a_crashed_view_waits_for_the_restart_command() {
    let setup = setup(false);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    setup.firefox.view(ViewId(4)).expect("opened").crash();
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Exited)).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(setup.firefox.views().len(), 1);
    assert_eq!(
        setup.windows.restart(EngineKind::Firefox, None),
        Ok(vec![ViewId(4)])
    );
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    assert_eq!(setup.firefox.views().len(), 2);
}

/// A window the person closed stays closed, restart on crash or not; "Open again" (the restart
/// command) brings it back with its last page (RB-080-38).
#[tokio::test(flavor = "multi_thread")]
async fn a_window_closed_by_hand_stays_closed_until_opened_again() {
    let setup = setup(true);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let first = setup.firefox.view(ViewId(4)).expect("opened");
    first.close_by_user();
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Closed)).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        setup.firefox.views().len(),
        1,
        "no restart of a closed window"
    );
    // The next sync (a mode switch, a config reload) opens nothing either.
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(status(&setup.windows, 4), Some(ExternalStatus::Closed));
    assert_eq!(setup.firefox.views().len(), 1);
    assert_eq!(
        setup.windows.restart(EngineKind::Firefox, Some(ViewId(4))),
        Ok(vec![ViewId(4)])
    );
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    assert_eq!(setup.firefox.views().len(), 2);
}

/// An engine rebuilt under its views (a changed path or profile) starts them again - except a
/// window the person closed, which stays closed (RB-080-38).
#[tokio::test(flavor = "multi_thread")]
async fn a_rebuild_leaves_a_window_closed_by_hand_closed() {
    let setup = setup(true);
    setup.windows.sync(vec![
        sample_spec(EngineKind::Firefox, 4),
        sample_spec(EngineKind::Firefox, 5),
    ]);
    until(|| {
        status(&setup.windows, 4) == Some(ExternalStatus::Running)
            && status(&setup.windows, 5) == Some(ExternalStatus::Running)
    })
    .await;
    setup
        .firefox
        .view(ViewId(4))
        .expect("opened")
        .close_by_user();
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Closed)).await;
    assert_eq!(
        setup.windows.restart_after_rebuild(EngineKind::Firefox),
        Ok(vec![ViewId(5)])
    );
    until(|| setup.firefox.views().len() == 3).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(status(&setup.windows, 4), Some(ExternalStatus::Closed));
    assert_eq!(status(&setup.windows, 5), Some(ExternalStatus::Running));
    assert_eq!(setup.firefox.views().len(), 3, "one new window, for view 5");
}

/// A window that ends because its engine was switched off says so (the toggle's reason), not
/// "window closed"; the engine switched on again starts it (RB-080-38).
#[tokio::test(flavor = "multi_thread")]
async fn a_window_that_ends_with_a_switched_off_engine_says_so() {
    let setup = setup(true);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    setup.firefox.set_health(EngineHealth::Unavailable(
        "Firefox is switched off".to_owned(),
    ));
    setup
        .firefox
        .view(ViewId(4))
        .expect("opened")
        .close_by_user();
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Unavailable)).await;
    assert_eq!(
        setup.windows.view(ViewId(4)).and_then(|known| known.reason),
        Some("Firefox is switched off".to_owned())
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(setup.firefox.views().len(), 1, "no restart");
}

/// A view the layout keeps (a mode switch, a preset, a config reload) keeps its window: no
/// second start, the page stays (RB-080-38).
#[tokio::test(flavor = "multi_thread")]
async fn a_kept_view_keeps_its_window_across_syncs() {
    let setup = setup(true);
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let first = setup.firefox.view(ViewId(4)).expect("opened");
    first
        .navigate("http://localhost/deep/page")
        .await
        .expect("navigates");
    for _ in 0..3 {
        setup.windows.sync(vec![
            sample_spec(EngineKind::Firefox, 4),
            sample_spec(EngineKind::Firefox, 6),
        ]);
    }
    until(|| status(&setup.windows, 6) == Some(ExternalStatus::Running)).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(setup.firefox.views().len(), 2, "one window per view");
    let still = setup.firefox.view(ViewId(4)).expect("still there");
    assert!(Arc::ptr_eq(&first, &still));
    assert!(!first.is_closed());
    assert_eq!(first.url(), "http://localhost/deep/page");
}

/// A start that fails is not tried again by the next sync: the placeholder names the reason
/// until the person restarts (RB-080-38: no start loop while an engine is missing).
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_start_is_not_retried_by_the_next_sync() {
    let setup = setup(true);
    setup.firefox.fail_next_start("no Firefox found");
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Failed)).await;
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(status(&setup.windows, 4), Some(ExternalStatus::Failed));
    assert!(setup.firefox.views().is_empty(), "no second start");
}

/// A restart asked for while the view's start is still under way is not dropped (RB-080-40:
/// `engine.restart` right after a start ended "unavailable" left the view "restarting" for
/// good): the start under way finishes, its window closes, and the view starts once more.
#[tokio::test(flavor = "multi_thread")]
async fn a_restart_during_a_start_runs_after_it() {
    let setup = setup(true);
    setup.firefox.hold_starts();
    setup
        .windows
        .sync(vec![sample_spec(EngineKind::Firefox, 4)]);
    until(|| setup.firefox.starts_entered() == 1).await;
    assert_eq!(status(&setup.windows, 4), Some(ExternalStatus::Starting));
    setup
        .windows
        .restart(EngineKind::Firefox, Some(ViewId(4)))
        .expect("restarts");
    setup.firefox.let_starts_go();
    until(|| setup.firefox.views().len() == 2).await;
    until(|| status(&setup.windows, 4) == Some(ExternalStatus::Running)).await;
    let views = setup.firefox.views();
    assert!(
        views[0].is_closed(),
        "the window of the first start is closed"
    );
    assert!(!views[1].is_closed());
    let current = setup.windows.handle(ViewId(4)).expect("running");
    current
        .navigate("http://localhost/x")
        .await
        .expect("the second window is the view's");
    assert_eq!(views[1].url(), "http://localhost/x");
    assert_eq!(
        setup.firefox.starts_entered(),
        2,
        "one start per request, no more"
    );
}
