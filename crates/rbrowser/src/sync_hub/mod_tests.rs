use std::time::Duration;

use rb_engine::CellRect;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// A window with the default config's three views (1 desktop, 2 tablet, 3 mobile).
pub(crate) struct Setup {
    pub(crate) fake: Arc<FakeBrowser>,
    pub(crate) ui: Arc<RecordingUi>,
    pub(crate) core: Arc<Core>,
}

pub(crate) fn setup() -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    Setup { fake, ui, core }
}

/// Lets the spawned tasks run until `done` holds (or fails the test after a second).
pub(crate) async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

/// Attaches views 1 to 3 in order - sessions `S1` to `S3` - each with its agent's world as
/// context `10 + id` in frame `T<id>`.
pub(crate) async fn attach_all(setup: &Setup) {
    let host = setup.core.host();
    for id in 1..=3_u32 {
        let view = ViewId(id);
        host.on_view_bounds(
            view,
            CellRect {
                x: 0,
                y: 0,
                width: 400,
                height: 300,
            },
        );
        host.on_load_end(view, &marker_url(view));
        until(|| host.sync_agents().config(view).is_some()).await;
        setup.fake.emit(json!({
            "method": "Runtime.executionContextCreated",
            "sessionId": format!("S{id}"),
            "params": { "context": {
                "id": 10 + i64::from(id), "name": "rbrowser", "auxData": { "frameId": format!("T{id}") },
            } },
        }));
    }
}

/// The agent message `payload` from view `id`'s top frame.
pub(crate) fn emit_from(setup: &Setup, id: u32, payload: &Value) {
    setup.fake.emit(json!({
        "method": "Runtime.bindingCalled",
        "sessionId": format!("S{id}"),
        "params": {
            "name": "__rbEmit", "payload": payload.to_string(), "executionContextId": 10 + i64::from(id),
        },
    }));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hub_starts_once_per_window() {
    let setup = setup();
    assert!(for_host(setup.core.host()).is_none());
    on_start(&setup.core);
    let first = for_host(setup.core.host()).expect("started");
    on_start(&setup.core);
    let second = for_host(setup.core.host()).expect("still there");
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test(flavor = "multi_thread")]
async fn before_the_hub_runs_only_the_active_view_captures() {
    let setup = setup();
    let host = setup.core.host();
    host.set_active_view(ViewId(2)).expect("a view");
    assert!(agent_config_for(host, ViewId(2)).active);
    assert!(!agent_config_for(host, ViewId(1)).active);
}

#[test]
fn the_switches_start_from_the_project_s_sync() {
    let sync = SyncConfig {
        hover: false,
        leader_only_submit: false,
        ..SyncConfig::default()
    };
    let switches = switches_from(&sync);
    assert!(switches.enabled && switches.click);
    assert!(!switches.hover && !switches.leader_only_submit);
}
