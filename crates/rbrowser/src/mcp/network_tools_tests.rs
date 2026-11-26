use std::sync::{Arc, Mutex};

use rb_engine::EngineKind;
use serde_json::json;

use super::*;
use crate::cdp::lock;
use crate::network_rules::compose::{Answer, ComposeRequest, Runner};
use crate::network_rules::document::Stage;
use crate::network_rules::held::{Edits, HeldRequest};

fn rules() -> Arc<Rules> {
    Arc::new(Rules::new(
        Box::new(|| vec![(ViewId(1), EngineKind::Chromium)]),
        "s1".to_owned(),
    ))
}

fn held(url: &str) -> HeldRequest {
    HeldRequest {
        id: String::new(),
        key: String::new(),
        view: ViewId(1),
        session: "S1".to_owned(),
        request_id: "i1".to_owned(),
        stage: Stage::Request,
        method: "GET".to_owned(),
        url: url.to_owned(),
        headers: vec![("Cookie".to_owned(), "sid=1".to_owned())],
        body: None,
        body_truncated: false,
        resource_type: "XHR".to_owned(),
        breakpoint: "bp-1".to_owned(),
        since: 1,
        response: None,
    }
}

#[test]
fn the_list_carries_breakpoints_and_the_held_requests_masked() {
    let rules = rules();
    rules
        .apply(
            "network-panel",
            &json!({ "breakpoints": [{ "id": "bp-1", "match": { "urlPattern": "*/api/*" } }] }),
        )
        .expect("reads");
    let (_, _rx) = rules.held().hold(held("https://a.test/api?token=x"));
    let answer = list(&rules, true);
    assert_eq!(answer["enabled"], true);
    assert_eq!(answer["breakpoints"][0]["id"], "bp-1");
    assert_eq!(answer["heldCount"], 1);
    assert_eq!(answer["held"][0]["url"], "https://a.test/api?token=***");
    assert_eq!(answer["held"][0]["headers"][0][1], "***");
    assert_eq!(answer["timeoutMs"], 30000);
}

#[test]
fn a_continue_with_edits_needs_evaluate_and_without_edits_does_not() {
    let rules = rules();
    let (id, rx) = rules.held().hold(held("https://a.test/api"));
    let edited = json!({ "id": id, "edits": { "status": 500 } });
    let refused = continue_held(&rules, false, &edited).expect_err("refused");
    assert_eq!(refused.code, "evaluate.disabled");
    assert_eq!(refused.settings_key.as_deref(), Some("mcp.allowEvaluate"));
    assert_eq!(rules.held().len(), 1, "still held");
    assert!(
        continue_held(
            &rules,
            false,
            &json!({ "id": id, "edits": { "status": 999 } })
        )
        .is_err()
    );
    let answer = continue_held(&rules, false, &json!({ "id": id })).expect("plain");
    assert_eq!(answer["resolved"], 1);
    assert_eq!(answer["held"], 0);
    assert!(
        matches!(rx.blocking_recv(), Ok(Resolution::Continue(edits)) if edits == Edits::default())
    );
    let (id, rx) = rules.held().hold(held("https://a.test/api"));
    continue_held(
        &rules,
        true,
        &json!({ "id": id, "edits": { "status": 500 } }),
    )
    .expect("allowed");
    assert!(
        matches!(rx.blocking_recv(), Ok(Resolution::Continue(edits)) if edits.status == Some(500))
    );
    let missing = abort(&rules, &json!({ "id": "h99" })).expect_err("unknown");
    assert_eq!(missing.code, "network.held_not_found");
    assert!(abort(&rules, &json!({})).is_err());
}

#[test]
fn an_abort_for_all_views_fails_every_held_copy() {
    let rules = rules();
    let (first, rx1) = rules.held().hold(held("https://a.test/api"));
    let (_, rx2) = rules.held().hold(held("https://a.test/api"));
    let answer = abort(&rules, &json!({ "id": first, "allViews": true })).expect("aborted");
    assert_eq!(answer["resolved"], 2);
    assert!(matches!(rx1.blocking_recv(), Ok(Resolution::Abort)));
    assert!(matches!(rx2.blocking_recv(), Ok(Resolution::Abort)));
}

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<String>>,
}

fn result() -> ComposeResult {
    ComposeResult {
        method: "GET".to_owned(),
        url: "https://a.test/?sid=9".to_owned(),
        view: Some(ViewId(1)),
        status: Some(200),
        headers: vec![("set-cookie".to_owned(), "a=b".to_owned())],
        body: b"ok".to_vec(),
        body_truncated: false,
        time_ms: 1,
        at: 1,
        error: None,
    }
}

impl Runner for FakeRunner {
    fn compose(
        &self,
        view: Option<ViewId>,
        request: ComposeRequest,
        times: u32,
    ) -> Answer<Vec<ComposeResult>> {
        lock(&self.calls).push(format!("compose {view:?} {} {times}", request.url));
        Box::pin(async move { vec![result(); times as usize] })
    }

    fn repeat(
        &self,
        view: ViewId,
        request_id: String,
        _edits: Edits,
        times: u32,
    ) -> Answer<Result<Vec<ComposeResult>, String>> {
        lock(&self.calls).push(format!("repeat {view:?} {request_id} {times}"));
        Box::pin(async move {
            if request_id == "known" {
                Ok(vec![result()])
            } else {
                Err("the view no longer lists this request".to_owned())
            }
        })
    }

    fn spawn(&self, task: Answer<()>) {
        tokio::spawn(task);
    }
}

#[tokio::test]
async fn compose_and_repeat_are_gated_by_the_setting_and_answer_masked() {
    let rules = rules();
    let runner = Arc::new(FakeRunner::default());
    rules.set_runner(Arc::downgrade(&rules), runner.clone());
    let request =
        json!({ "request": { "method": "GET", "url": "https://a.test/" }, "view": 1, "times": 2 });
    let refused = compose(&rules, &request).await.expect_err("off by default");
    assert_eq!(refused.code, COMPOSE_DISABLED);
    assert_eq!(refused.settings_key.as_deref(), Some(ALLOW_COMPOSE));
    assert!(lock(&runner.calls).is_empty());
    rules
        .apply(
            "network-panel",
            &json!({ "settings": { "allowComposeFromMcp": true } }),
        )
        .expect("reads");
    let answer = compose(&rules, &request).await.expect("sent");
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(2));
    assert_eq!(answer["results"][0]["url"], "https://a.test/?sid=***");
    assert_eq!(answer["results"][0]["headers"][0][1], "***");
    assert_eq!(
        lock(&runner.calls)[0],
        "compose Some(ViewId(1)) https://a.test/ 2"
    );
    assert!(
        compose(&rules, &json!({ "request": { "url": "ftp://x" } }))
            .await
            .is_err()
    );
    assert!(
        compose(
            &rules,
            &json!({ "request": { "url": "https://a.test/" }, "times": 11 })
        )
        .await
        .is_err()
    );
    let repeated = repeat(&rules, &json!({ "view": 1, "requestId": "known" }))
        .await
        .expect("sent");
    assert_eq!(repeated["results"][0]["status"], 200);
    let gone = repeat(&rules, &json!({ "view": 1, "requestId": "gone" }))
        .await
        .expect_err("gone");
    assert_eq!(gone.code, "network.request_not_found");
    assert!(
        repeat(&rules, &json!({ "requestId": "known" }))
            .await
            .is_err(),
        "the view is required"
    );
}

#[test]
fn held_bodies_stay_back_without_evaluate_and_the_plugins_tools_keep_the_gates() {
    let rules = rules();
    let mut request = held("https://a.test/login");
    request.body = Some(b"password=hunter2".to_vec());
    let (_, _rx) = rules.held().hold(request);
    let without = list(&rules, false);
    assert!(without["held"][0].get("body").is_none());
    assert_eq!(without["held"][0]["bodyWithheld"], true);
    assert!(!without.to_string().contains("hunter2"));
    assert_eq!(list(&rules, true)["held"][0]["body"], "password=hunter2");
    // No window runs the rules here: a compose over the plugin's tool is refused all the same.
    for tool in ["networkpanel_compose", "networkpanel_repeat"] {
        let refused = plugin_gate(tool, true).expect_err("gated");
        assert_eq!(refused.code, COMPOSE_DISABLED);
        assert_eq!(refused.settings_key.as_deref(), Some(ALLOW_COMPOSE));
    }
    assert_eq!(
        plugin_gate("networkpanel_debug", false)
            .expect_err("gated")
            .code,
        "evaluate.disabled"
    );
    assert!(plugin_gate("networkpanel_debug", true).is_ok());
    assert!(plugin_gate("networkpanel_list", false).is_ok());
}
