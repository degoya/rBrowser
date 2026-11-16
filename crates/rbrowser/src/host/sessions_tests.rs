use std::sync::Arc;
use std::time::Duration;

use rb_config::ProjectConfig;
use rb_engine::{Target, ViewHost, ViewId};
use serde_json::json;
use tokio::runtime::Handle;

use super::super::Host;
use super::super::nav::marker_url;
use super::super::ui::recording::{RecordingUi, UiCall};
use crate::cdp::fake::FakeBrowser;
use crate::sessions::{Moved, SessionBadge, auto_color};

/// Desktop in `default`, mobile in `admin`; `qa` is listed but unused.
fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "https://shop.test/",
        "sessions": ["default", "admin", "qa"],
        "sessionColors": { "admin": "#e5484d" },
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "mobile", "width": 393, "height": 852, "session": "admin" }
        ],
        "layout": { "columns": 3, "rows": 1, "cells": [
            { "breakpoint": "desktop", "col": 1, "row": 1 },
            { "breakpoint": "tablet", "col": 2, "row": 1 },
            { "breakpoint": "mobile", "col": 3, "row": 1 }
        ] }
    }))
    .expect("a config")
}

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    host: Arc<Host>,
}

fn setup() -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config(), None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    Setup { fake, ui, host }
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

/// Attaches `view` and lets it show `url`.
async fn attach(setup: &Setup, view: ViewId, url: &str) {
    setup.host.on_load_end(view, &marker_url(view));
    let attaches = setup.fake.sent("Target.attachToTarget").len();
    until(|| setup.fake.sent("Target.attachToTarget").len() > attaches).await;
    until(|| {
        setup
            .host
            .state()
            .views
            .get(&view)
            .is_some_and(|e| e.session.is_some())
    })
    .await;
    setup.host.on_address_change(view, url);
}

#[tokio::test]
async fn every_cell_is_created_in_its_breakpoints_session() {
    let setup = setup();
    let grid = setup.ui.last_grid().expect("a grid");
    let sessions: Vec<&str> = grid.cells.iter().map(|c| c.session.as_str()).collect();
    assert_eq!(sessions, ["default", "default", "admin"]);
    assert_eq!(grid.cells[0].badge, None);
    assert_eq!(
        grid.cells[2].badge,
        Some(SessionBadge {
            name: "admin".to_owned(),
            rgb: 0xe5_48_4d
        })
    );
    let infos = setup.host.views();
    assert_eq!(infos[2].session, "admin");
    assert_eq!(setup.host.session_views("admin"), [ViewId(3)]);
    assert_eq!(setup.host.session_views("default"), [ViewId(1), ViewId(2)]);
}

#[tokio::test]
async fn assigning_replaces_the_view_in_its_cell_and_keeps_the_url() {
    let setup = setup();
    attach(&setup, ViewId(2), "https://shop.test/login").await;
    setup.host.set_active_view(ViewId(2)).expect("active");
    let moved = setup
        .host
        .assign_session(Target::View(ViewId(2)), "qa")
        .expect("moves");
    assert_eq!(
        moved,
        [Moved {
            from: ViewId(2),
            view: ViewId(4)
        }]
    );
    let grid = setup.ui.last_grid().expect("a grid");
    let views: Vec<ViewId> = grid.cells.iter().map(|c| c.view).collect();
    assert_eq!(views, [ViewId(1), ViewId(4), ViewId(3)]);
    assert_eq!(grid.cells[1].session, "qa");
    assert_eq!(grid.cells[1].initial_url, marker_url(ViewId(4)));
    assert_eq!(
        grid.cells[1].badge.as_ref().map(|b| b.rgb),
        Some(auto_color("qa"))
    );
    assert_eq!(setup.host.view_session(ViewId(4)).expect("known"), "qa");
    assert!(setup.host.view_session(ViewId(2)).is_err());
    {
        let state = setup.host.state();
        assert_eq!(state.active, Some(ViewId(4)));
        let entry = state.views.get(&ViewId(4)).expect("the new view");
        assert_eq!(
            entry.pending_url.as_deref(),
            Some("https://shop.test/login")
        );
        assert_eq!(entry.cell_breakpoint.as_deref(), Some("tablet"));
        assert!(entry.session.is_none());
    }
    // Once attached, the new view loads the old URL.
    setup.host.on_load_end(ViewId(4), &marker_url(ViewId(4)));
    until(|| {
        setup.ui.calls().contains(&UiCall::Navigate(
            ViewId(4),
            "https://shop.test/login".to_owned(),
        ))
    })
    .await;
}

#[tokio::test]
async fn a_view_already_in_the_session_stays() {
    let setup = setup();
    let grids = setup.ui.calls().len();
    let moved = setup
        .host
        .assign_session(Target::View(ViewId(3)), "admin")
        .expect("nothing to do");
    assert!(moved.is_empty());
    assert_eq!(setup.ui.calls().len(), grids);
}

#[tokio::test]
async fn all_views_move_into_default() {
    let setup = setup();
    let moved = setup
        .host
        .assign_session(Target::All, "default")
        .expect("moves");
    assert_eq!(
        moved,
        [Moved {
            from: ViewId(3),
            view: ViewId(4)
        }]
    );
    let grid = setup.ui.last_grid().expect("a grid");
    assert!(
        grid.cells
            .iter()
            .all(|c| c.session == "default" && c.badge.is_none())
    );
}

#[tokio::test]
async fn unknown_sessions_and_views_are_refused() {
    let setup = setup();
    assert!(
        setup
            .host
            .assign_session(Target::View(ViewId(1)), "ghost")
            .is_err()
    );
    assert!(
        setup
            .host
            .assign_session(Target::View(ViewId(1)), "../admin")
            .is_err()
    );
    assert!(
        setup
            .host
            .assign_session(Target::View(ViewId(99)), "admin")
            .is_err()
    );
    assert!(setup.host.clear_session("ghost").is_err());
}

#[tokio::test]
async fn a_hand_chosen_session_survives_a_config_change() {
    let setup = setup();
    setup
        .host
        .assign_session(Target::View(ViewId(1)), "qa")
        .expect("moves");
    // The config moves tablet into admin and mobile back into default.
    let mut changed = config();
    changed.breakpoints[1].session = Some("admin".to_owned());
    changed.breakpoints[2].session = None;
    setup.host.open_config(changed, None).expect("reopens");
    let moved = setup.host.follow_config_sessions().expect("moves");
    assert_eq!(
        moved,
        [
            Moved {
                from: ViewId(2),
                view: ViewId(5)
            },
            Moved {
                from: ViewId(3),
                view: ViewId(6)
            }
        ]
    );
    let grid = setup.ui.last_grid().expect("a grid");
    let sessions: Vec<&str> = grid.cells.iter().map(|c| c.session.as_str()).collect();
    assert_eq!(sessions, ["qa", "admin", "default"]);
}

#[tokio::test]
async fn clearing_a_session_touches_only_its_views() {
    let setup = setup();
    attach(&setup, ViewId(1), "https://shop.test/account").await;
    attach(&setup, ViewId(3), "https://shop.test/admin?tab=1").await;
    let plan = setup.host.clear_session("admin").expect("clears");
    assert_eq!(plan.views, [ViewId(3)]);
    assert_eq!(plan.origins, ["https://shop.test"]);
    until(|| setup.ui.calls().contains(&UiCall::Reload(ViewId(3), true))).await;
    assert!(
        setup
            .ui
            .calls()
            .contains(&UiCall::ClearCookies("admin".to_owned()))
    );
    let cleared = setup.fake.sent("Storage.clearDataForOrigin");
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0]["params"]["origin"], "https://shop.test");
    assert_eq!(cleared[0]["params"]["storageTypes"], "all");
    let admin_cdp = setup
        .host
        .state()
        .views
        .get(&ViewId(3))
        .and_then(|entry| entry.session.clone());
    assert_eq!(cleared[0]["sessionId"].as_str(), admin_cdp.as_deref());
    assert!(!setup.ui.calls().contains(&UiCall::Reload(ViewId(1), true)));
}

#[tokio::test]
async fn a_moved_view_leaves_no_agent_session_behind() {
    let setup = setup();
    attach(&setup, ViewId(2), "https://shop.test/login").await;
    until(|| setup.host.sync_agents().views().contains(&ViewId(2))).await;
    setup
        .host
        .assign_session(Target::View(ViewId(2)), "qa")
        .expect("moves");
    assert!(!setup.host.sync_agents().views().contains(&ViewId(2)));
}

#[tokio::test]
async fn a_failed_move_changes_nothing() {
    let setup = setup();
    // The grid names `desktop`, which the config no longer has: the rebuild fails.
    setup
        .host
        .state()
        .config
        .breakpoints
        .retain(|breakpoint| breakpoint.id != "desktop");
    let grids = setup.ui.calls().len();
    assert!(
        setup
            .host
            .assign_session(Target::View(ViewId(2)), "qa")
            .is_err()
    );
    let state = setup.host.state();
    assert_eq!(state.order, [ViewId(1), ViewId(2), ViewId(3)]);
    assert!(state.views.contains_key(&ViewId(2)));
    assert!(!state.views.contains_key(&ViewId(4)));
    assert_eq!(state.next_id, 3);
    drop(state);
    assert_eq!(setup.ui.calls().len(), grids);
    assert_eq!(setup.host.view_session(ViewId(2)).expect("kept"), "default");
}
