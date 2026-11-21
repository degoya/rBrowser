use std::fs;
use std::sync::Mutex;

use rb_api_model::{Auth, Body, Entry as Header};
use serde_json::json;

use super::*;
use crate::api_client::send::{Exchange, Prepared};
use crate::api_client::timing::Phases;

/// Answers every send with the next of `answers` (the last one again), noting what it got.
struct FakeSender {
    sent: Mutex<Vec<Prepared>>,
    answers: Vec<Exchange>,
}

impl FakeSender {
    fn answering(answers: Vec<Exchange>) -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            answers,
        }
    }

    fn sent(&self) -> Vec<Prepared> {
        self.sent.lock().expect("sent").clone()
    }
}

impl Sender for FakeSender {
    fn send(
        &self,
        prepared: &Prepared,
        _limit: u64,
        _secrets: &[String],
    ) -> Result<Exchange, Failure> {
        let mut sent = self.sent.lock().expect("sent");
        sent.push(prepared.clone());
        let index = (sent.len() - 1).min(self.answers.len() - 1);
        Ok(self.answers[index].clone())
    }
}

fn exchange(status: u16, headers: &[(&str, &str)], body: &str) -> Exchange {
    Exchange {
        status,
        status_text: String::new(),
        headers: headers
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect(),
        body: body.as_bytes().to_vec(),
        truncated: false,
        phases: Phases::default(),
    }
}

fn env(name: &str) -> Option<String> {
    (name == "RB_SECRET_API_KEY").then(|| "key-from-env".to_owned())
}

fn context(root: &std::path::Path) -> Context {
    Context {
        root: Some(root.to_path_buf()),
        trusted: true,
        environment: Some("staging".into()),
        environments: vec![("staging".into(), "https://staging.example.com/".into())],
        allowed_hosts: vec!["staging.example.com".into()],
        secrets: json!({ "staging": { "token": "tok-secret-1" } }),
        settings: Settings {
            history_size: 10,
            max_response_bytes: 1024,
            bench_repeats: 3,
            bench_concurrency: 1,
            code: crate::api_client::export::CodeSettings::default(),
        },
        now: 1_700_000_000,
    }
}

fn saved(root: &std::path::Path) -> String {
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/me");
    request.name = "Me".into();
    request.auth = Auth::Bearer {
        token: "{{token}}".into(),
    };
    request
        .headers
        .push(Header::new("X-Api-Key", "{{$secret api-key}}"));
    Store::new(root).save(None, None, &request).expect("saved")
}

#[test]
fn every_op_needs_an_open_and_trusted_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let mut ctx = context(dir.path());
    ctx.trusted = false;
    let failure = run(Op::List, &ctx, &sender, &env).expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.untrusted"));
    ctx.root = None;
    let failure = run(Op::List, &ctx, &sender, &env).expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.no_project"));
}

#[test]
fn a_saved_request_runs_with_its_variables_and_comes_back_masked() {
    let dir = tempfile::tempdir().expect("temp dir");
    let id = saved(dir.path());
    let sender = FakeSender::answering(vec![exchange(
        200,
        &[("Content-Type", "application/json")],
        "{\"seen\": \"tok-secret-1 key-from-env\"}",
    )]);
    let outcome = run(
        Op::Run {
            id: id.clone(),
            environment: None,
        },
        &context(dir.path()),
        &sender,
        &env,
    )
    .expect("runs");
    let sent = sender.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].url, "https://staging.example.com/me");
    assert!(
        sent[0]
            .headers
            .contains(&("Authorization".to_owned(), "Bearer tok-secret-1".to_owned()))
    );
    assert!(
        sent[0]
            .headers
            .contains(&("X-Api-Key".to_owned(), "key-from-env".to_owned()))
    );
    assert_eq!(outcome.answer["status"], 200);
    assert_eq!(outcome.answer["body"], "{\"seen\": \"*** ***\"}");
    assert_eq!(outcome.answer["environment"], "staging");
    assert_eq!(outcome.allowed_hosts, None);
    let history = history::read(dir.path());
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].url, "{{baseUrl}}/me");
    assert_eq!(history[0].id.as_deref(), Some(id.as_str()));
    let written = fs::read_to_string(dir.path().join(history::HISTORY_FILE)).expect("history");
    assert!(!written.contains("tok-secret-1"));
}

#[test]
fn an_encoded_secret_never_reaches_the_run_answer() {
    use base64::Engine as _;
    use rb_api_model::KeyPlace;

    // A query key goes out form-encoded and a Basic pair base64-encoded: both forms are masked.
    let secret = "tok/secret 1";
    let pair = base64::engine::general_purpose::STANDARD.encode(format!("ada:{secret}"));
    let auths = [
        Auth::ApiKey {
            name: "key".into(),
            value: "{{token}}".into(),
            place: KeyPlace::Query,
        },
        Auth::Basic {
            username: "ada".into(),
            password: "{{token}}".into(),
        },
    ];
    for auth in auths {
        // A server echoes what it got: the pair only reaches it with Basic auth.
        let basic = matches!(auth, Auth::Basic { .. });
        let dir = tempfile::tempdir().expect("temp dir");
        let mut request = ApiRequest::new("GET", "{{baseUrl}}/me");
        request.name = "Me".into();
        request.auth = auth;
        let id = Store::new(dir.path())
            .save(None, None, &request)
            .expect("saved");
        let mut ctx = context(dir.path());
        ctx.secrets = json!({ "staging": { "token": secret } });
        let seen = if basic { pair.as_str() } else { "none" };
        let echo = format!("{{\"url\": \"/me?key=tok%2Fsecret+1\", \"auth\": \"Basic {seen}\"}}");
        let sender = FakeSender::answering(vec![exchange(200, &[("X-Echo", seen)], &echo)]);
        let outcome = run(
            Op::Run {
                id,
                environment: None,
            },
            &ctx,
            &sender,
            &env,
        )
        .expect("runs");
        let text = outcome.answer.to_string();
        for form in [secret, "tok%2Fsecret+1", "tok/secret%201", pair.as_str()] {
            assert!(!text.contains(form), "{form} in {text}");
        }
        let written = fs::read_to_string(dir.path().join(history::HISTORY_FILE)).expect("history");
        assert!(!written.contains("tok%2Fsecret+1") && !written.contains(&pair));
    }
}

#[test]
fn a_host_not_granted_is_asked_for_and_nothing_goes_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let draft = ApiRequest::new("POST", "https://api.other.example/v1/items");
    let failure = run(
        Op::Send {
            request: draft,
            environment: None,
            id: None,
        },
        &context(dir.path()),
        &sender,
        &env,
    )
    .expect_err("asks");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.host_not_allowed")
    );
    assert_eq!(
        failure.params.get("host").map(String::as_str),
        Some("api.other.example")
    );
    assert!(sender.sent().is_empty());
    let outcome = run(
        Op::Allow {
            host: "API.other.example".into(),
        },
        &context(dir.path()),
        &sender,
        &env,
    )
    .expect("allowed");
    assert_eq!(
        outcome.allowed_hosts,
        Some(vec![
            "api.other.example".to_owned(),
            "staging.example.com".to_owned()
        ])
    );
}

#[test]
fn a_redirect_to_a_host_not_granted_is_not_followed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sender = FakeSender::answering(vec![
        exchange(302, &[("Location", "/next")], ""),
        exchange(302, &[("Location", "http://192.168.0.1/admin")], ""),
        exchange(200, &[], "internal"),
    ]);
    let outcome = run(
        Op::Send {
            request: ApiRequest::new("GET", "https://staging.example.com/start"),
            environment: None,
            id: None,
        },
        &context(dir.path()),
        &sender,
        &env,
    )
    .expect("answers");
    assert_eq!(sender.sent().len(), 2);
    assert_eq!(outcome.answer["status"], 302);
    assert_eq!(
        outcome.answer["redirects"],
        json!(["https://staging.example.com/next"])
    );
}

#[test]
fn a_missing_variable_is_named_before_anything_goes_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let failure = run(
        Op::Send {
            request: ApiRequest::new("GET", "{{baseUrl}}/{{version}}/x"),
            environment: None,
            id: None,
        },
        &context(dir.path()),
        &sender,
        &env,
    )
    .expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.unresolved"));
    assert_eq!(
        failure.params.get("names").map(String::as_str),
        Some("version")
    );
    assert!(sender.sent().is_empty());
}

#[test]
fn the_state_names_environments_and_variables_never_secret_values() {
    let dir = tempfile::tempdir().expect("temp dir");
    let api = dir.path().join(".rbrowser/api");
    fs::create_dir_all(&api).expect("dir");
    fs::write(
        api.join("http-client.env.json"),
        r#"{"local": {"baseUrl": "http://localhost:3000"}, "staging": {"version": "v2"}}"#,
    )
    .expect("written");
    saved(dir.path());
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let outcome = run(Op::State, &context(dir.path()), &sender, &env).expect("state");
    let state = &outcome.answer;
    assert_eq!(state["environments"], json!(["local", "staging"]));
    assert_eq!(state["environment"], "staging");
    let variables = state["variables"].to_string();
    assert!(variables.contains("\"name\":\"token\",\"secret\":true"));
    assert!(variables.contains("\"name\":\"version\",\"secret\":false"));
    assert!(!state.to_string().contains("tok-secret-1"));
    assert_eq!(state["requests"][0]["name"], "Me");
}

#[test]
fn save_load_and_delete_work_on_the_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let ctx = context(dir.path());
    let mut request = ApiRequest::new("POST", "{{baseUrl}}/items");
    request.name = "Add item".into();
    request.body = Body::Json {
        text: "{\"a\": 1}".into(),
    };
    let saved = run(
        Op::Save {
            id: None,
            folder: Some("items".into()),
            request: request.clone(),
        },
        &ctx,
        &sender,
        &env,
    )
    .expect("saved");
    assert_eq!(saved.answer["id"], "items/add-item.http");
    let loaded = run(
        Op::Load {
            id: "items/add-item.http".into(),
        },
        &ctx,
        &sender,
        &env,
    )
    .expect("loaded");
    assert_eq!(loaded.answer["request"]["name"], "Add item");
    assert!(
        loaded.answer["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("### Add item\n"))
    );
    run(
        Op::Delete {
            id: "items/add-item.http".into(),
        },
        &ctx,
        &sender,
        &env,
    )
    .expect("deleted");
    assert!(
        !dir.path()
            .join(".rbrowser/api/items/add-item.http")
            .exists()
    );
}

#[test]
fn a_send_answers_the_verdicts_of_its_checks_masked() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/me");
    request.checks = vec![
        rb_api_model::Check::Status {
            expected: "2xx".into(),
        },
        rb_api_model::Check::Json {
            path: "$.token".into(),
            compare: rb_api_model::Compare::Equals,
            value: "x".into(),
        },
    ];
    let sender = FakeSender::answering(vec![exchange(200, &[], "{\"token\": \"tok-secret-1\"}")]);
    let mut ctx = context(dir.path());
    ctx.secrets = json!({ "staging": { "token": "tok-secret-1" } });
    request
        .headers
        .push(Header::new("Authorization", "Bearer {{token}}"));
    let outcome = run(
        Op::Send {
            request,
            environment: None,
            id: None,
        },
        &ctx,
        &sender,
        &env,
    )
    .expect("sent");
    assert_eq!(outcome.answer["checks"][0]["passed"], true);
    assert_eq!(outcome.answer["checks"][1]["passed"], false);
    assert_eq!(outcome.answer["checks"][1]["actual"], "***");
    assert_eq!(outcome.findings, None);
}

#[test]
fn a_collection_run_keeps_its_report_and_hands_over_its_findings() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/health");
    request.name = "Health".into();
    request.checks = vec![rb_api_model::Check::Status {
        expected: "200".into(),
    }];
    Store::new(dir.path())
        .save(None, Some("smoke"), &request)
        .expect("saved");
    let sender = FakeSender::answering(vec![exchange(503, &[], "")]);
    let ctx = context(dir.path());
    let outcome = run(
        Op::RunCollection {
            folder: Some("smoke".into()),
            environment: None,
        },
        &ctx,
        &sender,
        &env,
    )
    .expect("ran");
    assert_eq!(outcome.answer["tally"]["failed"], 1);
    assert_eq!(outcome.answer["environment"], "staging");
    let findings = outcome.findings.expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "check-failed");
    let report = run(Op::Report, &ctx, &sender, &env).expect("read");
    assert_eq!(report.answer["report"]["tally"]["failed"], 1);
    assert_eq!(history::read(dir.path()).len(), 0, "a run is no history");
}

#[test]
fn a_benchmark_takes_its_repeats_from_the_settings() {
    let dir = tempfile::tempdir().expect("temp dir");
    let id = saved(dir.path());
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let ctx = context(dir.path());
    let outcome = run(
        Op::Bench {
            id,
            environments: Vec::new(),
        },
        &ctx,
        &sender,
        &env,
    )
    .expect("ran");
    assert_eq!(sender.sent().len(), 3);
    assert_eq!(outcome.answer["repeats"], 3);
    assert_eq!(outcome.answer["series"][0]["environment"], "staging");
}

#[test]
fn a_capture_is_the_windows_with_the_views_capture() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sender = FakeSender::answering(vec![exchange(200, &[], "")]);
    let op = Op::Capture {
        requests: Vec::new(),
        save: true,
        open: false,
    };
    let failure = run(op, &context(dir.path()), &sender, &env).expect_err("not here");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.capture_unavailable")
    );
}
