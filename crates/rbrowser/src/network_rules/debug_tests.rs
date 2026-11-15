use std::sync::{Arc, Mutex};

use rb_engine::EngineKind;
use serde_json::json;

use super::*;
use crate::network_rules::compose::{Answer, ComposeRequest, ComposeResult, Runner};
use crate::network_rules::document::Stage;
use crate::network_rules::fetch::Outcome;
use crate::network_rules::held::{Edits, HeldRequest, Resolution};

fn rules() -> Arc<Rules> {
    Arc::new(Rules::new(
        Box::new(|| {
            vec![
                (ViewId(1), EngineKind::Chromium),
                (ViewId(2), EngineKind::Chromium),
            ]
        }),
        "s1".to_owned(),
    ))
}

fn paused(url: &str, stage: Stage) -> Paused {
    let mut params = json!({ "requestId": "r1", "request": { "url": url, "method": "GET" } });
    if stage == Stage::Response {
        params["responseStatusCode"] = json!(200);
    }
    Paused::of(&params).expect("paused")
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
fn a_breakpoint_holds_a_matching_request_of_its_views_and_stage_before_any_rule() {
    let rules = rules();
    rules.set_trusted(true);
    rules
        .apply(
            "network-panel",
            &json!({
                "rules": [{ "id": "mock", "origin": "project", "match": { "urlPattern": "*/api/*" },
                            "action": { "type": "mock", "status": 500 } }],
                "breakpoints": [
                    { "id": "bp-1", "match": { "urlPattern": "*/api/*" }, "views": [1] },
                    { "id": "bp-2", "match": { "urlPattern": "*/page" }, "stage": "response" },
                ],
            }),
        )
        .expect("reads");
    assert!(matches!(
        rules.verdict(ViewId(1), &paused("http://a.test/api/user", Stage::Request)),
        Verdict::Hold(id) if id == "bp-1"
    ));
    assert!(
        matches!(
            rules.verdict(ViewId(2), &paused("http://a.test/api/user", Stage::Request)),
            Verdict::Outcome(Outcome::Call(decision)) if decision.method == "Fetch.fulfillRequest"
        ),
        "view 2 is not among the breakpoint's views: the mock applies"
    );
    assert!(
        matches!(
            rules.verdict(ViewId(2), &paused("http://a.test/page", Stage::Request)),
            Verdict::Outcome(Outcome::Call(decision)) if decision.rule.is_none()
        ),
        "a response breakpoint holds nothing at the request stage"
    );
    assert!(matches!(
        rules.verdict(ViewId(2), &paused("http://a.test/page", Stage::Response)),
        Verdict::Hold(id) if id == "bp-2"
    ));
    let state = rules.state("network-panel");
    assert_eq!(
        state["breakpoints"],
        json!({ "enabled": true, "active": 2 })
    );
    assert_eq!(state["hits"]["mock"], 1);
}

#[test]
fn the_breakpoint_switch_off_lets_everything_through() {
    let rules = rules();
    rules
        .apply(
            "network-panel",
            &json!({ "breakpoints": [{ "id": "bp-1", "match": { "urlPattern": "*" } }],
                     "breakpointsEnabled": false }),
        )
        .expect("reads");
    assert!(matches!(
        rules.verdict(ViewId(1), &paused("http://a.test/", Stage::Request)),
        Verdict::Outcome(_)
    ));
    assert_eq!(
        rules.state("network-panel")["breakpoints"]["enabled"],
        false
    );
    assert!(
        rules.patterns().is_empty(),
        "no pattern for a switched-off breakpoint"
    );
}

#[test]
fn every_plugin_keeps_its_own_answers() {
    let rules = rules();
    let (first, _rx1) = rules.held().hold(held("http://a.test/api"));
    rules
        .apply(
            "network-panel",
            &json!({ "asks": [{ "id": "a1", "kind": "abort", "held": first }] }),
        )
        .expect("reads");
    assert_eq!(rules.state("network-panel")["answers"]["a1"]["done"], true);
    // Another plugin's sync with asks of its own neither wipes nor re-runs the first's.
    rules
        .apply(
            "other",
            &json!({ "asks": [{ "id": "a1", "kind": "abort", "held": "nope" }] }),
        )
        .expect("reads");
    assert_eq!(
        rules.state("other")["answers"]["a1"]["error"],
        "unknown_held"
    );
    assert_eq!(rules.state("network-panel")["answers"]["a1"]["done"], true);
    rules.apply("other", &json!({})).expect("reads");
    assert_eq!(rules.state("other")["answers"], json!({}));
    assert_eq!(rules.state("network-panel")["answers"]["a1"]["done"], true);
}

#[test]
fn a_project_plugin_brings_no_breakpoints_settings_or_asks() {
    let rules = rules();
    rules
        .apply_from_project(
            "shared",
            &json!({ "breakpoints": [{ "id": "bp-1", "match": { "urlPattern": "*" } }],
                     "settings": { "breakpointTimeoutMs": 1 },
                     "asks": [{ "id": "a1", "kind": "abort", "held": "h1" }] }),
        )
        .expect("reads");
    assert!(matches!(
        rules.verdict(ViewId(1), &paused("http://a.test/", Stage::Request)),
        Verdict::Outcome(_)
    ));
    assert_eq!(rules.settings(), DebugSettings::default());
    let state = rules.state("shared");
    assert_eq!(state["answers"], json!({}));
    assert!(
        state.get("held").is_none(),
        "a project plugin sees no held request"
    );
}

#[test]
fn asks_resolve_held_requests_and_their_answers_live_while_the_ask_does() {
    let rules = rules();
    let (first, rx1) = rules.held().hold(held("http://a.test/api"));
    let (_second, rx2) = rules.held().hold(held("http://a.test/api"));
    let (_third, rx3) = rules.held().hold(held("http://a.test/other"));
    let asks = |list: Value| json!({ "asks": list });
    rules
        .apply(
            "network-panel",
            &asks(json!([
                { "id": "a1", "kind": "continue", "held": first, "allViews": true, "edits": { "status": 500 } },
                { "id": "a2", "kind": "abort", "held": "nope" },
            ])),
        )
        .expect("reads");
    assert!(
        matches!(rx1.blocking_recv(), Ok(Resolution::Continue(edits)) if edits.status == Some(500))
    );
    assert!(matches!(rx2.blocking_recv(), Ok(Resolution::Continue(_))));
    let state = rules.state("network-panel");
    assert_eq!(
        state["answers"]["a1"],
        json!({ "done": true, "resolved": 2 })
    );
    assert_eq!(state["answers"]["a2"]["error"], "unknown_held");
    assert_eq!(state["held"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        state["held"][0]["headers"][0][1], "***",
        "masked for the panel too"
    );
    // The same document again: the asks are not run twice, and a pruned ask loses its answer.
    rules
        .apply(
            "network-panel",
            &asks(json!([{ "id": "a1", "kind": "continue", "held": first, "allViews": true }])),
        )
        .expect("reads");
    let state = rules.state("network-panel");
    assert_eq!(state["answers"]["a1"]["resolved"], 2);
    assert!(state["answers"].get("a2").is_none());
    rules.apply("network-panel", &json!({})).expect("reads");
    assert_eq!(rules.state("network-panel")["answers"], json!({}));
    drop(rx3);
}

#[derive(Default)]
struct FakeRunner {
    composed: Mutex<Vec<(Option<ViewId>, ComposeRequest, u32)>>,
}

fn result(status: u16) -> ComposeResult {
    ComposeResult {
        method: "GET".to_owned(),
        url: "https://a.test/?token=x".to_owned(),
        view: Some(ViewId(1)),
        status: Some(status),
        headers: vec![("set-cookie".to_owned(), "a=b".to_owned())],
        body: b"ok".to_vec(),
        body_truncated: false,
        time_ms: 3,
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
        lock(&self.composed).push((view, request, times));
        Box::pin(async move { vec![result(200); times as usize] })
    }

    fn repeat(
        &self,
        _view: ViewId,
        request_id: String,
        _edits: Edits,
        _times: u32,
    ) -> Answer<Result<Vec<ComposeResult>, String>> {
        Box::pin(async move {
            if request_id == "known" {
                Ok(vec![result(204)])
            } else {
                Err("no such request".to_owned())
            }
        })
    }

    fn spawn(&self, task: Answer<()>) {
        tokio::spawn(task);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn compose_and_repeat_asks_run_on_the_runner_and_land_in_composed() {
    let rules = rules();
    let runner = Arc::new(FakeRunner::default());
    rules.set_runner(Arc::downgrade(&rules), runner.clone());
    rules
        .apply(
            "network-panel",
            &json!({ "asks": [
                { "id": "c1", "kind": "compose", "view": 1, "times": 2,
                  "request": { "method": "GET", "url": "https://a.test/" } },
                { "id": "r1", "kind": "repeat", "view": 1, "requestId": "known" },
                { "id": "r2", "kind": "repeat", "view": 1, "requestId": "gone" },
            ] }),
        )
        .expect("reads");
    for _ in 0..200 {
        if rules.composed().len() == 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(lock(&runner.composed).len(), 1);
    let state = rules.state("network-panel");
    assert_eq!(
        state["answers"]["c1"],
        json!({ "done": true, "results": 2 })
    );
    assert_eq!(
        state["answers"]["r1"],
        json!({ "done": true, "results": 1 })
    );
    assert_eq!(
        state["answers"]["r2"],
        json!({ "error": "no such request" })
    );
    let composed = state["composed"].as_array().expect("a list");
    assert_eq!(composed.len(), 3);
    // The asks run at once: the order of their results is not fixed.
    let of_compose = composed
        .iter()
        .find(|entry| entry["ask"] == "c1")
        .expect("the compose's result");
    assert_eq!(
        of_compose["headers"][0][1], "***",
        "masked for the panel too"
    );
    assert_eq!(of_compose["url"], "https://a.test/?token=***");
}

#[test]
fn without_a_runner_a_compose_ask_is_answered_with_an_error() {
    let rules = rules();
    rules
        .apply(
            "network-panel",
            &json!({ "asks": [{ "id": "c1", "kind": "compose",
                               "request": { "method": "GET", "url": "https://a.test/" } }] }),
        )
        .expect("reads");
    assert_eq!(
        rules.state("network-panel")["answers"]["c1"]["error"],
        "no_runner"
    );
}

#[test]
fn the_settings_and_the_state_come_from_the_first_party_document() {
    let rules = rules();
    rules
        .apply(
            "network-panel",
            &json!({ "settings": { "breakpointTimeoutMs": 7, "maxBodyBytes": 99, "allowComposeFromMcp": true } }),
        )
        .expect("reads");
    let settings = rules.settings();
    assert_eq!((settings.timeout_ms, settings.max_body_bytes), (7, 99));
    assert!(settings.allow_compose_from_mcp);
    let state = rules.state("network-panel");
    assert_eq!(
        state["settings"],
        json!({ "timeoutMs": 7, "maxBodyBytes": 99, "allowComposeFromMcp": true })
    );
    rules.note_skipped("network-panel", "big");
    assert_eq!(rules.state("network-panel")["skipped"], json!({ "big": 1 }));
}

#[test]
fn a_map_local_rule_outside_the_allowed_roots_is_rejected_with_its_reason() {
    let project = tempfile::tempdir().expect("a temp dir");
    std::fs::create_dir_all(project.path().join("fixtures")).expect("dir");
    let outside = tempfile::tempdir().expect("a temp dir");
    let rules = rules();
    rules.set_trusted(true);
    let document = |folder: &str| {
        json!({ "rules": [{ "id": "l", "origin": "project", "match": { "urlPattern": "*/config.json" },
                            "action": { "type": "mapLocal", "folder": folder } }] })
    };
    let rejected = rules
        .apply("network-panel", &document("fixtures"))
        .expect("reads");
    assert_eq!(rejected[0].reason, "no project open");
    rules.set_project_root(Some(project.path().to_path_buf()));
    let rejected = rules
        .apply("network-panel", &document("fixtures"))
        .expect("reads");
    assert!(rejected.is_empty(), "{rejected:?}");
    assert!(matches!(
        rules.verdict(ViewId(1), &paused("http://a.test/config.json", Stage::Request)),
        Verdict::Outcome(Outcome::Local { rule, .. }) if rule == "l"
    ));
    let elsewhere = outside.path().display().to_string();
    let rejected = rules
        .apply("network-panel", &document(&elsewhere))
        .expect("reads");
    assert_eq!(
        rejected[0].reason,
        "folder outside the project and the allowed roots"
    );
    assert_eq!(rules.state("network-panel")["rejected"][0]["id"], "l");
    rules
        .apply(
            "network-panel",
            &json!({ "settings": { "mapLocalRoots": [elsewhere] },
                     "rules": document(&outside.path().display().to_string())["rules"] }),
        )
        .expect("reads");
    assert!(
        rules.state("network-panel")["rejected"]
            .as_array()
            .expect("list")
            .is_empty()
    );
}
