use std::time::Duration;

use rb_engine::external::ExternalView;
use rb_ipc::ViewMetrics;
use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup, view_ids};

fn lcp(view: ViewId) -> ViewMetrics {
    serde_json::from_value(json!({
        "view": view.0,
        "vitals": [{ "metric": "lcp", "value": 1200.0, "rating": "good" }]
    }))
    .expect("metrics")
}

/// The pieces the window got last.
fn last_infos(calls: &[UiCall]) -> Option<Vec<CellInfo>> {
    calls.iter().rev().find_map(|call| match call {
        UiCall::CellInfo(infos) => Some(infos.clone()),
        _ => None,
    })
}

fn of(infos: &[CellInfo], view: ViewId) -> CellInfo {
    infos
        .iter()
        .find(|info| info.view == view)
        .cloned()
        .expect("the view's pieces")
}

fn closed(view: ViewId) -> ExternalView {
    serde_json::from_value(json!({
        "view": view.0, "label": "Tablet", "breakpoint": "tablet", "engine": "firefox",
        "status": "closed", "url": "http://localhost:3000/"
    }))
    .expect("external view")
}

#[tokio::test]
async fn the_window_gets_the_pieces_and_the_host_the_vitals_bars() {
    // RB-090-71: the vitals left the label strip for the bar below the breakpoint bar.
    let setup = setup(config());
    let ids = view_ids(&setup);
    let host = setup.core.host();
    host.update_shell_state(|shell| shell.metrics = vec![lcp(ids[1])]);
    publish(&setup.core);
    let infos = last_infos(&setup.ui.calls()).expect("the pieces");
    assert_eq!(infos.len(), ids.len(), "one entry per view");
    assert!(host.footer_vitals_of(ids[1]).is_some());
    assert_eq!(host.footer_vitals_of(ids[0]), None, "no vitals there");
}

#[tokio::test]
async fn the_engine_state_follows_the_engine_badge_setting() {
    let setup = setup(config());
    let ids = view_ids(&setup);
    setup
        .core
        .host()
        .update_shell_state(|shell| shell.externals = vec![closed(ids[1])]);
    publish(&setup.core);
    let infos = last_infos(&setup.ui.calls()).expect("the pieces");
    assert!(of(&infos, ids[1]).engine_in_badge, "one badge by default");
    let mut user = setup.core.user_settings();
    user.ui.engine_badge = false;
    setup.core.replace_user_settings(user);
    let infos = last_infos(&setup.ui.calls()).expect("the settings change publishes");
    let apart = of(&infos, ids[1]);
    assert!(!apart.engine_in_badge, "no badge: a piece of its own");
    assert_eq!(
        apart.engine.map(|piece| piece.text),
        Some("Firefox: window closed".to_owned())
    );
}

#[tokio::test]
async fn a_change_of_the_shell_state_reaches_the_window_after_the_start() {
    let setup = setup(config());
    let ids = view_ids(&setup);
    on_start(&setup.core);
    assert!(
        last_infos(&setup.ui.calls()).is_some(),
        "the start publishes"
    );
    setup
        .core
        .host()
        .update_shell_state(|shell| shell.metrics = vec![lcp(ids[2])]);
    // A give-up deadline, not a measure.
    let host = setup.core.host().clone();
    let reached = tokio::time::timeout(Duration::from_secs(10), async move {
        loop {
            if host.footer_vitals_of(ids[2]).is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(reached.is_ok(), "the vitals reach the vitals bar");
}
