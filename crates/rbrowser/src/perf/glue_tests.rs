use rb_engine::CellRect;
use rb_sync::RecordingReplay;
use serde_json::{Value, json};

use super::*;
use crate::host::ui::recording::UiCall;
use crate::sync_hub::tests::{Setup, attach_all, emit_from, setup, until};

fn started() -> (Setup, Arc<crate::sync_hub::HostSync>) {
    let setup = setup();
    crate::sync_hub::on_start(&setup.core);
    crate::perf::on_start(&setup.core);
    let sync = crate::sync_hub::for_host(setup.core.host()).expect("started");
    (setup, sync)
}

fn bounds(width: u32) -> CellRect {
    CellRect {
        x: 0,
        y: 0,
        width,
        height: if width == 0 { 0 } else { 300 },
    }
}

fn scroll(y: f64) -> Value {
    json!({ "kind": "event", "v": 1, "top": true, "seq": 3, "time": 0, "event": {
        "type": "scroll", "x": 0.0, "y": y, "maxX": 0.0, "maxY": 2000.0,
    } })
}

fn navigations(setup: &Setup, view: u32, url: &str) -> usize {
    setup
        .ui
        .calls()
        .iter()
        .filter(|call| **call == UiCall::Navigate(ViewId(view), url.to_owned()))
        .count()
}

fn scrolls_into(replay: &RecordingReplay, view: u32) -> Vec<f64> {
    replay
        .calls_for(ViewId(view))
        .into_iter()
        .filter_map(|request| match &*request.event {
            SyncEvent::Scroll(data) => Some(data.y),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_shown_again_on_another_url_loads_the_leaders() {
    let (setup, sync) = started();
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    host.on_address_change(ViewId(1), "http://localhost/products");
    // Navigation sync's own loads of the followers come first.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    // View 3 missed it (a failed load while it was out of sight).
    host.on_address_change(ViewId(3), "http://localhost/old");
    host.on_view_bounds(ViewId(3), bounds(0));
    let before = navigations(&setup, 3, "http://localhost/products");
    host.on_view_bounds(ViewId(3), bounds(400));
    until(|| navigations(&setup, 3, "http://localhost/products") > before).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_view_shown_again_on_the_leaders_url_takes_its_last_scroll() {
    let (setup, sync) = started();
    let replay = Arc::new(RecordingReplay::default());
    sync.set_replay(replay.clone());
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &scroll(400.0));
    // The hub routes it to the followers as usual; the memo keeps it too.
    until(|| scrolls_into(&replay, 3) == vec![400.0]).await;
    host.on_view_bounds(ViewId(3), bounds(0));
    host.on_view_bounds(ViewId(3), bounds(400));
    until(|| scrolls_into(&replay, 3) == vec![400.0, 400.0]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn with_sync_off_a_view_shown_again_is_left_alone() {
    let (setup, sync) = started();
    let replay = Arc::new(RecordingReplay::default());
    sync.set_replay(replay.clone());
    attach_all(&setup).await;
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;
    emit_from(&setup, 1, &scroll(400.0));
    until(|| scrolls_into(&replay, 3) == vec![400.0]).await;
    sync.set_enabled(false);
    host.on_view_bounds(ViewId(3), bounds(0));
    host.on_view_bounds(ViewId(3), bounds(400));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(scrolls_into(&replay, 3), vec![400.0]);
}
