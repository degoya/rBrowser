use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::{Value, json};

use super::{ApiHost, with_host};
use crate::api_client::protocol::{OFFER_KEY, Op, Origin, RESULT_KEY, offered};
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

    fn calls(&self) -> Vec<Value> {
        self.calls.lock().expect("calls").clone()
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
    fail: bool,
}

#[async_trait]
impl ApiHost for FakeHost {
    async fn run(&self, op: Op, origin: Origin) -> Result<Value, Failure> {
        self.ran.lock().expect("ran").push((op, origin));
        if self.fail {
            return Err(Failure::coded(
                FailureKind::AccessDenied,
                "plugin.api-client.errors.host_not_allowed",
            )
            .with_param("host", "api.example.com"));
        }
        Ok(json!({ "status": 200 }))
    }
}

fn shared(host: &Arc<FakeHost>) -> Option<Arc<dyn ApiHost>> {
    let host: Arc<dyn ApiHost> = host.clone();
    Some(host)
}

fn run_op(id: &str) -> Value {
    json!({ "op": { "op": "run", "id": id } })
}

#[tokio::test]
async fn a_granted_plugins_op_runs_and_its_second_call_gets_the_result() {
    let plugin = Plugin::new(false);
    let host = Arc::new(FakeHost::default());
    let args = offered(&run_op("users/list.http"), true, Origin::Tool);
    let answer = with_host(&plugin, Call::Tool("run"), &args, shared(&host))
        .await
        .expect("answers");
    assert_eq!(
        serde_json::from_str::<Value>(&answer).expect("json"),
        json!({ "result": { "status": 200 } })
    );
    let ran = host.ran.lock().expect("ran").clone();
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].1, Origin::Tool);
    let calls = plugin.calls();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].get(OFFER_KEY).is_none());
}

#[tokio::test]
async fn a_plugin_not_offered_the_client_is_answered_as_it_is() {
    let plugin = Plugin::new(false);
    let host = Arc::new(FakeHost::default());
    let args = offered(&run_op("a.http"), false, Origin::Tool);
    let answer = with_host(&plugin, Call::Tool("run"), &args, shared(&host))
        .await
        .expect("answers");
    assert_eq!(answer, json!({ "plain": true }).to_string());
    assert!(host.ran.lock().expect("ran").is_empty());
}

#[tokio::test]
async fn mcp_cannot_send_a_free_request_or_grant_a_host() {
    let plugin = Plugin::new(false);
    let host = Arc::new(FakeHost::default());
    for op in [
        json!({ "op": "send", "request": { "method": "GET", "url": "https://a.example" } }),
        json!({ "op": "allow", "host": "a.example" }),
        json!({ "op": "save", "request": { "method": "GET", "url": "https://a.example" } }),
    ] {
        let args = offered(&json!({ "op": op }), true, Origin::Tool);
        let failure = with_host(&plugin, Call::Tool("panel"), &args, shared(&host))
            .await
            .expect_err("refused");
        assert_eq!(failure.code(), Some(codes::CAPABILITY_DENIED));
    }
    assert!(host.ran.lock().expect("ran").is_empty());
}

#[tokio::test]
async fn the_ops_failure_is_the_calls() {
    let plugin = Plugin::new(false);
    let host = Arc::new(FakeHost {
        fail: true,
        ..FakeHost::default()
    });
    let args = offered(&run_op("a.http"), true, Origin::Panel);
    let failure = with_host(&plugin, Call::Tool("run"), &args, shared(&host))
        .await
        .expect_err("fails");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.host_not_allowed")
    );
    assert_eq!(plugin.calls().len(), 1);
}

#[tokio::test]
async fn a_plugin_that_asks_twice_fails_and_none_runs_without_a_window() {
    let greedy = Plugin::new(true);
    let host = Arc::new(FakeHost::default());
    let args = offered(&run_op("a.http"), true, Origin::Panel);
    let failure = with_host(&greedy, Call::Tool("run"), &args, shared(&host))
        .await
        .expect_err("fails");
    assert_eq!(failure.code(), Some(codes::BAD_OUTPUT));
    let plugin = Plugin::new(false);
    let failure = with_host(&plugin, Call::Tool("run"), &args, None)
        .await
        .expect_err("fails");
    assert_eq!(failure.code(), Some(codes::NOT_AVAILABLE));
}
