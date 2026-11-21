use std::time::Duration;

use rb_config::{SyncConfig, TextDirection};
use rb_engine::Emulation;
use rb_sync::{PageDir, PseudoLocale};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use crate::cdp::fake::FakeBrowser;
use crate::sync_agent::{SyncAgents, agent_config};

const BUNDLE: &str = "(()=>{})();";
const READY: &str = r#"{"kind":"ready","v":1,"top":true,"url":"http://localhost/"}"#;

fn setup() -> (std::sync::Arc<FakeBrowser>, std::sync::Arc<SyncAgents>) {
    let (fake, router) = FakeBrowser::connect();
    let agents = SyncAgents::with_bundle(router, Handle::current(), BUNDLE);
    (fake, agents)
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

fn context_created(session: &str, id: i64, frame: &str) -> Value {
    json!({
        "method": "Runtime.executionContextCreated",
        "sessionId": session,
        "params": { "context": { "id": id, "name": "rbrowser", "auxData": { "frameId": frame } } },
    })
}

/// The `locale` commands sent so far, by context.
fn locale_commands(fake: &FakeBrowser) -> Vec<(i64, String)> {
    fake.sent("Runtime.evaluate")
        .iter()
        .filter_map(|call| {
            let expression = call["params"]["expression"].as_str()?;
            expression.contains(r#""cmd":"locale""#).then(|| {
                (
                    call["params"]["contextId"].as_i64().unwrap_or(-1),
                    expression.to_owned(),
                )
            })
        })
        .collect()
}

const PSEUDO: PseudoLocale = PseudoLocale {
    factor: 1.4,
    accents: true,
    rtl: false,
};

#[tokio::test(flavor = "multi_thread")]
async fn the_emulation_s_dir_and_lang_reach_every_agent_of_the_view_once() {
    let (fake, agents) = setup();
    let config = agent_config(&SyncConfig::default(), true);
    agents
        .attach(rb_engine::ViewId(1), "T1", "S1", config)
        .await
        .expect("attaches");
    fake.emit(context_created("S1", 11, "T1"));
    fake.emit(context_created("S1", 12, "F-child"));
    until(|| agents.contexts_of(rb_engine::ViewId(1)).len() == 2).await;
    let emulation = Emulation {
        dir: Some(TextDirection::Rtl),
        lang: Some("ar".into()),
        ..Emulation::default()
    };
    agents
        .follow_emulation(rb_engine::ViewId(1), &emulation)
        .await;
    let sent = locale_commands(&fake);
    let mut contexts: Vec<i64> = sent.iter().map(|(context, _)| *context).collect();
    contexts.sort_unstable();
    assert_eq!(contexts, vec![11, 12]);
    assert!(sent[0].1.contains(r#""dir":"rtl""#), "{}", sent[0].1);
    assert!(sent[0].1.contains(r#""lang":"ar""#));
    // The same emulation again (another field changed): nothing more to send.
    agents
        .follow_emulation(rb_engine::ViewId(1), &emulation)
        .await;
    assert_eq!(locale_commands(&fake).len(), 2);
    assert_eq!(
        agents.page_locale(rb_engine::ViewId(1)).dir,
        Some(PageDir::Rtl)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_document_gets_the_view_s_locale_after_its_configuration() {
    let (fake, agents) = setup();
    let view = rb_engine::ViewId(2);
    let config = agent_config(&SyncConfig::default(), true);
    agents
        .attach(view, "T2", "S2", config)
        .await
        .expect("attaches");
    assert!(
        agents.set_pseudo(view, Some(PSEUDO)),
        "pseudo on is a change"
    );
    assert!(!agents.set_pseudo(view, Some(PSEUDO)), "the same is none");
    fake.emit(context_created("S2", 21, "T2"));
    fake.emit(json!({
        "method": "Runtime.bindingCalled",
        "sessionId": "S2",
        "params": { "name": "__rbEmit", "payload": READY, "executionContextId": 21 },
    }));
    until(|| !locale_commands(&fake).is_empty()).await;
    let expressions: Vec<String> = fake
        .sent("Runtime.evaluate")
        .iter()
        .filter_map(|call| call["params"]["expression"].as_str().map(str::to_owned))
        .collect();
    let configure = expressions
        .iter()
        .position(|text| text.contains(r#""cmd":"configure""#))
        .expect("configured");
    let locale = expressions
        .iter()
        .position(|text| text.contains(r#""cmd":"locale""#))
        .expect("the locale");
    assert!(configure < locale, "the configuration first");
    assert!(expressions[locale].contains(r#""factor":1.4"#));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plain_page_gets_no_locale_and_a_forgotten_view_none_either() {
    let (fake, agents) = setup();
    let view = rb_engine::ViewId(3);
    let config = agent_config(&SyncConfig::default(), true);
    agents
        .attach(view, "T3", "S3", config)
        .await
        .expect("attaches");
    fake.emit(context_created("S3", 31, "T3"));
    fake.emit(json!({
        "method": "Runtime.bindingCalled",
        "sessionId": "S3",
        "params": { "name": "__rbEmit", "payload": READY, "executionContextId": 31 },
    }));
    until(|| !fake.sent("Runtime.evaluate").is_empty()).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(locale_commands(&fake).is_empty(), "a new document is plain");
    agents.set_pseudo(view, Some(PSEUDO));
    agents.forget(view);
    assert!(agents.page_locale(view).is_plain());
}
