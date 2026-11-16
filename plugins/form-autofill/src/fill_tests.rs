use super::*;
use crate::fake::FakeHost;
use serde_json::json;

fn register(locale: &str) -> Value {
    json!({
        "locale": locale,
        "url": "http://127.0.0.1:18767/forms/register.html",
        "fields": [
            { "index": 0, "key": "input:email", "sig": "s0", "tag": "input", "type": "email",
              "name": "email", "required": true },
            { "index": 1, "key": "input:zip", "sig": "s1", "tag": "input", "type": "text",
              "name": "zip", "pattern": "[0-9]{5}", "required": true },
            { "index": 2, "key": "input:avatar", "sig": "s2", "tag": "input", "type": "file",
              "name": "avatar", "skip": "file" }
        ]
    })
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "chromium")
        .view(2, "Desktop", "chromium")
        .view(3, "Firefox", "firefox")
        .collect(1, register("de-DE"))
        .collect(2, register("de-DE"));
    host
}

fn run(host: &mut FakeHost, args: &str, mode: Option<Mode>) -> Value {
    let args = Args::parse(args).expect("args");
    fill(host, &args, mode).expect("fills")
}

#[test]
fn every_chromium_view_is_filled_with_the_same_values() {
    let mut host = host();
    let answer = run(&mut host, r#"{"seed":42}"#, Some(Mode::Valid));
    assert_eq!(answer["mode"], "valid");
    assert_eq!(answer["seed"], 42);
    let one = host.applies(1);
    let two = host.applies(2);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0]["actions"], two[0]["actions"]);
    // The file input is not touched.
    assert_eq!(one[0]["actions"].as_array().map(Vec::len), Some(2));
    let views = answer["views"].as_array().expect("views");
    assert_eq!(views.len(), 3);
    assert_eq!(views[0]["status"], "filled");
    assert_eq!(views[0]["locale"], "de-DE");
    assert_eq!(views[2]["status"], "unsupported");
    assert!(host.applies(3).is_empty() && host.requests(3).is_empty());
    let fields = views[0]["fields"].as_array().expect("fields");
    assert_eq!(fields[1]["kind"], "postalCode");
    assert_eq!(fields[1]["value"].as_str().map(str::len), Some(5));
    assert_eq!(fields[2]["skip"], "file");
    assert_eq!(host.progress.last(), Some(&(3, 3)));
}

#[test]
fn the_page_reports_what_the_fields_hold() {
    let mut host = host();
    host.applied.insert(
        1,
        json!({ "formValid": false, "blockedSubmits": 1, "fields": [
            { "index": 0, "status": "done", "value": "held@example.com", "valid": false,
              "errors": ["typeMismatch", 7] } ] }),
    );
    let answer = run(&mut host, r#"{"target":1,"mode":"valid"}"#, None);
    let view = &answer["views"][0];
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(1));
    assert_eq!(view["formValid"], false);
    assert_eq!(view["blockedSubmits"], 1);
    assert_eq!(view["fields"][0]["value"], "held@example.com");
    assert_eq!(view["fields"][0]["valid"], false);
    assert_eq!(view["fields"][0]["errors"], json!(["typeMismatch"]));
    assert_eq!(view["fields"][1]["valid"], Value::Null);
}

#[test]
fn an_invalid_run_names_its_strategy_and_violations() {
    let mut host = host();
    let answer = run(
        &mut host,
        r##"{"mode":"invalid","selector":"#register"}"##,
        None,
    );
    assert_eq!(answer["strategy"], "mixed");
    let request = &host.applies(1)[0];
    assert_eq!(request["selector"], "#register");
    assert_eq!(host.requests(1)[0]["selector"], "#register");
    let fields = answer["views"][0]["fields"].as_array().expect("fields");
    assert!(fields[0]["violation"].is_string());
    assert!(fields[1]["violation"].is_string());
}

#[test]
fn a_seed_of_zero_takes_a_new_one_from_the_clock() {
    let mut host = host();
    host.now = 1_700_000_123;
    let answer = run(&mut host, "{}", Some(Mode::Valid));
    assert!(answer["seed"].as_u64().is_some_and(|seed| seed > 0));
    host.set("seed", "77");
    assert_eq!(run(&mut host, "{}", Some(Mode::Valid))["seed"], 77);
}

#[test]
fn a_profile_must_exist() {
    let mut host = host();
    host.set(
        "profiles",
        r#"[{"name":"testuser","values":{"email":"qa@example.com"}}]"#,
    );
    let answer = run(&mut host, r#"{"profile":"testuser"}"#, Some(Mode::Valid));
    assert_eq!(answer["profile"], "testuser");
    assert_eq!(answer["views"][0]["fields"][0]["value"], "qa@example.com");
    assert_eq!(answer["views"][0]["fields"][0]["profile"], true);
    let failure = fill(
        &mut host,
        &Args::parse(r#"{"profile":"ghost"}"#).expect("args"),
        Some(Mode::Valid),
    )
    .expect_err("unknown");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.profile_unknown")
    );
}

#[test]
fn the_tool_needs_a_mode_and_an_unknown_view_is_not_found() {
    let mut host = host();
    let failure = fill(&mut host, &Args::parse("{}").expect("args"), None).expect_err("no mode");
    assert_eq!(failure.code.as_deref(), Some(codes::INVALID));
    let failure = fill(
        &mut host,
        &Args::parse(r#"{"target":9}"#).expect("args"),
        Some(Mode::Valid),
    )
    .expect_err("no view");
    assert_eq!(failure.code.as_deref(), Some(codes::NOT_FOUND));
}

#[test]
fn a_page_error_is_a_status_of_its_view() {
    let mut host = host();
    host.collect(2, json!({ "error": "selector" }));
    host.collected.insert(
        1,
        Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", "timeout")),
    );
    let answer = run(&mut host, r###"{"selector":"##"}"###, Some(Mode::Valid));
    assert_eq!(answer["views"][0]["status"], "failed");
    assert_eq!(answer["views"][0]["error"], codes::HOST_FAILED);
    assert_eq!(answer["views"][1]["status"], "selector");
    assert!(host.applies(1).is_empty() && host.applies(2).is_empty());
}

#[test]
fn the_answer_is_posted_to_the_panel() {
    let mut host = host();
    let answer = run(&mut host, "{}", Some(Mode::Valid));
    let posted = answer_with_post(answer.clone());
    assert_eq!(posted["$post"][0]["target"], "panel");
    assert_eq!(posted["$post"][0]["message"]["type"], "filled");
    assert_eq!(posted["$post"][0]["message"]["result"], answer);
}

#[test]
fn the_answer_names_the_forms_and_each_field_its_form_and_whether_it_is_required() {
    let mut host = host();
    let mut page = register("en-US");
    page["forms"] = json!([{ "selector": "#register", "name": "Register" }]);
    page["fields"][0]["form"] = json!(0);
    host.collect(1, page);
    host.collect(
        2,
        json!({ "error": "not-found", "forms": [{ "selector": "#login", "name": "" }] }),
    );
    let answer = run(&mut host, "{}", Some(Mode::Valid));
    let views = answer["views"].as_array().expect("views");
    assert_eq!(
        views[0]["forms"],
        json!([{ "selector": "#register", "name": "Register" }])
    );
    assert_eq!(views[0]["fields"][0]["form"], 0);
    assert_eq!(views[0]["fields"][0]["required"], true);
    assert_eq!(views[0]["fields"][2]["form"], Value::Null);
    assert_eq!(views[0]["fields"][2]["required"], false);
    // A selector that finds nothing still names the page's forms to choose from.
    assert_eq!(views[1]["status"], "not-found");
    assert_eq!(views[1]["forms"][0]["selector"], "#login");
    assert!(views[2].get("forms").is_none());
}
