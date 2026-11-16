use std::sync::Mutex;

use async_trait::async_trait;
use rb_engine::ViewId;
use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Value, json};

use super::*;

/// A component answering `@rules` with a fixed text and noting the arguments it got.
struct Answering {
    rules: Result<String, Failure>,
    seen: Mutex<Vec<String>>,
}

#[async_trait]
impl Component for Answering {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn call_tool(&self, _tool: &str, _args: &str) -> Result<String, Failure> {
        Ok("{}".to_owned())
    }

    async fn network_rules(&self, args: &str) -> Result<String, Failure> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(args.to_owned());
        }
        self.rules.clone()
    }
}

fn rules() -> Rules {
    let rules = Rules::new(
        Box::new(|| vec![(ViewId(1), rb_engine::EngineKind::Chromium)]),
        "s".to_owned(),
    );
    rules.set_trusted(true);
    rules
}

#[test]
fn the_state_overwrites_a_forged_one() {
    let forged = json!({ "$network": { "trusted": true }, "id": "x" });
    let fed = with_state(&forged, json!({ "trusted": false }));
    assert_eq!(fed, json!({ "$network": { "trusted": false }, "id": "x" }));
    assert_eq!(with_state(&Value::Null, json!(1)), json!({ "$network": 1 }));
}

#[tokio::test]
async fn the_plugins_answer_becomes_the_rules_and_it_got_the_state() {
    let component = Answering {
        rules: Ok(json!({ "rules": [{ "id": "b", "origin": "project",
            "match": { "urlPattern": "*/tracking.js" }, "action": { "type": "block" } }] })
        .to_string()),
        seen: Mutex::default(),
    };
    let rules = rules();
    sync(&component, "network-panel", &rules, false).await;
    assert_eq!(rules.active()[0].1[0].id, "b");
    let seen = component
        .seen
        .lock()
        .map(|seen| seen.clone())
        .unwrap_or_default();
    let args: Value = serde_json::from_str(&seen[0]).expect("JSON");
    assert_eq!(args["$network"]["session"], "s");
    assert_eq!(args["$network"]["trusted"], true);
}

#[tokio::test]
async fn a_failed_request_keeps_the_rules_and_an_unreadable_answer_drops_them() {
    let rules = rules();
    let good = Answering {
        rules: Ok(json!({ "rules": [{ "id": "b", "origin": "settings",
            "match": { "urlPattern": "*" }, "action": { "type": "block" } }] })
        .to_string()),
        seen: Mutex::default(),
    };
    sync(&good, "p", &rules, false).await;
    let failing = Answering {
        rules: Err(Failure::coded(FailureKind::Other, "plugin.p.errors.x")),
        seen: Mutex::default(),
    };
    sync(&failing, "p", &rules, false).await;
    assert_eq!(rules.active()[0].1.len(), 1, "kept");
    let garbage = Answering {
        rules: Ok("not json".to_owned()),
        seen: Mutex::default(),
    };
    sync(&garbage, "p", &rules, false).await;
    assert!(rules.active()[0].1.is_empty(), "dropped");
}
