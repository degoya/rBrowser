//! The page API through the host: the attach, a breakpoint change, the user-agent suffix and a
//! bus message from one view to the others, against the fake browser. The bundles are not
//! needed: the agents' sessions and contexts are tracked either way.

use std::sync::Arc;
use std::time::Duration;

use rb_config::{Breakpoint, ProjectConfig};
use rb_engine::{CellRect, ViewHost, ViewId};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::cdp::fake::{FAKE_USER_AGENT, FakeBrowser};
use crate::host::Host;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::sync_agent::ua::UA_PRODUCT;

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    host: Arc<Host>,
}

fn config(extra: Value) -> ProjectConfig {
    let mut config = json!({
        "version": 1,
        "startUrl": "about:blank",
        "breakpoints": [
            { "id": "desktop", "label": "Desktop", "width": 1440, "height": 900 },
            { "id": "mobile", "label": "Mobile", "width": 393, "height": 852, "touch": true }
        ]
    });
    if let (Some(config), Some(extra)) = (config.as_object_mut(), extra.as_object()) {
        config.extend(extra.clone());
    }
    serde_json::from_value(config).expect("a config")
}

fn setup(config: ProjectConfig) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, None).expect("opens");
    fake.add_target("T1", &marker_url(ViewId(1)));
    fake.add_target("T2", &marker_url(ViewId(2)));
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

/// Lays out and attaches view `id` (session `S<id>`, target `T<id>`) and gives its agent the
/// context `<id>0`.
async fn attach(setup: &Setup, id: u32) {
    let view = ViewId(id);
    setup.host.on_view_bounds(
        view,
        CellRect {
            x: 0,
            y: 44,
            width: 400,
            height: 800,
        },
    );
    setup.host.on_load_end(view, &marker_url(view));
    let session = format!("S{id}");
    // The pending URL loads once the agent and the page API are attached.
    until(|| {
        setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::Navigate(navigated, _) if *navigated == view))
    })
    .await;
    setup.fake.emit(json!({
        "method": "Runtime.executionContextCreated",
        "sessionId": session,
        "params": { "context": { "id": i64::from(id) * 10, "name": "rbrowser", "auxData": { "frameId": format!("T{id}") } } },
    }));
}

fn binding(id: u32, payload: &str) -> Value {
    json!({
        "method": "Runtime.bindingCalled",
        "sessionId": format!("S{id}"),
        "params": { "name": "__rbEmit", "payload": payload, "executionContextId": i64::from(id) * 10 },
    })
}

fn evaluated(fake: &FakeBrowser, context: i64, needle: &str) -> Vec<String> {
    fake.sent("Runtime.evaluate")
        .into_iter()
        .filter(|call| call["params"]["contextId"] == context)
        .filter_map(|call| call["params"]["expression"].as_str().map(str::to_owned))
        .filter(|text| text.contains(needle))
        .collect()
}

const READY: &str = r#"{"kind":"ready","v":1,"top":true,"url":"https://app.test/"}"#;
const PAN: &str = r#"{"kind":"bus","v":1,"top":true,"time":1,"origin":"https://app.test","type":"canvas:pan","payload":{"x":1}}"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_ready_agent_gets_the_view_and_a_new_breakpoint_follows() {
    let setup = setup(config(json!({})));
    attach(&setup, 1).await;
    setup.fake.emit(binding(1, READY));
    until(|| !evaluated(&setup.fake, 10, r#""cmd":"page""#).is_empty()).await;
    let first = &evaluated(&setup.fake, 10, r#""cmd":"page""#)[0];
    assert!(first.contains(r#""label":"Desktop""#), "{first}");
    assert!(first.contains(r#""index":0"#), "{first}");
    // Ctrl+Shift+2 sets the breakpoint of the active view (RB-010-16).
    let mobile: Breakpoint = setup.host.config().breakpoints[1].clone();
    setup
        .host
        .set_breakpoint(ViewId(1), &mobile)
        .expect("a breakpoint");
    until(|| {
        evaluated(&setup.fake, 10, r#""cmd":"page""#)
            .iter()
            .any(|text| text.contains(r#""label":"Mobile""#) && text.contains(r#""width":393"#))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_user_agent_ends_in_the_suffix_unless_switched_off() {
    let on = setup(config(json!({})));
    attach(&on, 1).await;
    let calls = on.fake.sent("Emulation.setUserAgentOverride");
    let agent = calls[0]["params"]["userAgent"]
        .as_str()
        .expect("a user agent");
    assert_eq!(agent, format!("{FAKE_USER_AGENT} {UA_PRODUCT}"));
    assert_eq!(
        calls[0]["params"]["userAgentMetadata"]["brands"][0]["brand"],
        "Chromium"
    );

    let off = setup(config(json!({ "pageApi": { "uaSuffix": false } })));
    attach(&off, 1).await;
    let calls = off.fake.sent("Emulation.setUserAgentOverride");
    assert_eq!(calls[0]["params"]["userAgent"], "");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bus_message_reaches_the_other_view_not_the_sender() {
    let setup = setup(config(json!({})));
    attach(&setup, 1).await;
    attach(&setup, 2).await;
    setup.fake.emit(binding(1, PAN));
    until(|| !evaluated(&setup.fake, 20, r#""cmd":"bus""#).is_empty()).await;
    let delivered = &evaluated(&setup.fake, 20, r#""cmd":"bus""#)[0];
    assert!(delivered.contains(r#""from":1"#), "{delivered}");
    assert!(delivered.contains(r#""origin":"https://app.test""#));
    assert!(delivered.contains(r#""payload":{"x":1}"#));
    assert!(evaluated(&setup.fake, 10, r#""cmd":"bus""#).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn with_the_bus_off_nothing_is_relayed() {
    let setup = setup(config(json!({ "bus": { "enabled": false } })));
    attach(&setup, 1).await;
    attach(&setup, 2).await;
    setup.fake.emit(binding(1, PAN));
    setup.fake.emit(binding(1, READY));
    // The ready is handled after the bus message: once its state went out, the bus had its turn.
    until(|| !evaluated(&setup.fake, 10, r#""cmd":"page""#).is_empty()).await;
    assert!(evaluated(&setup.fake, 20, r#""cmd":"bus""#).is_empty());
}

/// Review batch 3, finding 11: a hung page holds up neither another page's ready nor its bus
/// messages.
#[tokio::test(flavor = "multi_thread")]
async fn a_hung_page_holds_up_no_other_page() {
    let setup = setup(config(json!({})));
    attach(&setup, 1).await;
    attach(&setup, 2).await;
    setup.fake.hold("Runtime.evaluate", Some("S2"));
    // View 1's bus message goes to view 2, which does not answer.
    setup.fake.emit(binding(1, PAN));
    until(|| !evaluated(&setup.fake, 20, r#""cmd":"bus""#).is_empty()).await;
    let before = evaluated(&setup.fake, 10, r#""cmd":"page""#).len();
    setup.fake.emit(binding(1, READY));
    until(|| evaluated(&setup.fake, 10, r#""cmd":"page""#).len() > before).await;
    setup.fake.release();
}
