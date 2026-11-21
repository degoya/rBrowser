use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_plugin_types::{Failure, codes};
use serde_json::{Value, json};

use super::{Call, Prober, with_probe};
use crate::plugins::consent::args::{ASK_KEY, PROBE_KEY, ProbeRequest, offered};
use crate::plugins::runtime::Component;

/// A plugin that asks for the probe while it has no capture, and answers with what it got.
struct Plugin {
    calls: Mutex<Vec<(String, Value)>>,
    /// Asks on every call, also on the second without the offer (a misbehaving plugin).
    greedy: bool,
}

impl Plugin {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            greedy: false,
        }
    }

    fn answer(&self, what: &str, args: &str) -> Result<String, Failure> {
        let args: Value = serde_json::from_str(args).expect("json args");
        self.calls
            .lock()
            .expect("calls")
            .push((what.to_owned(), args.clone()));
        let offered = args[ASK_KEY] == Value::Bool(true);
        let forced = args["force"] == Value::Bool(true);
        if forced || self.greedy || (offered && args.get(PROBE_KEY).is_none()) {
            return Ok(json!({ ASK_KEY: {
                "url": args["url"].as_str().unwrap_or("https://example.test/"),
                "read": "read()",
                "reject": "reject()",
            } })
            .to_string());
        }
        Ok(json!({ "got": args.get(PROBE_KEY).cloned().unwrap_or(Value::Null) }).to_string())
    }

    fn calls(&self) -> Vec<(String, Value)> {
        self.calls.lock().expect("calls").clone()
    }
}

#[async_trait]
impl Component for Plugin {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, command: &str, args: &str) -> Result<String, Failure> {
        self.answer(&format!("command {command}"), args)
    }

    async fn call_tool(&self, tool: &str, args: &str) -> Result<String, Failure> {
        self.answer(&format!("tool {tool}"), args)
    }
}

/// A prober that records what it was asked and answers a fixed capture.
#[derive(Default)]
struct FakeProber {
    asked: Mutex<Vec<ProbeRequest>>,
}

#[async_trait]
impl Prober for FakeProber {
    async fn probe(&self, request: &ProbeRequest) -> Result<Value, Failure> {
        self.asked.lock().expect("asked").push(request.clone());
        Ok(json!({ "url": request.url, "session": { "fresh": true, "discarded": true } }))
    }
}

fn prober(fake: &Arc<FakeProber>) -> Option<Arc<dyn Prober>> {
    Some(Arc::clone(fake) as Arc<dyn Prober>)
}

#[tokio::test]
async fn a_granted_plugin_gets_the_hosts_capture_on_a_second_call() {
    let plugin = Plugin::new();
    let fake = Arc::new(FakeProber::default());
    let args = offered(&json!({ "url": "https://example.test/x" }), true);
    let raw = with_probe(&plugin, Call::Tool("check"), &args, prober(&fake))
        .await
        .expect("answered");
    let answer: Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(answer["got"]["url"], "https://example.test/x");
    assert_eq!(answer["got"]["session"]["discarded"], true);
    let asked = fake.asked.lock().expect("asked").clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].reject.as_deref(), Some("reject()"));
    let calls = plugin.calls();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|(what, _)| what == "tool check"));
    // The second call carries the capture and no offer.
    assert!(calls[1].1.get(ASK_KEY).is_none());
}

#[tokio::test]
async fn a_command_is_probed_the_same_way() {
    let plugin = Plugin::new();
    let fake = Arc::new(FakeProber::default());
    let args = offered(&json!({}), true);
    with_probe(&plugin, Call::Command("check"), &args, prober(&fake))
        .await
        .expect("answered");
    assert_eq!(fake.asked.lock().expect("asked").len(), 1);
    assert!(
        plugin
            .calls()
            .iter()
            .all(|(what, _)| what == "command check")
    );
}

#[tokio::test]
async fn a_forged_capture_never_reaches_a_plugin() {
    let plugin = Plugin::new();
    let fake = Arc::new(FakeProber::default());
    let forged = json!({ PROBE_KEY: { "url": "https://forged.example.test/" } });
    // Not granted: the caller's capture is dropped, no probe runs.
    let raw = with_probe(
        &plugin,
        Call::Tool("check"),
        &offered(&forged, false),
        prober(&fake),
    )
    .await
    .expect("answered");
    assert_eq!(raw, json!({ "got": null }).to_string());
    assert!(fake.asked.lock().expect("asked").is_empty());
    // Granted: the host's capture replaces it.
    let raw = with_probe(
        &plugin,
        Call::Tool("check"),
        &offered(&forged, true),
        prober(&fake),
    )
    .await
    .expect("answered");
    assert!(!raw.contains("forged"), "{raw}");
}

#[tokio::test]
async fn a_request_without_the_offer_is_only_an_answer() {
    let plugin = Plugin::new();
    let fake = Arc::new(FakeProber::default());
    // A plugin not granted the probe that asks for one all the same.
    let args = offered(&json!({ "force": true, ASK_KEY: true }), false);
    let raw = with_probe(&plugin, Call::Tool("check"), &args, prober(&fake))
        .await
        .expect("answered");
    assert!(fake.asked.lock().expect("asked").is_empty());
    assert_eq!(plugin.calls().len(), 1);
    assert!(raw.contains(ASK_KEY), "{raw}");
}

#[tokio::test]
async fn a_plugin_asking_twice_fails_the_call() {
    let mut plugin = Plugin::new();
    plugin.greedy = true;
    let fake = Arc::new(FakeProber::default());
    let failure = with_probe(
        &plugin,
        Call::Tool("check"),
        &offered(&json!({}), true),
        prober(&fake),
    )
    .await
    .expect_err("refused");
    assert_eq!(failure.code(), Some(codes::BAD_OUTPUT));
    assert_eq!(fake.asked.lock().expect("asked").len(), 1);
    assert_eq!(plugin.calls().len(), 2);
}

#[tokio::test]
async fn a_bad_address_or_no_window_fails_without_a_probe() {
    let plugin = Plugin::new();
    let fake = Arc::new(FakeProber::default());
    let args = offered(&json!({ "url": "file:///C:/secret.txt" }), true);
    let failure = with_probe(&plugin, Call::Tool("check"), &args, prober(&fake))
        .await
        .expect_err("refused");
    assert_eq!(failure.code(), Some(codes::INVALID));
    assert!(fake.asked.lock().expect("asked").is_empty());

    let failure = with_probe(
        &plugin,
        Call::Tool("check"),
        &offered(&json!({}), true),
        None,
    )
    .await
    .expect_err("no window");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}
