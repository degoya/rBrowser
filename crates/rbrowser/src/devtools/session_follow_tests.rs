//! RB-099-19: the follower that fell behind the host's events reads the views instead.

use rb_engine::{ViewHost, ViewId};

use super::tests::{Setup, attach_views, docks, open_prepared, setup, setup_with, until};
use crate::host::state::{default_config, layout_for};

/// Closes view 1 by a config without its breakpoint: the host's `Closed` event for it comes after
/// every event sent before.
fn close_view_one(setup: &Setup) {
    let mut config = default_config();
    config.breakpoints.remove(0);
    config.layout = rb_config::Layout::default();
    config.layout = layout_for(&config);
    setup.core.host().open_config(config, None).expect("opens");
}

/// More events than the host keeps for a subscriber, sent while the follower cannot run (the
/// test's runtime has one thread): it lags and misses what came before them.
fn flood(setup: &Setup) {
    let host = setup.core.host();
    for n in 0..400 {
        host.on_title_change(ViewId(2), &format!("title {n}"));
    }
}

#[tokio::test]
async fn docked_devtools_follow_an_active_view_the_lag_dropped() {
    let setup = setup();
    attach_views(&setup).await;
    open_prepared(&setup).await;
    setup
        .core
        .host()
        .set_active_view(ViewId(3))
        .expect("activates");
    flood(&setup);
    until(|| setup.devtools.current().map(|open| open.view) == Some(ViewId(3))).await;
}

#[tokio::test]
async fn devtools_of_a_view_the_lag_dropped_the_close_of_close_too() {
    let setup = setup();
    attach_views(&setup).await;
    open_prepared(&setup).await;
    close_view_one(&setup);
    assert!(
        !setup
            .core
            .host()
            .views()
            .iter()
            .any(|info| info.id == ViewId(1)),
        "view 1 is gone"
    );
    flood(&setup);
    until(|| setup.devtools.current().is_none()).await;
}

/// RB-099-18: the close of view 1 is the barrier - the follower takes the host's events in order,
/// so once the DevTools closed with view 1 it had the change of the active view before, and had
/// not followed it.
#[tokio::test]
async fn without_follow_the_devtools_stay_with_their_view() {
    let mut user = rb_config::UserSettings::default();
    user.devtools.follow_active_view = false;
    let setup = setup_with(user);
    attach_views(&setup).await;
    open_prepared(&setup).await;
    setup
        .core
        .host()
        .set_active_view(ViewId(3))
        .expect("activates");
    close_view_one(&setup);
    // Closed with view 1: they were still its DevTools when its close came.
    until(|| setup.devtools.current().is_none()).await;
    assert!(
        docks(&setup.ui)
            .iter()
            .flatten()
            .all(|dock| dock.view == ViewId(1)),
        "no dock for another view"
    );
}
