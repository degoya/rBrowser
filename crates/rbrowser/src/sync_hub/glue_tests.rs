use rb_sync::{DesyncReason, RecordingReplay, ReplayOutcome};
use serde_json::{Value, json};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::sync_hub::tests::{Setup, attach_all, emit_from, setup, until};
use crate::sync_hub::{for_host, on_start};

fn started() -> (Setup, Arc<HostSync>) {
    let setup = setup();
    on_start(&setup.core);
    let sync = for_host(setup.core.host()).expect("started");
    (setup, sync)
}

/// The `configure` commands sent to session `S<id>`, in order.
fn configures(setup: &Setup, id: u32) -> Vec<String> {
    setup
        .fake
        .sent("Runtime.evaluate")
        .iter()
        .filter(|call| call["sessionId"] == format!("S{id}"))
        .filter_map(|call| call["params"]["expression"].as_str().map(str::to_owned))
        .filter(|expression| expression.contains(r#""cmd":"configure""#))
        .collect()
}

fn active_in_last_configure(setup: &Setup, id: u32) -> Option<bool> {
    configures(setup, id)
        .last()
        .map(|expression| expression.contains(r#""active":true"#))
}

fn click(path: &str) -> Value {
    json!({ "kind": "event", "v": 1, "top": true, "seq": 1, "time": 0, "event": {
        "type": "click", "button": "left", "detail": 1, "modifiers": 0,
        "target": { "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } },
    } })
}

fn submit() -> Value {
    json!({ "kind": "event", "v": 1, "top": true, "seq": 2, "time": 0, "event": {
        "type": "submit", "method": "post", "action": "http://localhost/form",
        "target": { "element": { "kind": "css", "path": "form" }, "offset": { "x": 0.5, "y": 0.5 } },
    } })
}

fn desynced(setup: &Setup) -> Vec<(ViewId, DesyncReason)> {
    setup
        .core
        .host()
        .shell_state()
        .sync
        .map(|state| {
            state
                .desynced
                .iter()
                .map(|desync| (desync.view, desync.reason))
                .collect()
        })
        .unwrap_or_default()
}

/// Review finding 1 (W1c): a new active view, and the old one, reach the agents.
#[tokio::test(flavor = "multi_thread")]
async fn the_active_view_change_reaches_the_old_and_the_new_agent() {
    let (setup, _sync) = started();
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(2)).expect("a view");
    until(|| active_in_last_configure(&setup, 2) == Some(true)).await;
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| {
        active_in_last_configure(&setup, 1) == Some(true)
            && active_in_last_configure(&setup, 2) == Some(false)
    })
    .await;
    assert_eq!(
        active_in_last_configure(&setup, 3),
        None,
        "unchanged: nothing sent"
    );
}

/// Review finding 1 (W1c): a `sync.*` change reaches the agents live.
#[tokio::test(flavor = "multi_thread")]
async fn a_sync_config_change_reaches_the_agents_live() {
    let (setup, sync) = started();
    attach_all(&setup).await;
    let host = setup.core.host();
    let mut config = host.config();
    config.sync.hover = false;
    host.open_config(config, None).expect("reopens");
    sync.config_changed();
    for id in 1..=3 {
        until(|| {
            configures(&setup, id)
                .last()
                .is_some_and(|expression| expression.contains(r#""hover":false"#))
        })
        .await;
    }
    assert!(!host.shell_state().sync.expect("published").features.hover);
}

/// K2 with the test double: the mobile view misses the desktop menu item.
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_target_shows_the_badge_within_a_second() {
    let (setup, sync) = started();
    let replay = Arc::new(RecordingReplay::default());
    replay.answer(ViewId(3), ReplayOutcome::NotFound);
    sync.set_replay(replay.clone());
    attach_all(&setup).await;
    setup
        .core
        .host()
        .set_active_view(ViewId(1))
        .expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &click("nav > a.products"));
    until(|| desynced(&setup) == vec![(ViewId(3), DesyncReason::NotFound)]).await;
    assert_eq!(replay.calls_for(ViewId(2)).len(), 1, "the tablet clicked");
    let state = setup.core.host().shell_state().sync.expect("published");
    assert_eq!(
        state.desynced[0].selector.as_deref(),
        Some("nav > a.products")
    );
}

/// K4 with the test double: one submit in the leader, the others load its URL.
#[tokio::test(flavor = "multi_thread")]
async fn the_followers_load_the_leader_s_url_after_its_submit() {
    let (setup, sync) = started();
    let replay = Arc::new(RecordingReplay::default());
    sync.set_replay(replay.clone());
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &click("form > button"));
    emit_from(&setup, 1, &submit());
    until(|| replay.calls().len() == 2).await;
    assert!(replay.calls().iter().all(|request| request.block_submit));
    host.on_address_change(ViewId(1), "http://localhost/thanks");
    until(|| {
        let calls = setup.ui.calls();
        [2, 3].iter().all(|id| {
            calls.contains(&UiCall::Navigate(
                ViewId(*id),
                "http://localhost/thanks".into(),
            ))
        })
    })
    .await;
    assert!(
        !setup.ui.calls().contains(&UiCall::Navigate(
            ViewId(1),
            "http://localhost/thanks".into()
        )),
        "the leader is not loaded again"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn resync_loads_the_leader_s_url_and_clears_the_badge() {
    let (setup, sync) = started();
    let replay = Arc::new(RecordingReplay::default());
    replay.answer(ViewId(3), ReplayOutcome::NotVisible);
    sync.set_replay(replay);
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    host.on_address_change(ViewId(1), "http://localhost/products");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &click("button.menu"));
    until(|| !desynced(&setup).is_empty()).await;
    sync.resync(ViewId(3)).expect("known");
    assert!(desynced(&setup).is_empty());
    assert!(setup.ui.calls().contains(&UiCall::Navigate(
        ViewId(3),
        "http://localhost/products".into()
    )));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_excluded_view_s_agent_does_not_capture_and_attach_asks_the_hub() {
    let (setup, sync) = started();
    let host = setup.core.host();
    host.set_active_view(ViewId(2)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(2))).await;
    assert!(crate::sync_hub::agent_config_for(host, ViewId(2)).active);
    sync.exclude(ViewId(2), true).expect("known");
    assert!(!crate::sync_hub::agent_config_for(host, ViewId(2)).active);
    assert_eq!(
        host.shell_state().sync.expect("published").excluded,
        vec![ViewId(2)]
    );
    sync.set_enabled(false);
    sync.exclude(ViewId(2), false).expect("known");
    assert!(
        !crate::sync_hub::agent_config_for(host, ViewId(2)).active,
        "sync is off"
    );
}

/// Review batch 3, finding 1: a resync to the URL the view already shows commits no new
/// address; the end of its load gives it events again.
#[tokio::test(flavor = "multi_thread")]
async fn a_resync_to_the_url_the_view_shows_takes_events_after_its_load() {
    let (setup, sync) = started();
    let replay = Arc::new(RecordingReplay::default());
    replay.answer(ViewId(3), ReplayOutcome::NotVisible);
    sync.set_replay(replay.clone());
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    for id in 1..=3 {
        host.on_address_change(ViewId(id), "http://localhost/products");
    }
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &click("button.menu"));
    until(|| !desynced(&setup).is_empty()).await;
    sync.resync(ViewId(3)).expect("known");
    let loading = |sync: &HostSync| {
        sync.hub()
            .with(|hub| hub.is_loading(ViewId(3), Instant::now()))
    };
    assert!(loading(&sync));
    host.on_loading_state(ViewId(3), true);
    host.on_loading_state(ViewId(3), false);
    until(|| !loading(&sync)).await;
    replay.answer(ViewId(3), ReplayOutcome::Done);
    emit_from(&setup, 1, &click("button.menu"));
    until(|| replay.calls_for(ViewId(3)).len() == 2).await;
}

/// Review batch 3, finding 1: a resync that cannot load - the leader shows no URL, or one a
/// view may not open - leaves the view taking events.
#[tokio::test(flavor = "multi_thread")]
async fn a_resync_that_loads_nothing_leaves_the_view_in_sync() {
    let (setup, sync) = started();
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    let loading = |sync: &HostSync| {
        sync.hub()
            .with(|hub| hub.is_loading(ViewId(3), Instant::now()))
    };
    host.on_address_change(ViewId(1), "javascript:void(0)");
    sync.resync(ViewId(3)).expect("known");
    assert!(!loading(&sync), "a URL no view may open");
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::Navigate(ViewId(3), _)))
    );
}

/// Review batch 3, finding 6: a form posted to its own page commits no new URL; the end of
/// the leader's load has the followers load it.
#[tokio::test(flavor = "multi_thread")]
async fn the_followers_load_the_leader_s_url_after_a_submit_to_the_same_page() {
    let (setup, sync) = started();
    sync.set_replay(Arc::new(RecordingReplay::default()));
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    host.on_address_change(ViewId(1), "http://localhost/form");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &submit());
    until(|| {
        sync.hub()
            .with(|hub| hub.status().switches.leader_only_submit)
    })
    .await;
    // The submit reaches the hub before the leader's load ends.
    until(|| sync.hub().with(|hub| hub.follow_leader()) == Some(ViewId(1))).await;
    host.on_loading_state(ViewId(1), true);
    host.on_address_change(ViewId(1), "http://localhost/form");
    host.on_loading_state(ViewId(1), false);
    until(|| {
        let calls = setup.ui.calls();
        [2, 3].iter().all(|id| {
            calls.contains(&UiCall::Navigate(
                ViewId(*id),
                "http://localhost/form".into(),
            ))
        })
    })
    .await;
}

/// Review batch 3, finding 10: `settled` waits for the agents' configuration and for the
/// loads the hub gave, at most its timeout.
#[tokio::test(flavor = "multi_thread")]
async fn settled_waits_for_the_configuration_and_the_loads() {
    let (setup, sync) = started();
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    host.on_address_change(ViewId(1), "http://localhost/products");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    sync.set_kind(rb_sync::SyncKind::Hover, false);
    sync.configured().await;
    for id in 1..=3 {
        assert!(
            configures(&setup, id)
                .last()
                .is_some_and(|expression| expression.contains(r#""hover":false"#)),
            "{id} configured before the answer"
        );
    }
    sync.resync(ViewId(3)).expect("known");
    let started = Instant::now();
    sync.settled(Duration::from_millis(150)).await;
    assert!(
        started.elapsed() >= Duration::from_millis(150),
        "still loading"
    );
    let waiting = {
        let sync = sync.clone();
        tokio::spawn(async move { sync.settled(Duration::from_secs(5)).await })
    };
    host.on_loading_state(ViewId(3), true);
    host.on_loading_state(ViewId(3), false);
    tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .expect("settled once loaded")
        .expect("ran");
}
