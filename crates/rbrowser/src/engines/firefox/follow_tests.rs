use rb_bidi::testing::FakeFirefox;
use rb_engine::{ViewHost, ViewId};
use serde_json::{Value, json};

use super::*;
use crate::engines::firefox::tests::{attach, started_with};
use crate::engines::tests::until;
use crate::host::ui::recording::UiCall;

fn event(value: Value) -> SyncEvent {
    serde_json::from_value(value).expect("a sync event")
}

fn target() -> Value {
    json!({ "element": { "kind": "css", "path": "#go" }, "offset": { "x": 0.5, "y": 0.5 } })
}

fn context(core: &crate::control::Core) -> String {
    let bridge = crate::engines::firefox::for_host(core.host()).expect("bridge");
    let firefox = bridge.firefox().expect("Firefox runs");
    firefox
        .view_state(ViewId(3))
        .expect("view 3")
        .context
        .clone()
}

#[test]
fn a_press_or_a_click_takes_the_lead() {
    let pointer = |phase: &str| {
        event(
            json!({ "type": "pointer", "phase": phase, "button": "left", "buttons": 1,
                      "pointerType": "mouse", "modifiers": 0, "target": target() }),
        )
    };
    assert!(takes_lead(&pointer("down")));
    assert!(!takes_lead(&pointer("up")));
    assert!(takes_lead(&event(
        json!({ "type": "click", "button": "left", "detail": 1, "modifiers": 0, "target": target() })
    )));
    assert!(!takes_lead(&event(
        json!({ "type": "scroll", "x": 0, "y": 10, "maxX": 0, "maxY": 100 })
    )));
    assert!(!takes_lead(&event(
        json!({ "type": "hover", "target": target() })
    )));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_click_in_the_firefox_window_makes_it_the_leader() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let host = core.host().clone();
    assert_ne!(host.active_view(), Some(ViewId(3)));
    let click = json!({
        "kind": "event", "v": 1, "top": true, "seq": 1, "time": 1,
        "event": { "type": "click", "button": "left", "detail": 1, "modifiers": 0, "target": target() }
    });
    fake.agent_says("rb-agent-3", &context(&core), &click.to_string());
    until(|| host.active_view() == Some(ViewId(3))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn firefox_follows_its_stand_in_and_the_stand_in_follows_firefox() {
    let fake = FakeFirefox::start().await;
    let (core, ui) = started_with(&fake).await;
    let host = core.host().clone();
    let context = context(&core);
    attach(&core, ViewId(3)).await;

    // The grid gives view 3 a URL: the stand-in commits it, Firefox loads it.
    host.on_address_change(ViewId(3), "http://localhost/from-grid");
    until(|| {
        fake.calls("browsingContext.navigate")
            .iter()
            .any(|call| call["url"] == "http://localhost/from-grid")
    })
    .await;

    // Firefox goes somewhere by itself: the stand-in loads it, once.
    fake.emit(
        "browsingContext.domContentLoaded",
        json!({ "context": context, "url": "http://localhost/from-firefox", "navigation": "n", "timestamp": 0 }),
    );
    until(|| {
        ui.calls().iter().any(|call| {
            matches!(call, UiCall::Navigate(ViewId(3), url) if url == "http://localhost/from-firefox")
        })
    })
    .await;
    // The stand-in's commit of Firefox's URL sends nothing back to Firefox.
    let before = fake.calls("browsingContext.navigate").len();
    host.on_address_change(ViewId(3), "http://localhost/from-firefox");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(fake.calls("browsingContext.navigate").len(), before);
}

/// A click in a Firefox view that is out of sync leads nothing.
#[tokio::test(flavor = "multi_thread")]
async fn an_excluded_firefox_view_does_not_lead() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let host = core.host().clone();
    let sync = crate::sync_hub::for_host(&host).expect("sync");
    sync.exclude(ViewId(3), true).expect("excluded");
    let click = json!({
        "kind": "event", "v": 1, "top": true, "seq": 1, "time": 1,
        "event": { "type": "click", "button": "left", "detail": 1, "modifiers": 0, "target": target() }
    });
    fake.agent_says("rb-agent-3", &context(&core), &click.to_string());
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_ne!(host.active_view(), Some(ViewId(3)));
}

/// Taking the lead goes through the host, so the hub reconfigures the agents; the keys the
/// replay held in the Firefox view are let go: the person types now.
#[tokio::test(flavor = "multi_thread")]
async fn taking_the_lead_reaches_the_hub_and_releases_held_keys() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let host = core.host().clone();
    let sync = crate::sync_hub::for_host(&host).expect("sync");
    let click = json!({
        "kind": "event", "v": 1, "top": true, "seq": 1, "time": 1,
        "event": { "type": "click", "button": "left", "detail": 1, "modifiers": 0, "target": target() }
    });
    fake.agent_says("rb-agent-3", &context(&core), &click.to_string());
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(3))).await;
    until(|| !fake.calls("input.releaseActions").is_empty()).await;
}

/// The page zooms the window was asked for, in percent.
fn zooms(ui: &crate::host::ui::recording::RecordingUi) -> Vec<(ViewId, u32)> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::PageZoom(view, level) => Some((view, crate::page_zoom::percent_of(level))),
            _ => None,
        })
        .collect()
}

fn key(key: &str, code: &str, modifiers: u8) -> String {
    // The keyCode Firefox reports for the key, so the code, the key and it agree.
    let key_code = match code {
        "Digit0" => 48,
        "Equal" => 61,
        "Minus" => 173,
        _ => 0,
    };
    json!({
        "kind": "event", "v": 1, "top": true, "seq": 1, "time": 1,
        "event": { "type": "keydown", "key": key, "code": code, "keyCode": key_code, "location": 0,
                   "repeat": false, "modifiers": modifiers }
    })
    .to_string()
}

/// RB-090-69: Firefox zooms its own window on Ctrl+0, Ctrl+Plus and Ctrl+Minus, and BiDi tells
/// nothing of it; the key its agent reports runs the page zoom command for the view, as the
/// same key in a Chromium page does - so `views.pageZoom.sync` reaches the other views.
#[tokio::test(flavor = "multi_thread")]
async fn a_zoom_key_in_the_firefox_window_runs_the_page_zoom_for_its_view() {
    let fake = FakeFirefox::start().await;
    let (core, ui) = started_with(&fake).await;
    let context = context(&core);
    fake.agent_says(
        "rb-agent-3",
        &context,
        &key("+", "Equal", rb_sync::MOD_CTRL),
    );
    until(|| zooms(&ui).contains(&(ViewId(3), 110))).await;
    fake.agent_says(
        "rb-agent-3",
        &context,
        &key("0", "Digit0", rb_sync::MOD_CTRL),
    );
    until(|| zooms(&ui).contains(&(ViewId(3), 100))).await;
    // A key without Ctrl is typing, not a zoom.
    fake.agent_says("rb-agent-3", &context, &key("0", "Digit0", 0));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(zooms(&ui), vec![(ViewId(3), 110), (ViewId(3), 100)]);
}

/// RB-090-69 review: a zoom key needs no sync - as in a Chromium page, it zooms with sync off
/// and in a view out of sync.
#[tokio::test(flavor = "multi_thread")]
async fn a_zoom_key_in_firefox_zooms_with_sync_off_and_in_an_excluded_view() {
    let fake = FakeFirefox::start().await;
    let (core, ui) = started_with(&fake).await;
    let context = context(&core);
    let sync = crate::sync_hub::for_host(core.host()).expect("sync");
    sync.set_enabled(false);
    fake.agent_says(
        "rb-agent-3",
        &context,
        &key("+", "Equal", rb_sync::MOD_CTRL),
    );
    until(|| zooms(&ui).contains(&(ViewId(3), 110))).await;
    sync.set_enabled(true);
    sync.exclude(ViewId(3), true).expect("excluded");
    fake.agent_says(
        "rb-agent-3",
        &context,
        &key("0", "Digit0", rb_sync::MOD_CTRL),
    );
    until(|| zooms(&ui).contains(&(ViewId(3), 100))).await;
}
