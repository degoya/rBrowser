use std::time::Duration;

use rb_config::SyncConfig;
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::sync_agent::agent_config;

const BUNDLE: &str = "(()=>{})();";

fn setup() -> (Arc<FakeBrowser>, Arc<SyncAgents>) {
    let (fake, router) = FakeBrowser::connect();
    let agents = SyncAgents::with_bundle(router, Handle::current(), BUNDLE);
    (fake, agents)
}

fn config(active: bool) -> AgentConfig {
    agent_config(&SyncConfig::default(), active)
}

/// Lets the spawned tasks run until `done` holds (or fails the test after a second).
async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

fn context_created(session: &str, id: i64, frame: &str, name: &str) -> Value {
    json!({
        "method": "Runtime.executionContextCreated",
        "sessionId": session,
        "params": { "context": { "id": id, "name": name, "auxData": { "frameId": frame } } },
    })
}

fn binding(session: &str, context: i64, payload: &str) -> Value {
    json!({
        "method": "Runtime.bindingCalled",
        "sessionId": session,
        "params": { "name": "__rbEmit", "payload": payload, "executionContextId": context },
    })
}

const READY: &str = r#"{"kind":"ready","v":1,"top":true,"url":"http://localhost/"}"#;

#[tokio::test(flavor = "multi_thread")]
async fn attach_injects_the_boot_source_into_the_session() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(1), "T1", "S1", config(true))
        .await
        .expect("attaches");
    let script = &fake.sent("Page.addScriptToEvaluateOnNewDocument")[0];
    assert_eq!(script["sessionId"], "S1");
    let source = script["params"]["source"].as_str().expect("a source");
    assert!(source.contains("\"active\":true"));
    assert!(source.ends_with(BUNDLE));
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_bundle_nothing_is_injected() {
    let (fake, router) = FakeBrowser::connect();
    let agents = SyncAgents::with_bundle(router, Handle::current(), "");
    agents
        .attach(ViewId(1), "T1", "S1", config(true))
        .await
        .expect("no error");
    assert!(fake.sent("Runtime.addBinding").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_binding_call_becomes_a_report_and_ready_gets_the_config() {
    let (fake, agents) = setup();
    let mut reports = agents.subscribe();
    agents
        .attach(ViewId(3), "T3", "S3", config(false))
        .await
        .expect("attaches");
    fake.emit(context_created("S3", 11, "T3", "rbrowser"));
    fake.emit(binding("S3", 11, READY));
    let report = tokio::time::timeout(Duration::from_secs(1), reports.recv())
        .await
        .expect("in time")
        .expect("a report");
    assert_eq!(report.view, ViewId(3));
    assert_eq!(report.frame_id.as_deref(), Some("T3"));
    assert!(matches!(report.message, AgentMessage::Ready(_)));
    until(|| !fake.sent("Runtime.evaluate").is_empty()).await;
    let evaluate = &fake.sent("Runtime.evaluate")[0];
    assert_eq!(evaluate["params"]["contextId"], 11);
    let expression = evaluate["params"]["expression"].as_str().expect("text");
    assert!(expression.contains(r#""cmd":"configure""#), "{expression}");
}

/// One thread: the agents read nothing while the burst comes in, so a shared buffer would
/// have dropped the context before the binding call came.
#[tokio::test(flavor = "current_thread")]
async fn a_burst_of_screencast_frames_does_not_make_the_agents_miss_their_events() {
    let (fake, agents) = setup();
    let mut reports = agents.subscribe();
    agents
        .attach(ViewId(4), "T4", "S4", config(false))
        .await
        .expect("attaches");
    fake.emit(context_created("S4", 12, "T4", "rbrowser"));
    for _ in 0..3000 {
        fake.emit(json!({
            "method": "Page.screencastFrame", "sessionId": "S4",
            "params": { "data": "AAAA", "sessionId": 1 },
        }));
    }
    fake.emit(binding("S4", 12, READY));
    let report = tokio::time::timeout(Duration::from_secs(1), reports.recv())
        .await
        .expect("in time")
        .expect("a report");
    assert_eq!(report.view, ViewId(4));
    assert_eq!(
        report.frame_id.as_deref(),
        Some("T4"),
        "the context was kept"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn foreign_and_invalid_binding_calls_are_dropped() {
    let (fake, agents) = setup();
    let mut reports = agents.subscribe();
    agents
        .attach(ViewId(1), "T1", "S1", config(true))
        .await
        .expect("attaches");
    // Another binding, an unknown session, a payload of another version, garbage.
    fake.emit(json!({
        "method": "Runtime.bindingCalled", "sessionId": "S1",
        "params": { "name": "other", "payload": READY, "executionContextId": 1 },
    }));
    fake.emit(binding("S9", 1, READY));
    fake.emit(binding("S1", 1, &READY.replace("\"v\":1", "\"v\":2")));
    fake.emit(binding("S1", 1, "{not json"));
    fake.emit(binding("S1", 1, READY));
    let report = tokio::time::timeout(Duration::from_secs(1), reports.recv())
        .await
        .expect("in time")
        .expect("a report");
    assert_eq!(report.view, ViewId(1));
    assert!(
        reports.try_recv().is_err(),
        "only the valid call is reported"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_runs_in_the_top_frame_s_context() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(2), "T2", "S2", config(true))
        .await
        .expect("attaches");
    let missing = agents.command(ViewId(2), &AgentCommand::Ping).await;
    assert!(missing.is_err(), "no context yet");
    fake.emit(context_created("S2", 5, "F-child", "rbrowser"));
    fake.emit(context_created("S2", 6, "T2", ""));
    fake.emit(context_created("S2", 7, "T2", "rbrowser"));
    until(|| {
        lock(&agents.state)
            .contexts
            .context_of("S2", "T2")
            .is_some()
    })
    .await;
    // The fake answers `{}`: no value, an error, but the call went to context 7.
    let answer = agents.command(ViewId(2), &AgentCommand::Ping).await;
    assert!(answer.is_err());
    let evaluate = fake.sent("Runtime.evaluate");
    assert_eq!(evaluate[0]["params"]["contextId"], 7);
    assert_eq!(evaluate[0]["params"]["returnByValue"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_iframe_target_gets_the_agent_too() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(4), "T4", "S4", config(true))
        .await
        .expect("attaches");
    fake.emit(json!({
        "method": "Target.attachedToTarget", "sessionId": "S4",
        "params": { "sessionId": "C1", "targetInfo": { "type": "iframe" }, "waitingForDebugger": false },
    }));
    until(|| fake.sent("Runtime.addBinding").len() == 2).await;
    assert_eq!(fake.sent("Runtime.addBinding")[1]["sessionId"], "C1");
    // The iframe's agent reports for the view.
    let mut reports = agents.subscribe();
    fake.emit(binding("C1", 3, READY));
    let report = tokio::time::timeout(Duration::from_secs(1), reports.recv())
        .await
        .expect("in time")
        .expect("a report");
    assert_eq!(report.view, ViewId(4));
    // Detached: its calls are dropped.
    fake.emit(json!({
        "method": "Target.detachedFromTarget", "sessionId": "S4", "params": { "sessionId": "C1" },
    }));
    until(|| !lock(&agents.state).sessions.contains_key("C1")).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn configure_reaches_every_agent_of_the_view() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(5), "T5", "S5", config(false))
        .await
        .expect("attaches");
    fake.emit(context_created("S5", 21, "T5", "rbrowser"));
    fake.emit(context_created("S5", 22, "F-child", "rbrowser"));
    until(|| lock(&agents.state).contexts.contexts("S5").len() == 2).await;
    agents.configure(ViewId(5), config(true)).await;
    let mut contexts: Vec<i64> = fake
        .sent("Runtime.evaluate")
        .iter()
        .filter_map(|call| call["params"]["contextId"].as_i64())
        .collect();
    contexts.sort_unstable();
    assert_eq!(contexts, vec![21, 22]);
    agents.forget(ViewId(5));
    assert!(lock(&agents.state).sessions.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_re_attach_drops_the_view_s_older_sessions_and_contexts() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(6), "T6", "S6", config(true))
        .await
        .expect("attaches");
    fake.emit(context_created("S6", 31, "T6", "rbrowser"));
    fake.emit(json!({
        "method": "Target.attachedToTarget", "sessionId": "S6",
        "params": { "sessionId": "C6", "targetInfo": { "type": "iframe" }, "waitingForDebugger": false },
    }));
    until(|| {
        let state = lock(&agents.state);
        state.contexts.context_of("S6", "T6").is_some() && state.sessions.contains_key("C6")
    })
    .await;
    // The renderer crashed: the view is attached again in a new session.
    agents
        .attach(ViewId(6), "T6b", "S6b", config(true))
        .await
        .expect("attaches again");
    {
        let state = lock(&agents.state);
        assert!(!state.sessions.contains_key("S6"), "the old page session");
        assert!(!state.sessions.contains_key("C6"), "the old iframe session");
        assert!(state.contexts.contexts("S6").is_empty());
    }
    fake.emit(context_created("S6b", 41, "T6b", "rbrowser"));
    until(|| {
        lock(&agents.state)
            .contexts
            .context_of("S6b", "T6b")
            .is_some()
    })
    .await;
    let _ = agents.command(ViewId(6), &AgentCommand::Ping).await;
    let evaluate = fake.sent("Runtime.evaluate");
    assert_eq!(evaluate.last().expect("a call")["sessionId"], "S6b");
    assert_eq!(evaluate.last().expect("a call")["params"]["contextId"], 41);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_iframe_context_reported_before_its_attach_is_kept() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(7), "T7", "S7", config(true))
        .await
        .expect("attaches");
    // `runImmediately` created the world before the attach event was handled.
    fake.emit(context_created("C7", 3, "F7", "rbrowser"));
    fake.emit(json!({
        "method": "Target.attachedToTarget", "sessionId": "S7",
        "params": { "sessionId": "C7", "targetInfo": { "type": "iframe" }, "waitingForDebugger": false },
    }));
    until(|| lock(&agents.state).sessions.contains_key("C7")).await;
    let mut reports = agents.subscribe();
    fake.emit(binding("C7", 3, READY));
    let report = tokio::time::timeout(Duration::from_secs(1), reports.recv())
        .await
        .expect("in time")
        .expect("a report");
    assert_eq!(report.frame_id.as_deref(), Some("F7"));
    // Detached: its contexts go with it.
    fake.emit(json!({
        "method": "Target.detachedFromTarget", "sessionId": "S7", "params": { "sessionId": "C7" },
    }));
    until(|| lock(&agents.state).contexts.contexts("C7").is_empty()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_agent_world_s_contexts_of_unknown_sessions_are_kept() {
    let (fake, agents) = setup();
    fake.emit(context_created("X1", 1, "F1", ""));
    fake.emit(context_created("X1", 2, "F1", "rbrowser"));
    until(|| !lock(&agents.state).contexts.contexts("X1").is_empty()).await;
    assert_eq!(lock(&agents.state).contexts.contexts("X1"), vec![2]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_stored_config_of_a_view_is_readable() {
    let (_fake, agents) = setup();
    assert_eq!(agents.config(ViewId(8)), None);
    agents
        .attach(ViewId(8), "T8", "S8", config(false))
        .await
        .expect("attaches");
    assert_eq!(agents.config(ViewId(8)), Some(config(false)));
    agents.configure(ViewId(8), config(true)).await;
    assert_eq!(agents.config(ViewId(8)), Some(config(true)));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_expression_runs_in_the_agent_s_world_of_the_top_frame() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(8), "T8", "S8", config(false))
        .await
        .expect("attaches");
    let missing = agents.evaluate_in_world(ViewId(8), "1+1").await;
    assert!(missing.is_err(), "no context yet");
    fake.emit(context_created("S8", 11, "T8", ""));
    fake.emit(context_created("S8", 12, "T8", "rbrowser"));
    until(|| {
        lock(&agents.state)
            .contexts
            .context_of("S8", "T8")
            .is_some()
    })
    .await;
    // The fake answers `{}`: no value, an error, but the call went to the world's context.
    assert!(agents.evaluate_in_world(ViewId(8), "1+1").await.is_err());
    let evaluate = fake.sent("Runtime.evaluate");
    let call = evaluate.last().expect("a call");
    assert_eq!(call["sessionId"], "S8");
    assert_eq!(call["params"]["contextId"], 12);
    assert_eq!(call["params"]["expression"], "1+1");
    assert_eq!(call["params"]["returnByValue"], true);
}

/// The `configure` commands sent to `context`, in order.
fn configures_to(fake: &FakeBrowser, context: i64) -> Vec<String> {
    fake.sent("Runtime.evaluate")
        .iter()
        .filter(|call| call["params"]["contextId"] == context)
        .filter_map(|call| call["params"]["expression"].as_str().map(str::to_owned))
        .filter(|expression| expression.contains(r#""cmd":"configure""#))
        .collect()
}

/// Review batch 3, finding 4: a configuration that did not reach an agent is sent again by
/// the next reconfigure, though it is the one the view should have.
#[tokio::test(flavor = "multi_thread")]
async fn a_configuration_that_did_not_reach_an_agent_is_still_needed() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(1), "T1", "S1", config(false))
        .await
        .expect("attaches");
    fake.emit(context_created("S1", 11, "T1", "rbrowser"));
    until(|| agents.top_context(ViewId(1)).is_ok()).await;
    assert!(!agents.needs_config(ViewId(1), &config(false)));
    fake.fail("Runtime.evaluate", true);
    agents.configure(ViewId(1), config(true)).await;
    assert!(
        agents.needs_config(ViewId(1), &config(true)),
        "the evaluate failed: send it again"
    );
    fake.fail("Runtime.evaluate", false);
    fake.answer_with(
        "Runtime.evaluate",
        json!({ "result": { "value": { "status": "done" } } }),
    );
    agents.configure(ViewId(1), config(true)).await;
    assert!(!agents.needs_config(ViewId(1), &config(true)));
    assert!(agents.needs_config(ViewId(1), &config(false)));
    assert!(
        !agents.needs_config(ViewId(9), &config(true)),
        "no agent, nothing to send"
    );
}

/// Review batch 3, finding 4: a new document's agent ends with the configuration that is
/// current when its configure answers, not the one of its ready.
#[tokio::test(flavor = "multi_thread")]
async fn a_ready_agent_ends_with_the_newest_configuration() {
    let (fake, agents) = setup();
    agents
        .attach(ViewId(3), "T3", "S3", config(false))
        .await
        .expect("attaches");
    fake.emit(context_created("S3", 11, "T3", "rbrowser"));
    until(|| agents.top_context(ViewId(3)).is_ok()).await;
    fake.answer_with(
        "Runtime.evaluate",
        json!({ "result": { "value": { "status": "done" } } }),
    );
    fake.hold("Runtime.evaluate", None);
    fake.emit(binding("S3", 11, READY));
    until(|| configures_to(&fake, 11).len() == 1).await;
    // The active view changes while the ready's configure is under way.
    let changing = {
        let agents = agents.clone();
        tokio::spawn(async move { agents.configure(ViewId(3), config(true)).await })
    };
    until(|| configures_to(&fake, 11).len() == 2).await;
    fake.release();
    changing.await.expect("configured");
    until(|| configures_to(&fake, 11).len() == 3).await;
    let last = configures_to(&fake, 11).pop().expect("sent");
    assert!(last.contains(r#""active":true"#), "{last}");
}
