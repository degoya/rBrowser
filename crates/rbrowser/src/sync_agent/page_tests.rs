use std::time::Duration;

use rb_config::{ProjectConfig, SyncConfig};
use rb_engine::{Emulation, EngineKind, ViewInfo};
use rb_sync::{BusError, BusMessage, BusSettings};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::{FAKE_USER_AGENT, FakeBrowser};
use crate::sync_agent::page_state::{PageSwitches, page_state};
use crate::sync_agent::{SyncAgents, agent_config};

const BUNDLE: &str = "(()=>{})();";
const PAGE_BUNDLE: &str = "installPageApi();";

struct Setup {
    fake: Arc<FakeBrowser>,
    agents: Arc<SyncAgents>,
    page: Arc<PageApi>,
}

fn setup() -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let agents = SyncAgents::with_bundle(router.clone(), Handle::current(), BUNDLE);
    let page = PageApi::with_bundle(router, agents.clone(), PAGE_BUNDLE);
    Setup { fake, agents, page }
}

fn state(id: u32, width: u32) -> PageState {
    let info = ViewInfo {
        id: ViewId(id),
        label: "Tablet".into(),
        breakpoint: None,
        cell_breakpoint: None,
        engine: EngineKind::Chromium,
        width,
        height: 1024,
        dpr: 2.0,
        scale: 1.0,
        url: String::new(),
        title: String::new(),
        loading: false,
        session: "default".into(),
        cell: None,
    };
    let switches = PageSwitches {
        api: true,
        ua_suffix: true,
        bus: BusSettings::default(),
    };
    page_state(&info, 0, &Emulation::default(), &switches)
}

/// Attaches the view's agent and page API in session `S<id>` with an agent context `<id>0`.
async fn attached(setup: &Setup, id: u32) {
    let (target, session) = (format!("T{id}"), format!("S{id}"));
    setup
        .agents
        .attach(
            ViewId(id),
            &target,
            &session,
            agent_config(&SyncConfig::default(), false),
        )
        .await
        .expect("attaches");
    setup.fake.emit(json!({
        "method": "Runtime.executionContextCreated",
        "sessionId": session,
        "params": { "context": { "id": i64::from(id) * 10, "name": "rbrowser", "auxData": { "frameId": target } } },
    }));
    for _ in 0..200 {
        if setup
            .agents
            .command(ViewId(id), &AgentCommand::Ping)
            .await
            .is_err_and(|error| !error.to_string().contains("no sync agent"))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    setup
        .page
        .attach(ViewId(id), &session, state(id, 768), true)
        .await;
}

/// The main-world scripts registered, without the agent's own (which names its world).
fn page_scripts(fake: &FakeBrowser) -> Vec<Value> {
    fake.sent("Page.addScriptToEvaluateOnNewDocument")
        .into_iter()
        .filter(|call| call["params"].get("worldName").is_none())
        .collect()
}

fn evaluated(fake: &FakeBrowser, needle: &str) -> Vec<Value> {
    fake.sent("Runtime.evaluate")
        .into_iter()
        .filter(|call| {
            call["params"]["expression"]
                .as_str()
                .is_some_and(|text| text.contains(needle))
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn attach_registers_the_main_world_script_with_the_state() {
    let setup = setup();
    attached(&setup, 1).await;
    let scripts = page_scripts(&setup.fake);
    assert_eq!(scripts.len(), 1);
    assert_eq!(scripts[0]["sessionId"], "S1");
    assert_eq!(scripts[0]["params"]["runImmediately"], true);
    let source = scripts[0]["params"]["source"].as_str().expect("a source");
    assert!(source.contains(PAGE_BUNDLE));
    assert!(source.contains(r#""label":"Tablet""#), "{source}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_state_replaces_the_script_and_reaches_the_agent() {
    let setup = setup();
    attached(&setup, 1).await;
    setup.page.update(ViewId(1), state(1, 393), true).await;
    let removed = setup.fake.sent("Page.removeScriptToEvaluateOnNewDocument");
    assert_eq!(removed.len(), 1);
    let first = &page_scripts(&setup.fake)[0];
    let identifier = removed[0]["params"]["identifier"].clone();
    assert_ne!(identifier, Value::Null);
    assert_eq!(page_scripts(&setup.fake).len(), 2);
    assert!(first["params"]["source"].as_str().is_some());
    let page = evaluated(&setup.fake, r#""cmd":"page""#);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0]["params"]["contextId"], 10);
    assert!(
        page[0]["params"]["expression"]
            .as_str()
            .is_some_and(|text| text.contains(r#""width":393"#))
    );
    // The same state again: nothing is sent.
    setup.page.update(ViewId(1), state(1, 393), true).await;
    assert_eq!(page_scripts(&setup.fake).len(), 2);
    assert_eq!(evaluated(&setup.fake, r#""cmd":"page""#).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn switched_off_the_script_goes_and_none_comes() {
    let setup = setup();
    attached(&setup, 2).await;
    setup.page.update(ViewId(2), state(2, 768), false).await;
    assert_eq!(
        setup
            .fake
            .sent("Page.removeScriptToEvaluateOnNewDocument")
            .len(),
        1
    );
    assert_eq!(page_scripts(&setup.fake).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ready_agent_gets_the_state() {
    let setup = setup();
    attached(&setup, 3).await;
    setup.page.on_ready(ViewId(3)).await;
    assert_eq!(evaluated(&setup.fake, r#""cmd":"page""#).len(), 1);
    setup.page.forget(ViewId(3));
    setup.page.on_ready(ViewId(3)).await;
    assert_eq!(evaluated(&setup.fake, r#""cmd":"page""#).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn bus_messages_are_checked_and_rate_limited_per_view() {
    let setup = setup();
    let settings = BusSettings {
        enabled: true,
        max_payload_bytes: 64,
        max_per_second: 3,
    };
    let message = BusMessage {
        v: 1,
        top: true,
        frame: None,
        time: 0,
        origin: "https://app.test".into(),
        r#type: "canvas:pan".into(),
        payload: json!({ "x": 1 }),
    };
    let accepted = (0..1000)
        .filter(|_| setup.page.accept(ViewId(1), &message, &settings).is_ok())
        .count();
    assert_eq!(accepted, 3);
    assert!(setup.page.accept(ViewId(2), &message, &settings).is_ok());
    let large = BusMessage {
        payload: json!("x".repeat(100)),
        ..message.clone()
    };
    assert!(matches!(
        setup.page.accept(ViewId(3), &large, &settings),
        Err(BusError::TooLarge { .. })
    ));
    assert_eq!(
        setup.page.accept(ViewId(1), &message, &settings),
        Err(BusError::RateLimited)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delivery_reaches_each_target_s_agent() {
    let setup = setup();
    attached(&setup, 1).await;
    attached(&setup, 2).await;
    let delivered = setup
        .page
        .deliver(
            &[ViewId(2), ViewId(9)],
            Some(ViewId(1)),
            Some("https://app.test"),
            "canvas:pan",
            &rb_sync::BusPayload::of(&json!({ "x": 1 })).expect("serializes"),
        )
        .await;
    // The fake gives no answer: nothing counts as delivered, but the command went to view 2.
    assert!(delivered.is_empty());
    let bus = evaluated(&setup.fake, r#""cmd":"bus""#);
    assert_eq!(bus.len(), 1);
    assert_eq!(bus[0]["params"]["contextId"], 20);
    let expression = bus[0]["params"]["expression"].as_str().expect("text");
    assert!(expression.contains(r#""from":1"#), "{expression}");
    assert!(expression.contains(r#""origin":"https://app.test""#));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_engine_user_agent_is_read_once() {
    let setup = setup();
    let first = setup.page.engine_user_agent().await.expect("the fake's");
    assert_eq!(first.user_agent, FAKE_USER_AGENT);
    assert!(setup.page.engine_user_agent().await.is_some());
    assert_eq!(setup.fake.sent("Browser.getVersion").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_toggles_flip_the_switches_for_the_session() {
    let setup = setup();
    let config = ProjectConfig::default();
    assert!(setup.page.switches(&config).bus.enabled);
    assert!(!setup.page.toggle(Switch::Bus, &config));
    assert!(!setup.page.switches(&config).bus.enabled);
    assert!(!setup.page.switches(&config).bus_in_effect().enabled);
}
