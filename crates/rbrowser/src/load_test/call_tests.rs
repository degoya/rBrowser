use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_plugin_types::{Failure, codes};
use serde_json::{Value, json};

use super::{LoadHost, with_host};
use crate::load_test::protocol::{OFFER_KEY, Op, Origin, RESULT_KEY, offered};
use crate::plugins::consent::Call;
use crate::plugins::runtime::Component;

/// The plugin's forwarding: it asks for its `op` argument while it has no result, then answers
/// the result; `greedy` asks again on the second call.
struct Plugin {
    calls: Mutex<Vec<Value>>,
    greedy: bool,
}

impl Plugin {
    fn new(greedy: bool) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            greedy,
        }
    }

    fn answer(&self, args: &str) -> Result<String, Failure> {
        let args: Value = serde_json::from_str(args).expect("json args");
        self.calls.lock().expect("calls").push(args.clone());
        if let Some(result) = args.get(RESULT_KEY)
            && !self.greedy
        {
            return Ok(json!({ "result": result }).to_string());
        }
        if args.get(OFFER_KEY).is_some() || self.greedy {
            return Ok(json!({ OFFER_KEY: args["op"] }).to_string());
        }
        Ok(json!({ "plain": true }).to_string())
    }
}

#[async_trait]
impl Component for Plugin {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, _command: &str, args: &str) -> Result<String, Failure> {
        self.answer(args)
    }

    async fn call_tool(&self, _tool: &str, args: &str) -> Result<String, Failure> {
        self.answer(args)
    }
}

#[derive(Default)]
struct FakeHost {
    ran: Mutex<Vec<(Op, Origin)>>,
}

#[async_trait]
impl LoadHost for FakeHost {
    async fn run(&self, op: Op, origin: Origin) -> Result<Value, Failure> {
        self.ran.lock().expect("ran").push((op, origin));
        Ok(json!({ "stopped": true }))
    }
}

fn host() -> (Arc<FakeHost>, Option<Arc<dyn LoadHost>>) {
    let fake = Arc::new(FakeHost::default());
    let host: Arc<dyn LoadHost> = fake.clone();
    (fake, Some(host))
}

#[tokio::test]
async fn the_panels_op_runs_and_its_answer_is_the_calls() {
    let plugin = Plugin::new(false);
    let (fake, host) = host();
    let args = offered(&json!({ "op": { "op": "stop" } }), true, Origin::Panel);
    let raw = with_host(&plugin, Call::Tool("panel"), &args, host)
        .await
        .expect("ran");
    assert_eq!(
        serde_json::from_str::<Value>(&raw).expect("json"),
        json!({ "result": { "stopped": true } })
    );
    assert_eq!(fake.ran.lock().expect("ran")[0], (Op::Stop, Origin::Panel));
}

#[tokio::test]
async fn mcp_cannot_start_a_run() {
    let plugin = Plugin::new(false);
    let (fake, host) = host();
    let start = json!({ "op": { "op": "start", "id": "a.json",
        "confirm": { "targets": [], "users": 1, "rps": 1.0, "durationS": 1 } } });
    let args = offered(&start, true, Origin::Tool);
    let failure = with_host(&plugin, Call::Tool("panel"), &args, host)
        .await
        .expect_err("refused");
    assert_eq!(failure.code(), Some(codes::CAPABILITY_DENIED));
    assert!(fake.ran.lock().expect("ran").is_empty());
}

#[tokio::test]
async fn a_plugin_not_granted_gets_no_offer_and_runs_nothing() {
    let plugin = Plugin::new(false);
    let (fake, host) = host();
    let args = offered(
        &json!({ "op": { "op": "stop" }, OFFER_KEY: "panel" }),
        false,
        Origin::Panel,
    );
    let raw = with_host(&plugin, Call::Tool("panel"), &args, host)
        .await
        .expect("plain");
    assert_eq!(
        serde_json::from_str::<Value>(&raw).expect("json"),
        json!({ "plain": true })
    );
    assert!(fake.ran.lock().expect("ran").is_empty());
}

#[tokio::test]
async fn a_plugin_that_asks_twice_fails_the_call() {
    let plugin = Plugin::new(true);
    let (_fake, host) = host();
    let args = offered(&json!({ "op": { "op": "stop" } }), true, Origin::Command);
    let failure = with_host(&plugin, Call::Command("cancel"), &args, host)
        .await
        .expect_err("greedy");
    assert_eq!(failure.code(), Some(codes::BAD_OUTPUT));
}
