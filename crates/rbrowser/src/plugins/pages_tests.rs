use std::time::Duration;

use rb_plugin_host::{LogLevel, MAX_MESSAGE_BYTES, PageLog, PageScriptSource, world_name};
use serde_json::json;

use super::*;
use crate::cdp::fake::FakeBrowser;

/// An inbox that answers every request with `{"echo": <message>}` and keeps what it got.
#[derive(Default)]
struct FakeInbox {
    requests: Mutex<Vec<(String, ViewId, Value)>>,
    events: Mutex<Vec<(String, ViewId, Value)>>,
    logs: Mutex<Vec<(String, ViewId, PageLog)>>,
}

#[async_trait]
impl Inbox for FakeInbox {
    async fn request(&self, plugin: &str, view: ViewId, message: Value) -> Result<Value, String> {
        lock(&self.requests).push((plugin.to_owned(), view, message.clone()));
        if message == json!("fail") {
            return Err("plugin.hello.error.nope".to_owned());
        }
        Ok(json!({ "echo": message }))
    }

    fn emit(&self, plugin: &str, view: ViewId, message: Value) {
        lock(&self.events).push((plugin.to_owned(), view, message));
    }

    fn log(&self, plugin: &str, view: ViewId, line: PageLog) {
        lock(&self.logs).push((plugin.to_owned(), view, line));
    }
}

fn bridge(id: &str) -> Arc<PageBridge> {
    Arc::new(PageBridge {
        plugin_id: id.to_owned(),
        world: world_name(id),
        binding: format!("__rbp_{id}"),
        receiver: format!("__rbr_{id}"),
        scripts: vec![PageScriptSource {
            run_at: "document-idle".to_owned(),
            path: "page/a.js".to_owned(),
            text: "rbPlugin.emit(1);".to_owned(),
        }],
    })
}

fn setup() -> (Arc<FakeBrowser>, Arc<FakeInbox>, Arc<PluginPages>) {
    let (fake, router) = FakeBrowser::connect();
    let inbox = Arc::new(FakeInbox::default());
    let weak: Weak<dyn Inbox> = Arc::downgrade(&(inbox.clone() as Arc<dyn Inbox>));
    let pages = PluginPages::new(router, Handle::current(), weak);
    (fake, inbox, pages)
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

fn context_created(session: &str, id: i64, name: &str) -> Value {
    json!({
        "method": "Runtime.executionContextCreated",
        "sessionId": session,
        "params": { "context": { "id": id, "name": name, "auxData": { "frameId": "F" } } },
    })
}

fn binding(session: &str, name: &str, context: i64, payload: &str) -> Value {
    json!({
        "method": "Runtime.bindingCalled",
        "sessionId": session,
        "params": { "name": name, "payload": payload, "executionContextId": context },
    })
}

fn evaluated(fake: &FakeBrowser) -> Vec<Value> {
    fake.sent("Runtime.evaluate")
}

#[tokio::test(flavor = "multi_thread")]
async fn each_plugin_runs_in_its_own_world_and_the_page_gets_nothing() {
    let (fake, _, pages) = setup();
    pages.set_plugins(vec![bridge("hello"), bridge("other")]);
    pages.attach(ViewId(2), "S2").await;
    let bindings = fake.sent("Runtime.addBinding");
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0]["sessionId"], "S2");
    assert_eq!(
        bindings[0]["params"],
        json!({ "name": "__rbp_hello", "executionContextName": "rbplugin-hello" })
    );
    let scripts = fake.sent("Page.addScriptToEvaluateOnNewDocument");
    assert_eq!(scripts.len(), 2);
    for (script, world) in scripts.iter().zip(["rbplugin-hello", "rbplugin-other"]) {
        assert_eq!(script["params"]["worldName"], world, "never the main world");
        assert_eq!(script["params"]["runImmediately"], true);
        let source = script["params"]["source"].as_str().expect("a source");
        assert!(source.contains("\"view\":2"));
        assert!(source.contains("rbPlugin.emit(1);"));
    }
    assert_eq!(pages.session_of(ViewId(2)).as_deref(), Some("S2"));
}

#[tokio::test(flavor = "multi_thread")]
async fn without_plugins_nothing_is_injected() {
    let (fake, _, pages) = setup();
    pages.attach(ViewId(1), "S1").await;
    assert!(fake.sent("Runtime.addBinding").is_empty());
    assert!(
        fake.sent("Page.addScriptToEvaluateOnNewDocument")
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reconnect_injects_again_into_the_new_session() {
    let (fake, _, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(1), "S1").await;
    pages.attach(ViewId(1), "S9").await;
    let scripts = fake.sent("Page.addScriptToEvaluateOnNewDocument");
    assert_eq!(scripts.len(), 2);
    assert_eq!(scripts[1]["sessionId"], "S9");
    assert_eq!(pages.session_of(ViewId(1)).as_deref(), Some("S9"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_reaches_the_inbox_and_its_answer_the_calling_context() {
    let (fake, inbox, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(3), "S3").await;
    fake.emit(context_created("S3", 41, "rbplugin-hello"));
    fake.emit(binding("S3", "__rbp_hello", 41, "s7:{\"ping\":1}"));
    until(|| !evaluated(&fake).is_empty()).await;
    assert_eq!(
        lock(&inbox.requests).clone(),
        vec![("hello".to_owned(), ViewId(3), json!({ "ping": 1 }))]
    );
    let reply = &evaluated(&fake)[0];
    assert_eq!(reply["sessionId"], "S3");
    assert_eq!(reply["params"]["contextId"], 41);
    let expression = reply["params"]["expression"].as_str().expect("text");
    assert!(expression.contains("globalThis[\"__rbr_hello\"]"));
    assert!(expression.contains(r#"{"reply":7,"ok":true,"value":{"echo":{"ping":1}}}"#));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_request_answers_its_code() {
    // The inbox is held: the pages keep only a weak reference to it.
    let (fake, _inbox, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(3), "S3").await;
    fake.emit(context_created("S3", 41, "rbplugin-hello"));
    fake.emit(binding("S3", "__rbp_hello", 41, "s2:\"fail\""));
    until(|| !evaluated(&fake).is_empty()).await;
    let expression = evaluated(&fake)[0]["params"]["expression"].to_string();
    assert!(expression.contains("plugin.hello.error.nope"));
}

// RB-080-66: a line the page script logs goes to rBrowser, unanswered; a malformed one nowhere.
#[tokio::test(flavor = "multi_thread")]
async fn a_logged_line_reaches_the_inbox_and_nothing_is_answered() {
    let (fake, inbox, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(3), "S3").await;
    fake.emit(context_created("S3", 41, "rbplugin-hello"));
    fake.emit(binding(
        "S3",
        "__rbp_hello",
        41,
        "l:{\"level\":\"nope\",\"values\":[]}",
    ));
    fake.emit(binding(
        "S3",
        "__rbp_hello",
        41,
        "l:{\"level\":\"warning\",\"values\":[\"plugins.error.busy\"]}",
    ));
    let logs = inbox.clone();
    until(move || !lock(&logs.logs).is_empty()).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        lock(&inbox.logs).clone(),
        vec![(
            "hello".to_owned(),
            ViewId(3),
            PageLog {
                level: LogLevel::Warning,
                values: vec!["plugins.error.busy".to_owned()],
                stack: None,
            }
        )]
    );
    assert!(lock(&inbox.requests).is_empty());
    assert!(evaluated(&fake).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_over_the_limit_is_refused_and_never_reaches_the_component() {
    let (fake, inbox, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(3), "S3").await;
    fake.emit(context_created("S3", 41, "rbplugin-hello"));
    let large = format!("s5:\"{}\"", "x".repeat(MAX_MESSAGE_BYTES));
    fake.emit(binding("S3", "__rbp_hello", 41, &large));
    until(|| !evaluated(&fake).is_empty()).await;
    assert!(lock(&inbox.requests).is_empty());
    let expression = evaluated(&fake)[0]["params"]["expression"].to_string();
    assert!(expression.contains("plugins.error.too_large"));
    assert!(expression.contains("\\\"reply\\\":5"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_from_outside_the_plugins_world_or_an_unknown_session_is_ignored() {
    let (fake, inbox, pages) = setup();
    pages.set_plugins(vec![bridge("hello"), bridge("other")]);
    pages.attach(ViewId(3), "S3").await;
    fake.emit(context_created("S3", 41, "rbplugin-hello"));
    fake.emit(context_created("S3", 42, "rbplugin-other"));
    // The main world (context 1), another plugin's world, a session that is no view's.
    fake.emit(binding("S3", "__rbp_hello", 1, "s1:1"));
    fake.emit(binding("S3", "__rbp_hello", 42, "s2:2"));
    fake.emit(binding("S8", "__rbp_hello", 41, "s3:3"));
    fake.emit(binding("S3", "__rbp_hello", 41, "e:{\"ok\":1}"));
    let events = inbox.clone();
    until(move || !lock(&events.events).is_empty()).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(lock(&inbox.requests).is_empty());
    assert!(evaluated(&fake).is_empty());
    assert_eq!(
        lock(&inbox.events).clone(),
        vec![("hello".to_owned(), ViewId(3), json!({ "ok": 1 }))]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_post_reaches_the_plugins_contexts_in_the_views_asked_for() {
    let (fake, _, pages) = setup();
    pages.set_plugins(vec![bridge("hello"), bridge("other")]);
    pages.attach(ViewId(1), "S1").await;
    pages.attach(ViewId(2), "S2").await;
    fake.emit(context_created("S1", 11, "rbplugin-hello"));
    fake.emit(context_created("S2", 21, "rbplugin-hello"));
    fake.emit(context_created("S2", 22, "rbplugin-other"));
    let wait = pages.clone();
    until(move || lock(&wait.state).contexts.len() == 3).await;
    pages.post("hello", Some(ViewId(2)), &json!("two")).await;
    let sent = evaluated(&fake);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["params"]["contextId"], 21);
    pages.post("hello", None, &json!("all")).await;
    let mut contexts: Vec<i64> = evaluated(&fake)[1..]
        .iter()
        .filter_map(|message| message["params"]["contextId"].as_i64())
        .collect();
    contexts.sort_unstable();
    assert_eq!(contexts, vec![11, 21], "never another plugin's world");
}

#[tokio::test(flavor = "multi_thread")]
async fn switching_a_plugin_off_removes_its_script_and_stops_it_and_on_injects_it() {
    let (fake, _, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(1), "S1").await;
    fake.emit(context_created("S1", 11, "rbplugin-hello"));
    let wait = pages.clone();
    until(move || !lock(&wait.state).contexts.is_empty()).await;
    let identifier = fake.sent("Page.addScriptToEvaluateOnNewDocument").len();
    assert_eq!(identifier, 1);

    pages.set_plugins(Vec::new());
    until(|| !evaluated(&fake).is_empty()).await;
    let removed = fake.sent("Page.removeScriptToEvaluateOnNewDocument");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0]["sessionId"], "S1");
    assert!(removed[0]["params"]["identifier"].is_string());
    let stop = evaluated(&fake)[0]["params"]["expression"].to_string();
    assert!(stop.contains("\\\"stop\\\":true"));

    pages.set_plugins(vec![bridge("hello")]);
    until(|| fake.sent("Page.addScriptToEvaluateOnNewDocument").len() == 2).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_closed_view_and_a_detached_session_are_forgotten() {
    let (fake, _, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(1), "S1").await;
    pages.attach(ViewId(2), "S2").await;
    pages.forget(ViewId(1));
    assert_eq!(pages.session_of(ViewId(1)), None);
    fake.emit(json!({
        "method": "Target.detachedFromTarget",
        "params": { "sessionId": "S2" },
    }));
    let wait = pages.clone();
    until(move || wait.session_of(ViewId(2)).is_none()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn switched_on_again_in_the_same_document_the_world_is_reached_without_a_new_context() {
    // Chromium keeps one isolated world per frame and name: switched on again, the script runs
    // in the world it had, and no `executionContextCreated` follows (review finding F8).
    let (fake, inbox, pages) = setup();
    pages.set_plugins(vec![bridge("hello")]);
    pages.attach(ViewId(1), "S1").await;
    fake.emit(context_created("S1", 11, "rbplugin-hello"));
    let wait = pages.clone();
    until(move || !lock(&wait.state).contexts.is_empty()).await;

    pages.set_plugins(Vec::new());
    until(|| !evaluated(&fake).is_empty()).await;
    // Off: a call of the old binding is ignored.
    fake.emit(binding("S1", "__rbp_hello", 11, "s1:1"));
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(lock(&inbox.requests).is_empty());

    pages.set_plugins(vec![bridge("hello")]);
    until(|| fake.sent("Page.addScriptToEvaluateOnNewDocument").len() == 2).await;
    fake.emit(binding("S1", "__rbp_hello", 11, "s2:2"));
    let requests = inbox.clone();
    until(move || lock(&requests.requests).len() == 1).await;
}
