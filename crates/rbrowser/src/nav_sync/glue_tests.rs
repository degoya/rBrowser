use std::time::Duration;

use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::nav_sync::tests::{started, views_of};
use crate::nav_sync::{for_host, status};
use crate::sync_hub::tests::{Setup, attach_all, emit_from, until};

fn navigations_to(setup: &Setup, url: &str) -> Vec<ViewId> {
    views_of(setup, |call| match call {
        UiCall::Navigate(view, to) if to == url => Some(*view),
        _ => None,
    })
}

/// Attached views, view 1 the active one, known to the hub.
async fn leading(setup: &Setup) {
    attach_all(setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    let sync = sync_hub::for_host(host).expect("started");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
}

/// K1: the mobile layout's link led elsewhere; after the settle time it shows the leader's URL.
#[tokio::test(flavor = "multi_thread")]
async fn a_follower_elsewhere_loads_the_leader_s_url_after_the_settle_time() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    host.on_address_change(ViewId(3), "http://localhost/m/produkt");
    host.on_address_change(ViewId(2), "http://localhost/produkt");
    host.on_address_change(ViewId(1), "http://localhost/produkt");
    until(|| navigations_to(&setup, "http://localhost/produkt") == vec![ViewId(3)]).await;
    let sync = sync_hub::for_host(host).expect("started");
    assert!(
        sync.hub()
            .with(|hub| hub.is_loading(ViewId(3), std::time::Instant::now())),
        "no events until it commits"
    );
    // Its commit is no navigation of the leader: nothing follows it.
    host.on_address_change(ViewId(3), "http://localhost/produkt");
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        navigations_to(&setup, "http://localhost/produkt"),
        vec![ViewId(3)]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_follower_s_own_navigation_leads_nobody() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    host.on_address_change(ViewId(1), "http://localhost/a");
    host.on_address_change(ViewId(2), "http://localhost/a");
    host.on_address_change(ViewId(3), "http://localhost/a");
    host.on_address_change(ViewId(2), "http://localhost/elsewhere");
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(navigations_to(&setup, "http://localhost/elsewhere").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_navigation_sync_nobody_follows() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    let sync = sync_hub::for_host(host).expect("started");
    sync.set_kind(rb_sync::SyncKind::Navigation, false);
    host.on_address_change(ViewId(2), "http://localhost/m");
    host.on_address_change(ViewId(1), "http://localhost/d");
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(navigations_to(&setup, "http://localhost/d").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decoupled_view_keeps_its_page() {
    let setup = started(&["mobile"]);
    leading(&setup).await;
    let host = setup.core.host();
    host.on_address_change(ViewId(2), "http://localhost/");
    host.on_address_change(ViewId(3), "http://localhost/settings");
    host.on_address_change(ViewId(1), "http://localhost/next");
    until(|| navigations_to(&setup, "http://localhost/next") == vec![ViewId(2)]).await;
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        navigations_to(&setup, "http://localhost/next"),
        vec![ViewId(2)]
    );
}

/// K2: a SPA route moves through the agent's history API; when the agent cannot, it loads.
#[tokio::test(flavor = "multi_thread")]
async fn a_spa_route_follows_through_the_agent_first() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    for id in [1, 2, 3] {
        host.on_address_change(ViewId(id), "http://localhost/");
    }
    // The check of the first page has run.
    tokio::time::sleep(Duration::from_millis(100)).await;
    setup.fake.emit(json!({
        "method": "Page.navigatedWithinDocument",
        "sessionId": "S1",
        "params": { "frameId": "T1", "url": "http://localhost/about", "navigationType": "historyApi" },
    }));
    host.on_address_change(ViewId(1), "http://localhost/about");
    let evaluated = || {
        setup
            .fake
            .sent("Runtime.evaluate")
            .iter()
            .filter(|call| call["sessionId"] == "S2")
            .filter_map(|call| call["params"]["expression"].as_str().map(str::to_owned))
            .any(|expression| {
                expression.contains(r#""cmd":"navigate""#)
                    && expression.contains("http://localhost/about")
            })
    };
    until(evaluated).await;
    // The fake answers no `done`: the follower loads the URL instead.
    until(|| navigations_to(&setup, "http://localhost/about").contains(&ViewId(2))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_document_is_never_followed_through_the_history_api() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    for id in [1, 2, 3] {
        host.on_address_change(ViewId(id), "http://localhost/");
    }
    // The check of the first page has run.
    tokio::time::sleep(Duration::from_millis(100)).await;
    setup.fake.emit(json!({
        "method": "Page.navigatedWithinDocument", "sessionId": "S1",
        "params": { "frameId": "T1", "url": "http://localhost/about" },
    }));
    setup.fake.emit(json!({
        "method": "Page.frameNavigated", "sessionId": "S1",
        "params": { "frame": { "id": "T1", "url": "http://localhost/about" } },
    }));
    host.on_address_change(ViewId(1), "http://localhost/about");
    until(|| navigations_to(&setup, "http://localhost/about").len() == 2).await;
    let commands = setup
        .fake
        .sent("Runtime.evaluate")
        .iter()
        .filter_map(|call| call["params"]["expression"].as_str().map(str::to_owned))
        .filter(|expression| expression.contains(r#""cmd":"navigate""#))
        .count();
    assert_eq!(commands, 0);
}

fn detected() -> serde_json::Value {
    json!({ "kind": "detected", "v": 1, "top": true, "tool": "browsersync" })
}

/// K5: Browsersync switches sync off; switching it on again is "sync anyway" for the site.
#[tokio::test(flavor = "multi_thread")]
async fn browsersync_switches_sync_off_until_the_person_syncs_anyway() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    let sync = sync_hub::for_host(host).expect("started");
    host.on_address_change(ViewId(2), "http://localhost:3000/");
    emit_from(&setup, 2, &detected());
    until(|| !sync.hub().with(|hub| hub.switches().enabled)).await;
    assert_eq!(
        status(host).browsersync.as_deref(),
        Some("http://localhost:3000")
    );
    sync.set_enabled(true);
    emit_from(&setup, 2, &detected());
    until(|| status(host).browsersync.is_none()).await;
    emit_from(&setup, 2, &detected());
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(sync.hub().with(|hub| hub.switches().enabled), "allowed");
}

#[tokio::test(flavor = "multi_thread")]
async fn browsersync_is_ignored_with_detection_off() {
    let setup = started(&[]);
    leading(&setup).await;
    let host = setup.core.host();
    let mut config = host.config();
    config.sync.detect_browsersync = false;
    host.open_config(config, None).expect("reopens");
    for_host(host).expect("started").config_changed();
    host.on_address_change(ViewId(2), "http://localhost:3000/");
    emit_from(&setup, 2, &detected());
    tokio::time::sleep(Duration::from_millis(60)).await;
    let sync = sync_hub::for_host(host).expect("started");
    assert!(sync.hub().with(|hub| hub.switches().enabled));
    assert_eq!(status(host).browsersync, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn coupling_a_view_reaches_the_status_and_needs_a_known_view() {
    let setup = started(&[]);
    let host = setup.core.host();
    let nav = for_host(host).expect("started");
    assert_eq!(nav.couple(ViewId(2), false).ok(), Some(true));
    assert_eq!(nav.couple(ViewId(2), false).ok(), Some(false));
    assert_eq!(status(host).decoupled, vec![ViewId(2)]);
    assert!(matches!(
        nav.couple(ViewId(9), false),
        Err(EngineError::UnknownView(_))
    ));
    // A config change drops the runtime change.
    nav.config_changed();
    assert!(status(host).decoupled.is_empty());
}
