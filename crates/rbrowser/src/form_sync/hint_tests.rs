use rb_engine::{ViewHost, ViewId};
use serde_json::Value;

use crate::form_sync::commands::skip_next_click_in;
use crate::host::ui::recording::UiCall;
use crate::sync_hub::tests::{setup, until};
use crate::sync_hub::{SKIP_NEXT, for_host, on_start};

fn labels(calls: &[UiCall]) -> Vec<(ViewId, String)> {
    calls
        .iter()
        .filter_map(|call| match call {
            UiCall::CellLabel(view, label, None) => Some((*view, label.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_active_view_s_strip_shows_the_hint_until_the_skip_is_gone() {
    let setup = setup();
    on_start(&setup.core);
    let host = setup.core.host();
    let sync = for_host(host).expect("started");
    host.set_active_view(ViewId(2)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(2))).await;
    let label = host.view(ViewId(2)).expect("view 2").label;
    setup
        .core
        .run_command(SKIP_NEXT, Value::Null)
        .expect("armed");
    until(|| labels(&setup.ui.calls()).len() == 1).await;
    let shown = labels(&setup.ui.calls());
    assert_eq!(shown[0].0, ViewId(2));
    assert_ne!(shown[0].1, label, "the hint, not the plain label");

    // Escape in the active view takes it back: the label returns.
    sync.hub().on_event(ViewId(2), &escape());
    until(|| labels(&setup.ui.calls()).len() == 2).await;
    assert_eq!(labels(&setup.ui.calls())[1], (ViewId(2), label));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_context_menu_entry_skips_the_next_click_of_its_view() {
    let setup = setup();
    on_start(&setup.core);
    let host = setup.core.host();
    host.set_active_view(ViewId(1)).expect("a view");
    skip_next_click_in(&setup.core, ViewId(3)).expect("armed");
    assert_eq!(host.active_view(), Some(ViewId(3)));
    let sync = for_host(host).expect("started");
    assert!(sync.hub().with(|hub| hub.skip_next_armed()));
    assert!(skip_next_click_in(&setup.core, ViewId(9)).is_err());
}

fn escape() -> rb_sync::EventMessage {
    serde_json::from_value(serde_json::json!({
        "v": 1, "top": true, "seq": 1, "time": 0,
        "event": { "type": "keydown", "key": "Escape", "code": "Escape", "keyCode": 27,
                   "location": 0, "repeat": false, "modifiers": 0 },
    }))
    .expect("an event message")
}
