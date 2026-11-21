use rb_api_model::{ApiRequest, Auth, Check, Compare};

use super::*;
use crate::api_client::deliver::Courier;
use crate::api_client::fakes::{UrlSender, answer, context, no_env};
use crate::api_client::store::Store;

fn save(store: &Store, folder: Option<&str>, name: &str, url: &str, checks: Vec<Check>) -> String {
    let mut request = ApiRequest::new("GET", url);
    request.name = name.into();
    request.checks = checks;
    store.save(None, folder, &request).expect("saved")
}

fn status(expected: &str) -> Check {
    Check::Status {
        expected: expected.into(),
    }
}

#[test]
fn a_run_sends_the_folder_in_order_and_judges_every_check() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let ok = save(
        &store,
        Some("users"),
        "A list",
        "{{baseUrl}}/users",
        vec![
            status("2xx"),
            Check::Json {
                path: "$[0].name".into(),
                compare: Compare::Equals,
                value: "Ada".into(),
            },
        ],
    );
    let bad = save(
        &store,
        Some("users"),
        "B one",
        "{{baseUrl}}/users/9",
        vec![status("200")],
    );
    save(&store, None, "Outside", "{{baseUrl}}/other", Vec::new());
    let sender = UrlSender::new(vec![
        (
            "https://staging.example.com/users/9",
            answer(404, &[], "{}", 9.0),
        ),
        (
            "https://staging.example.com/users",
            answer(200, &[], r#"[{"name":"Ada"}]"#, 12.0),
        ),
    ]);
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let report = run(courier, Some("users/"), Some("staging".into())).expect("ran");
    assert_eq!(
        sender.urls(),
        [
            "https://staging.example.com/users",
            "https://staging.example.com/users/9"
        ]
    );
    assert_eq!(report.folder.as_deref(), Some("users"));
    assert_eq!(report.requests[0].id, ok);
    assert!(report.requests[0].passed);
    assert_eq!(report.requests[0].checks.len(), 2);
    assert_eq!(report.requests[1].id, bad);
    assert!(!report.requests[1].passed);
    assert_eq!(report.requests[1].status, Some(404));
    assert_eq!(report.requests[1].checks[0].actual.as_deref(), Some("404"));
    assert_eq!(
        report.tally,
        Tally {
            requests: 2,
            passed: 1,
            failed: 1,
            errors: 0,
            checks: 3,
            checks_failed: 1,
        }
    );
}

#[test]
fn a_request_that_cannot_go_out_is_an_error_and_the_run_goes_on() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    save(
        &store,
        None,
        "A foreign",
        "https://elsewhere.example.net/",
        Vec::new(),
    );
    save(&store, None, "B missing", "{{nowhere}}/x", Vec::new());
    save(&store, None, "C down", "{{baseUrl}}/down", Vec::new());
    save(
        &store,
        None,
        "D fine",
        "https://api.example.com/",
        Vec::new(),
    );
    let sender = UrlSender::new(vec![(
        "https://api.example.com/",
        answer(204, &[], "", 3.0),
    )]);
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let report = run(courier, None, Some("staging".into())).expect("ran");
    let codes: Vec<Option<&str>> = report
        .requests
        .iter()
        .map(|ran| ran.error.as_ref().and_then(Failure::code))
        .collect();
    assert_eq!(
        codes,
        [
            Some("plugin.api-client.errors.host_not_allowed"),
            Some("plugin.api-client.errors.unresolved"),
            Some("plugin.api-client.errors.send_failed"),
            None,
        ]
    );
    assert!(report.requests[3].passed);
    assert_eq!(report.tally.errors, 3);
    assert_eq!(report.tally.passed, 1);
    assert_eq!(
        sender.urls(),
        [
            "https://staging.example.com/down",
            "https://api.example.com/"
        ]
    );
}

#[test]
fn a_secret_never_reaches_the_report() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let mut request = ApiRequest::new("GET", "{{baseUrl}}/me?t={{token}}");
    request.name = "Me".into();
    request.auth = Auth::Bearer {
        token: "{{token}}".into(),
    };
    request.checks = vec![Check::Json {
        path: "$.seen".into(),
        compare: Compare::Equals,
        value: "nothing".into(),
    }];
    store.save(None, None, &request).expect("saved");
    let sender = UrlSender::new(vec![(
        "https://staging.example.com/me",
        answer(200, &[], r#"{"seen":"tok-secret-1"}"#, 5.0),
    )]);
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let report = run(courier, None, Some("staging".into())).expect("ran");
    let text = serde_json::to_string(&report).expect("serialises");
    assert!(!text.contains("tok-secret-1"), "{text}");
    assert_eq!(report.requests[0].checks[0].actual.as_deref(), Some("***"));
    assert_eq!(
        report.requests[0].sent_url.as_deref(),
        Some("https://staging.example.com/me?t=***")
    );
}

#[test]
fn an_encoded_secret_never_reaches_the_report_or_the_findings() {
    use base64::Engine as _;
    use rb_api_model::KeyPlace;

    let secret = "tok/secret 1";
    let pair = base64::engine::general_purpose::STANDARD.encode(format!("ada:{secret}"));
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let failing = Check::Json {
        path: "$.seen".into(),
        compare: Compare::Equals,
        value: "nothing".into(),
    };
    let mut keyed = ApiRequest::new("GET", "{{baseUrl}}/key");
    keyed.name = "A key".into();
    keyed.auth = Auth::ApiKey {
        name: "key".into(),
        value: "{{token}}".into(),
        place: KeyPlace::Query,
    };
    keyed.checks = vec![failing.clone()];
    store.save(None, None, &keyed).expect("saved");
    let mut basic = ApiRequest::new("GET", "{{baseUrl}}/basic");
    basic.name = "B basic".into();
    basic.auth = Auth::Basic {
        username: "ada".into(),
        password: "{{token}}".into(),
    };
    basic.checks = vec![failing];
    store.save(None, None, &basic).expect("saved");
    let echo = format!(r#"{{"seen":"tok%2Fsecret+1 Basic {pair}"}}"#);
    let sender = UrlSender::new(vec![(
        "https://staging.example.com/",
        answer(200, &[], &echo, 5.0),
    )]);
    let mut ctx = context(dir.path());
    ctx.secrets = serde_json::json!({ "staging": { "token": secret } });
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let report = run(courier, None, Some("staging".into())).expect("ran");
    assert_eq!(report.requests.len(), 2);
    keep(dir.path(), &report).expect("kept");
    let found = crate::api_client::findings::of(&report);
    assert!(!found.is_empty());
    let texts = [
        serde_json::to_string(&report).expect("serialises"),
        serde_json::to_string(&found).expect("serialises"),
        last(dir.path())
            .map(|kept| serde_json::to_string(&kept).expect("serialises"))
            .unwrap_or_default(),
    ];
    for text in &texts {
        for form in [secret, "tok%2Fsecret+1", pair.as_str()] {
            assert!(!text.contains(form), "{form} in {text}");
        }
    }
    assert_eq!(
        report.requests[0].sent_url.as_deref(),
        Some("https://staging.example.com/key?key=***")
    );
}

#[test]
fn a_folder_must_be_a_folder_name_and_selects_its_own_requests() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    save(&store, Some("users"), "One", "{{baseUrl}}/a", Vec::new());
    save(
        &store,
        Some("users-old"),
        "Two",
        "{{baseUrl}}/b",
        Vec::new(),
    );
    let sender = UrlSender::new(vec![("https://", answer(200, &[], "", 1.0))]);
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let report = run(courier, Some("users"), None).expect("ran");
    assert_eq!(report.requests.len(), 1);
    let failure = run(courier, Some("../etc"), None).expect_err("refused");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.bad_folder"));
}

#[test]
fn the_last_report_is_kept_and_read_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(last(dir.path()), None);
    let report = Report {
        at: 1,
        folder: None,
        environment: Some("staging".into()),
        tally: Tally::default(),
        requests: Vec::new(),
        cut: false,
    };
    keep(dir.path(), &report).expect("kept");
    assert_eq!(last(dir.path()), Some(report));
}
