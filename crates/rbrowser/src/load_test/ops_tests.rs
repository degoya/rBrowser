use rb_api_model::ApiRequest;
use serde_json::json;

use super::*;
use crate::api_client::ops::{Context as ApiContext, Settings};
use crate::load_test::profile::Limits;
use crate::load_test::protocol::Confirm;
use crate::load_test::scenario::Scenario;

fn no_env(_name: &str) -> Option<String> {
    None
}

fn context(root: &std::path::Path) -> Context {
    Context {
        api: ApiContext {
            root: Some(root.to_path_buf()),
            trusted: true,
            environment: None,
            environments: vec![("local".into(), "http://localhost:3000".into())],
            allowed_hosts: Vec::new(),
            secrets: json!({}),
            settings: Settings {
                history_size: 0,
                max_response_bytes: 4096,
                bench_repeats: 1,
                bench_concurrency: 1,
                code: crate::api_client::export::CodeSettings::default(),
            },
            now: 1_700_000_000,
        },
        start_url: None,
        limits: Limits::default(),
        confirmed: Vec::new(),
    }
}

const SCENARIO: &str =
    r#"{"name":"Smoke","steps":[{"request":"ping.http"}],"profile":{"users":2,"holdS":5}}"#;

fn project(url: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    Store::new(dir.path())
        .save(Some("ping.http"), None, &ApiRequest::new("GET", url))
        .expect("saved");
    let scenario = Scenario::from_json(SCENARIO).expect("scenario");
    Scenarios::new(dir.path())
        .save(Some("smoke.json"), &scenario)
        .expect("saved");
    dir
}

fn answer(op: Op, ctx: &Context, run: &RunState) -> Result<Outcome, Failure> {
    super::run(op, ctx, run, &no_env)
}

#[test]
fn nothing_runs_outside_a_trusted_project() {
    let dir = project("http://localhost:3000/ping");
    let mut ctx = context(dir.path());
    ctx.api.trusted = false;
    let failure = answer(Op::State, &ctx, &RunState::default())
        .err()
        .expect("untrusted");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.untrusted"));
    ctx.api.root = None;
    let failure = answer(Op::Reports, &ctx, &RunState::default())
        .err()
        .expect("no project");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.no_project"));
}

#[test]
fn the_state_lists_requests_scenarios_ceilings_and_hosts() {
    let dir = project("http://localhost:3000/ping");
    let outcome = answer(Op::State, &context(dir.path()), &RunState::default()).expect("state");
    let state = outcome.answer;
    assert_eq!(state["requests"][0]["id"], "ping.http");
    assert_eq!(state["scenarios"][0]["id"], "smoke.json");
    assert_eq!(state["limits"]["maxUsers"], 50);
    assert_eq!(state["limits"]["maxDurationS"], 300);
    assert_eq!(state["hosts"]["own"], json!(["localhost"]));
    assert_eq!(state["running"], false);
}

#[test]
fn a_run_starts_from_the_confirmed_summary_only_and_only_once() {
    let dir = project("http://localhost:3000/ping");
    let ctx = context(dir.path());
    let idle = RunState::default();
    let preview = answer(
        Op::Preview {
            id: "smoke.json".into(),
        },
        &ctx,
        &idle,
    )
    .expect("preview")
    .answer;
    assert_eq!(preview["users"], 2);
    assert_eq!(preview["durationS"], 5);
    let confirm: Confirm = serde_json::from_value(preview["confirm"].clone()).expect("confirm");
    let start = Op::Start {
        id: "smoke.json".into(),
        confirm: confirm.clone(),
    };
    let outcome = answer(start, &ctx, &idle).expect("started");
    assert!(outcome.launch.is_some());
    assert_eq!(outcome.answer["started"], true);
    let busy = RunState {
        running: true,
        snapshot: None,
    };
    let again = Op::Start {
        id: "smoke.json".into(),
        confirm,
    };
    let failure = answer(again, &ctx, &busy).err().expect("busy");
    assert_eq!(
        failure.code(),
        Some("plugin.load-test.errors.already_running")
    );
}

#[test]
fn stop_needs_a_run() {
    let dir = project("http://localhost:3000/ping");
    let ctx = context(dir.path());
    let failure = answer(Op::Stop, &ctx, &RunState::default())
        .err()
        .expect("idle");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.not_running"));
    let running = RunState {
        running: true,
        snapshot: None,
    };
    assert!(answer(Op::Stop, &ctx, &running).expect("stops").stop);
}

#[test]
fn a_host_is_kept_only_on_the_persons_explicit_confirmation() {
    let dir = project("https://perf.partner.example/ping");
    let ctx = context(dir.path());
    let idle = RunState::default();
    let refused = Op::Allow {
        host: "perf.partner.example".into(),
        confirmed: false,
    };
    let failure = answer(refused, &ctx, &idle).err().expect("not confirmed");
    assert_eq!(
        failure.code(),
        Some("plugin.load-test.errors.not_confirmed")
    );
    let confirmed = Op::Allow {
        host: "Perf.Partner.Example".into(),
        confirmed: true,
    };
    let outcome = answer(confirmed, &ctx, &idle).expect("kept");
    assert_eq!(
        outcome.confirmed,
        Some(vec!["perf.partner.example".to_owned()])
    );
    let own = Op::Allow {
        host: "localhost".into(),
        confirmed: true,
    };
    assert_eq!(answer(own, &ctx, &idle).expect("own").confirmed, None);
    let bad = Op::Allow {
        host: "not a host".into(),
        confirmed: true,
    };
    let failure = answer(bad, &ctx, &idle).err().expect("bad");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.bad_host"));
}

#[test]
fn reports_read_back_and_compare() {
    let dir = project("http://localhost:3000/ping");
    let ctx = context(dir.path());
    let reports = Reports::new(dir.path());
    for (id, rps) in [("1-smoke", 10.0), ("2-smoke", 20.0)] {
        let report = json!({ "id": id, "name": "Smoke", "totals": { "requests": 10, "rps": rps,
            "errorRate": 0.0, "latency": { "p50": 1.0, "p95": 2.0, "p99": 3.0 } } });
        reports.save(&report, "<html></html>").expect("saved");
    }
    let idle = RunState::default();
    let list = answer(Op::Reports, &ctx, &idle).expect("list").answer;
    assert_eq!(list["reports"][0]["id"], "2-smoke");
    let latest = answer(Op::Report { id: None }, &ctx, &idle)
        .expect("latest")
        .answer;
    assert_eq!(latest["id"], "2-smoke");
    let compared = Op::Compare {
        a: "1-smoke".into(),
        b: "2-smoke".into(),
    };
    let compared = answer(compared, &ctx, &idle).expect("compared").answer;
    assert_eq!(compared["delta"]["rps"], 10.0);
}

/// RB-096-09: no saved request yet - the panel's hint opens the API client panel, where they
/// are made; the op only asks the window for it.
#[test]
fn the_api_client_is_opened_for_the_window_to_do() {
    let dir = tempfile::tempdir().expect("temp");
    let outcome = answer(
        Op::OpenApiClient,
        &context(dir.path()),
        &RunState::default(),
    )
    .expect("asks for it");
    assert!(outcome.open_api_client);
    assert_eq!(outcome.answer, json!({ "opening": "api-client" }));
    assert!(outcome.launch.is_none() && !outcome.stop && outcome.confirmed.is_none());
    let state = answer(Op::State, &context(dir.path()), &RunState::default()).expect("state");
    assert!(!state.open_api_client);
    assert_eq!(state.answer["requests"], json!([]), "no request saved");
}

/// The w31 review: with the API client plugin off or not loaded its panel command is not there;
/// the op is refused with a code the panel says, instead of answering `opening`.
#[test]
fn the_api_client_is_opened_only_while_its_command_is_there() {
    assert!(api_client_reachable(true).is_ok());
    let failure = api_client_reachable(false).expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.load-test.errors.api_client_missing")
    );
}
