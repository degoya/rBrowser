use std::fs;

use serde_json::{Map, json};

use super::*;
use crate::api_client::ops::Settings;

fn context(root: &std::path::Path) -> Context {
    Context {
        root: Some(root.to_path_buf()),
        trusted: true,
        environment: Some("staging".into()),
        environments: vec![("staging".into(), "https://staging.example.com/".into())],
        allowed_hosts: Vec::new(),
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

fn get(url: &str, token: &str) -> Option<Captured> {
    Some(Captured {
        method: "GET".into(),
        url: url.into(),
        headers: vec![
            ("Authorization".into(), format!("Bearer {token}")),
            ("Accept".into(), "application/json".into()),
        ],
        body: None,
        body_unread: false,
    })
}

const SAVE: Wanted = Wanted {
    save: true,
    open: false,
    keep: true,
};

#[test]
fn saving_writes_a_file_per_request_in_a_folder_per_host_and_keeps_the_secrets_private() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let taken = run(
        &ctx,
        vec![
            get(
                "https://api.example.test/v1/users?page=2",
                "tok-live-123456",
            ),
            get("https://cdn.example.test/v1/config", "tok-other-654321"),
        ],
        SAVE,
    )
    .expect("saves");
    let saved = taken.answer["saved"].as_array().expect("saved list");
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0]["id"], "api-example-test/get-v1-users.http");
    assert_eq!(saved[0]["name"], "GET /v1/users");
    assert_eq!(saved[1]["id"], "cdn-example-test/get-v1-config.http");
    let text = fs::read_to_string(
        dir.path()
            .join(".rbrowser/api/api-example-test/get-v1-users.http"),
    )
    .expect("the file is written");
    assert!(text.contains("{{api-example-test-token}}"), "{text}");
    assert!(!text.contains("tok-live-123456"), "{text}");
    let secrets = taken.secrets.expect("the values are kept");
    assert_eq!(
        secrets["$shared"]["api-example-test-token"],
        "tok-live-123456"
    );
    assert_eq!(secrets["staging"]["token"], "tok-secret-1");
    assert_eq!(
        taken.answer["variables"][0]["name"],
        "api-example-test-token"
    );
    assert_eq!(taken.answer["variables"][0]["value"], "kept");
    let answer = taken.answer.to_string();
    assert!(
        !answer.contains("tok-live-123456") && !answer.contains("tok-other-654321"),
        "{answer}"
    );
    assert!(taken.draft.is_none());
}

#[test]
fn a_duplicate_is_saved_once_and_one_saved_before_is_skipped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let first = run(
        &ctx,
        vec![
            get(
                "https://api.example.test/v1/users?page=1",
                "tok-live-123456",
            ),
            get(
                "https://api.example.test/v1/users?page=2",
                "tok-live-123456",
            ),
        ],
        SAVE,
    )
    .expect("saves");
    assert_eq!(first.answer["saved"].as_array().map(Vec::len), Some(1));
    assert_eq!(first.answer["skipped"][0]["reason"], "duplicate");
    let again = run(
        &ctx,
        vec![get("https://api.example.test/v1/users", "tok-live-123456")],
        SAVE,
    )
    .expect("runs");
    assert_eq!(again.answer["saved"].as_array().map(Vec::len), Some(0));
    assert_eq!(again.answer["skipped"][0]["reason"], "exists");
    assert_eq!(again.answer["skipped"][0]["name"], "GET /v1/users");
}

#[test]
fn without_keeping_no_value_is_written_and_the_variable_says_so() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut ctx = context(dir.path());
    ctx.secrets = json!({ "$shared": { "api-example-test-token": "older-one" } });
    let taken = run(
        &ctx,
        vec![
            get("https://api.example.test/v1/me", "tok-live-123456"),
            Some(Captured {
                method: "GET".into(),
                url: "https://api.example.test/v1/feed".into(),
                headers: vec![("X-Api-Key".into(), "key-plain-one".into())],
                body: None,
                body_unread: false,
            }),
        ],
        Wanted {
            keep: false,
            ..SAVE
        },
    )
    .expect("saves");
    assert!(taken.secrets.is_none());
    assert_eq!(taken.answer["variables"][0]["value"], "present");
    assert_eq!(
        taken.answer["variables"][1]["name"],
        "api-example-test-x-api-key"
    );
    assert_eq!(taken.answer["variables"][1]["value"], "missing");
}

#[test]
fn a_masked_value_is_never_written_and_the_variable_is_missing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let taken = run(
        &ctx,
        vec![Some(Captured {
            method: "GET".into(),
            url: "https://api.example.test/v1/feed?access_token=***".into(),
            headers: Vec::new(),
            body: None,
            body_unread: false,
        })],
        SAVE,
    )
    .expect("saves");
    assert!(taken.secrets.is_none());
    assert_eq!(
        taken.answer["variables"][0]["name"],
        "api-example-test-access-token"
    );
    assert_eq!(taken.answer["variables"][0]["value"], "missing");
}

#[test]
fn an_open_hands_the_draft_over_and_writes_no_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let taken = run(
        &ctx,
        vec![get("https://api.example.test/v1/me", "tok-live-123456")],
        Wanted {
            save: false,
            open: true,
            keep: true,
        },
    )
    .expect("opens");
    assert_eq!(taken.answer["request"]["name"], "GET /v1/me");
    assert_eq!(taken.answer["folder"], "api-example-test");
    assert_eq!(taken.draft.as_ref(), Some(&taken.answer));
    assert!(taken.secrets.is_some());
    assert!(!dir.path().join(".rbrowser/api").exists());
    assert!(!taken.answer.to_string().contains("tok-live-123456"));
}

#[test]
fn a_read_answers_the_request_without_a_draft() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let taken = run(
        &ctx,
        vec![get("https://api.example.test/v1/me", "tok-live-123456")],
        Wanted {
            save: false,
            open: false,
            keep: false,
        },
    )
    .expect("reads");
    assert_eq!(taken.answer["request"]["method"], "GET");
    assert!(taken.draft.is_none());
    assert!(taken.secrets.is_none());
}

#[test]
fn a_request_the_views_no_longer_hold_is_skipped_and_an_open_of_one_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let taken = run(&ctx, vec![None], SAVE).expect("runs");
    assert_eq!(taken.answer["skipped"][0]["reason"], "gone");
    let open = Wanted {
        save: false,
        open: true,
        keep: true,
    };
    let failure = run(&ctx, vec![None], open).expect_err("nothing to open");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.capture_gone")
    );
}

#[test]
fn a_project_trusted_and_a_bounded_list_are_needed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut ctx = context(dir.path());
    let many: Vec<Option<Captured>> = (0..=MAX_CAPTURES).map(|_| None).collect();
    let failure = run(&ctx, many, SAVE).expect_err("too many");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.too_many_captures")
    );
    let failure = run(&ctx, Vec::new(), SAVE).expect_err("none");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.nothing_captured")
    );
    ctx.trusted = false;
    let failure = run(&ctx, vec![None], SAVE).expect_err("untrusted");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.untrusted"));
    ctx.root = None;
    let failure = run(&ctx, vec![None], SAVE).expect_err("no project");
    assert_eq!(failure.code(), Some("plugin.api-client.errors.no_project"));
}

#[test]
fn the_setting_chooses_private_values_unless_it_says_drop() {
    let mut values = Map::new();
    assert_eq!(mode_of(&values), SecretMode::Private);
    values.insert(CAPTURE_SECRETS_KEY.into(), json!("drop"));
    assert_eq!(mode_of(&values), SecretMode::Drop);
    values.insert(CAPTURE_SECRETS_KEY.into(), json!("anything"));
    assert_eq!(mode_of(&values), SecretMode::Private);
}

#[test]
fn a_draft_is_taken_once() {
    put_draft(json!({ "request": { "method": "GET" } }));
    assert_eq!(
        take_draft(),
        Some(json!({ "request": { "method": "GET" } }))
    );
    assert_eq!(take_draft(), None);
}

#[test]
fn a_value_too_long_for_the_private_values_is_not_kept() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let long = "c".repeat(MAX_SECRET_CHARS + 1);
    let taken = run(
        &ctx,
        vec![Some(Captured {
            method: "GET".into(),
            url: "https://api.example.test/v1".into(),
            headers: vec![("Cookie".into(), long)],
            body: None,
            body_unread: false,
        })],
        SAVE,
    )
    .expect("saves");
    assert!(taken.secrets.is_none());
    assert_eq!(taken.answer["variables"][0]["value"], "missing");
}

/// The w31 review: a request saved before is found again although its path held a secret the
/// saved file holds as a variable.
#[test]
fn a_request_with_a_secret_in_its_path_is_found_saved_before() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let url = "https://api.example.test/v1/keys/tok-live-123456";
    let first = run(&ctx, vec![get(url, "tok-live-123456")], SAVE).expect("saves");
    assert_eq!(first.answer["saved"].as_array().map(Vec::len), Some(1));
    let again = run(&ctx, vec![get(url, "tok-live-123456")], SAVE).expect("runs");
    assert_eq!(again.answer["saved"].as_array().map(Vec::len), Some(0));
    assert_eq!(again.answer["skipped"][0]["reason"], "exists");
}

/// The w31 review: a value shorter than the sweep's minimum is a variable where it stood, and
/// the same text elsewhere is not masked into `***`.
#[test]
fn a_short_value_does_not_mask_other_text_of_the_answer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let read = Wanted {
        save: false,
        open: false,
        keep: false,
    };
    let taken = run(
        &ctx,
        vec![Some(Captured {
            method: "POST".into(),
            url: "https://api.example.test/v1/admin/login".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(r#"{"user":"admin","password":"admin"}"#.into()),
            body_unread: false,
        })],
        read,
    )
    .expect("reads");
    let answer = taken.answer.to_string();
    assert!(!answer.contains("***"), "{answer}");
    assert_eq!(
        taken.answer["request"]["url"],
        "https://api.example.test/v1/admin/login"
    );
    let body = taken.answer["request"]["body"]["text"]
        .as_str()
        .expect("a JSON body");
    assert!(body.contains("{{api-example-test-password}}"), "{body}");
    assert!(body.contains(r#""user": "admin""#), "{body}");
}

/// The w31 review: a body the view could not hand over, or one that could not be checked, is
/// said in the answer of an open and of a save.
#[test]
fn a_missing_body_is_said() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let unread = || {
        Some(Captured {
            method: "POST".into(),
            url: "https://api.example.test/v1/upload".into(),
            headers: vec![("Content-Length".into(), "42".into())],
            body: None,
            body_unread: true,
        })
    };
    let read = Wanted {
        save: false,
        open: true,
        keep: false,
    };
    let opened = run(&ctx, vec![unread()], read).expect("opens");
    assert_eq!(opened.answer["bodyMissing"], "unread");
    assert_eq!(
        opened.draft.as_ref().map(|draft| &draft["bodyMissing"]),
        Some(&json!("unread"))
    );
    let saved = run(&ctx, vec![unread()], SAVE).expect("saves");
    assert_eq!(saved.answer["saved"][0]["bodyMissing"], "unread");
    let plain = run(
        &ctx,
        vec![get("https://api.example.test/v1/me", "tok-live-123456")],
        read,
    )
    .expect("opens");
    assert_eq!(plain.answer["bodyMissing"], Value::Null);
}

/// The w31 review: the private values could not be written after the files were - the answer
/// stands, says so, and no variable claims a kept value.
#[test]
fn values_not_kept_are_said_and_no_variable_claims_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let open = Wanted {
        save: false,
        open: true,
        keep: true,
    };
    let mut taken = run(
        &ctx,
        vec![get("https://api.example.test/v1/me", "tok-live-123456")],
        open,
    )
    .expect("opens");
    assert_eq!(taken.answer["variables"][0]["value"], "kept");
    not_kept(&mut taken);
    assert!(taken.secrets.is_none());
    for value in [&taken.answer, taken.draft.as_ref().expect("a draft")] {
        assert_eq!(value["variables"][0]["value"], "missing");
        assert_eq!(value["warning"], "secrets_not_saved");
    }
    let mut saved = run(
        &ctx,
        vec![get("https://api.example.test/v1/me", "tok-live-123456")],
        SAVE,
    )
    .expect("saves");
    not_kept(&mut saved);
    assert_eq!(saved.answer["saved"].as_array().map(Vec::len), Some(1));
    assert_eq!(saved.answer["variables"][0]["value"], "missing");
    assert_eq!(saved.answer["warning"], "secrets_not_saved");
}

/// The w31 security review: a `{{` of the page is skipped as `template`, never saved or opened.
#[test]
fn a_request_with_a_template_of_the_page_is_skipped_and_not_opened() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ctx = context(dir.path());
    let page = || {
        get(
            "https://api.example.test/v1?q={{$secret x}}",
            "tok-live-123456",
        )
    };
    let taken = run(&ctx, vec![page()], SAVE).expect("runs");
    assert_eq!(taken.answer["skipped"][0]["reason"], "template");
    assert_eq!(taken.answer["saved"].as_array().map(Vec::len), Some(0));
    let open = Wanted {
        save: false,
        open: true,
        keep: true,
    };
    let failure = run(&ctx, vec![page()], open).expect_err("nothing to open");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.capture_template")
    );
}

#[test]
fn a_value_kept_in_place_of_another_says_so_and_the_path_token_stays_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut ctx = context(dir.path());
    ctx.secrets = json!({ "$shared": { "api-example-test-token": "older-one" } });
    let link = "aB3dE5fG7hI9jK1lM3nO";
    let url = format!("https://api.example.test/reset/{link}");
    let taken = run(&ctx, vec![get(&url, "tok-live-123456")], SAVE).expect("saves");
    let variables = taken.answer["variables"].as_array().expect("variables");
    let state = |name: &str| {
        variables
            .iter()
            .find(|one| one["name"] == name)
            .map(|one| one["value"].clone())
    };
    assert_eq!(state("api-example-test-token"), Some(json!("replaced")));
    assert_eq!(state("api-example-test-reset"), Some(json!("kept")));
    let id = taken.answer["saved"][0]["id"].as_str().expect("an id");
    assert!(!id.contains(link), "{id}");
    let answer = taken.answer.to_string();
    assert!(!answer.contains(link), "{answer}");
    let mut taken = taken;
    not_kept(&mut taken);
    assert!(
        taken.answer["variables"]
            .as_array()
            .expect("variables")
            .iter()
            .all(|one| one["value"] == "missing")
    );
}

#[test]
fn the_log_mask_learns_every_captured_value_with_a_bounded_dedupe_set() {
    for n in 0..=MAX_REGISTERED {
        register(&format!("w31-bounded-{n:06}"));
    }
    assert!(lock(&REGISTERED).len() <= MAX_REGISTERED);
    // Past the dedupe set's bound a value is still masked in the log (the w32 review).
    register("w32-one-past-the-limit");
    let line = rb_diag::redact::redact("sent w32-one-past-the-limit away");
    assert!(!line.contains("w32-one-past-the-limit"), "{line}");
    assert!(lock(&REGISTERED).len() <= MAX_REGISTERED);
}
