use super::*;
use crate::fake::FakeHost;
use rb_plugin_api::ViewId;
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

#[test]
fn a_stored_preset_fills_like_a_profile_and_a_settings_profile_wins_its_name() {
    // RB-090-70: presets taken from a form live in the plugin's own file.
    let mut host = host();
    host.files.insert(
        crate::store::READ_PATH.to_owned(),
        json!({ "version": 1, "presets": [
            { "name": "taken", "values": { "email": "taken@example.com" } },
            { "name": "testuser", "values": { "email": "shadowed@example.com" } } ] })
        .to_string()
        .into_bytes(),
    );
    let answer = run(&mut host, r#"{"profile":"taken"}"#, Some(Mode::Valid));
    assert_eq!(answer["profile"], "taken");
    assert_eq!(
        answer["views"][0]["fields"][0]["value"],
        "taken@example.com"
    );
    host.set("activeProfile", "\"taken\"");
    let answer = run(&mut host, "{}", Some(Mode::Valid));
    assert_eq!(
        answer["views"][0]["fields"][0]["value"],
        "taken@example.com"
    );
    host.set(
        "profiles",
        r#"[{"name":"testuser","values":{"email":"qa@example.com"}}]"#,
    );
    let answer = run(&mut host, r#"{"profile":"testuser"}"#, Some(Mode::Valid));
    assert_eq!(answer["views"][0]["fields"][0]["value"], "qa@example.com");
}

#[test]
fn hidden_inputs_are_read_for_the_fill_only_with_the_setting() {
    let mut host = host();
    run(&mut host, "{}", Some(Mode::Valid));
    assert_eq!(host.requests(1)[0]["hiddenInputs"], false);
    host.set("captureHiddenInputs", "true");
    run(&mut host, "{}", Some(Mode::Valid));
    assert_eq!(host.requests(1)[2]["hiddenInputs"], true);
}

#[test]
fn each_view_names_the_scope_its_fields_came_from() {
    let mut host = host();
    let mut chosen = register("en-US");
    chosen["scope"] = json!("selector");
    host.collect(1, chosen);
    let answer = run(&mut host, r#"{"seed":3}"#, Some(Mode::Valid));
    assert_eq!(answer["views"][0]["scope"], "selector");
    assert_eq!(
        answer["views"][1]["scope"], "",
        "an answer without one names none"
    );
}

/// A contact form whose topic select brings a company field and a privacy checkbox (RB-094-22):
/// the first read has the select and the name, the second the fields the select brought.
fn contact() -> (Value, Value) {
    let first = json!({ "locale": "en-US", "fields": [
        { "index": 0, "key": "input:text:name", "sig": "n", "tag": "input", "type": "text",
          "name": "name" },
        { "index": 1, "key": "select:select:topic", "sig": "t", "tag": "select", "type": "select",
          "name": "topic", "options": [{ "value": "", "label": "-" }, { "value": "sales", "label": "Sales" }],
          "required": true },
        { "index": 2, "key": "input:text:company", "sig": "c", "tag": "input", "type": "text",
          "name": "company", "skip": "disabled" }
    ] });
    let second = json!({ "locale": "en-US", "fields": [
        { "index": 0, "key": "input:text:name", "sig": "n", "tag": "input", "type": "text",
          "name": "name", "filled": true },
        { "index": 1, "key": "select:select:topic", "sig": "t", "tag": "select", "type": "select",
          "name": "topic", "options": [{ "value": "", "label": "-" }, { "value": "sales", "label": "Sales" }],
          "required": true, "filled": true },
        { "index": 2, "key": "input:text:company", "sig": "c", "tag": "input", "type": "text",
          "name": "company" },
        { "index": 3, "key": "input:text:vat", "sig": "v", "tag": "input", "type": "text",
          "name": "vat", "filled": true },
        { "index": 4, "key": "input:checkbox:privacy", "sig": "p", "tag": "input",
          "type": "checkbox", "name": "privacy", "required": true }
    ] });
    (first, second)
}

fn contact_host() -> FakeHost {
    let (first, second) = contact();
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "chromium")
        .collect(1, first)
        .collect_next(1, second)
        .apply_next(
            1,
            json!({ "fields": [
                { "index": 1, "status": "done", "changed": true, "value": "sales" },
                { "index": 0, "status": "done", "changed": true, "value": "Ada",
                  "record": { "kind": "fill" } } ] }),
        )
        .apply_next(
            1,
            json!({ "formValid": true, "fields": [
                { "index": 4, "status": "done", "changed": true, "value": "true" },
                { "index": 2, "status": "done", "changed": true, "value": "ACME" } ] }),
        );
    host
}

fn ops(host: &FakeHost, view: ViewId) -> Vec<String> {
    host.requests(view)
        .iter()
        .map(|request| request["op"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn indices(request: &Value) -> Vec<u64> {
    request["actions"]
        .as_array()
        .map(|list| list.iter().filter_map(|a| a["index"].as_u64()).collect())
        .unwrap_or_default()
}

#[test]
fn the_fields_a_select_brings_are_filled_in_the_same_run() {
    let mut host = contact_host();
    let answer = run(&mut host, r#"{"seed":5}"#, Some(Mode::Valid));
    // Read, set, read again after the page settled, set what is new, read once more: nothing left.
    assert_eq!(
        ops(&host, 1),
        ["collect", "apply", "collect", "apply", "collect"]
    );
    let applies = host.applies(1);
    // Control fields first in each pass: the select before the name, the checkbox before the text.
    assert_eq!(indices(&applies[0]), [1, 0]);
    assert_eq!(indices(&applies[1]), [4, 2]);
    // Every apply waits for the page to settle after a control field (300 ms by default).
    assert_eq!(applies[0]["settle"]["quiet"], 300);
    let view = &answer["views"][0];
    assert_eq!(view["passes"], 2);
    assert_eq!(view["status"], "filled");
    assert_eq!(view["formValid"], true);
    let fields: Vec<(&str, &str)> = view["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .map(|f| {
            (
                f["key"].as_str().unwrap_or_default(),
                f["status"]
                    .as_str()
                    .unwrap_or(f["skip"].as_str().unwrap_or_default()),
            )
        })
        .collect();
    assert_eq!(
        fields,
        [
            ("input:text:name", "done"),
            ("select:select:topic", "done"),
            ("input:text:company", "done"),
            ("input:text:vat", "filled"),
            ("input:checkbox:privacy", "done"),
        ]
    );
    assert_eq!(view["record"], json!([{ "kind": "fill" }]));
}

#[test]
fn the_passes_stop_at_the_setting_and_a_settle_of_zero_waits_for_nothing() {
    let mut host = contact_host();
    host.set("maxPasses", "1").set("settleMilliseconds", "0");
    let answer = run(&mut host, r#"{"seed":5}"#, Some(Mode::Valid));
    assert_eq!(ops(&host, 1), ["collect", "apply"]);
    assert!(host.applies(1)[0].get("settle").is_none());
    assert_eq!(answer["views"][0]["passes"], 1);
    assert_eq!(answer["views"][0]["fields"][2]["skip"], "disabled");
}

#[test]
fn every_synced_view_runs_the_same_passes_with_the_same_values() {
    let (first, second) = contact();
    let mut host = contact_host();
    host.view(2, "Desktop", "chromium")
        .collect(2, first)
        .collect_next(2, second)
        .apply_next(
            2,
            json!({ "fields": [{ "index": 1, "status": "done", "changed": true }] }),
        );
    run(&mut host, r#"{"seed":5}"#, Some(Mode::Valid));
    let one = host.applies(1);
    let two = host.applies(2);
    assert_eq!(one.len(), 2);
    assert_eq!(two.len(), 2);
    assert_eq!(one[1]["actions"], two[1]["actions"]);
}

#[test]
fn a_fill_stops_its_passes_at_the_overall_deadline_and_says_so() {
    let (first, _) = contact();
    let mut host = contact_host();
    host.view(2, "Desktop", "chromium").collect(2, first);
    // Every evaluate takes 20 s of the clock: the first view's first pass ends past the budget.
    host.tick = 20;
    let answer = run(&mut host, r#"{"seed":5}"#, Some(Mode::Valid));
    assert_eq!(Settings::default().fill_budget_seconds, 30);
    assert_eq!(ops(&host, 1), ["collect", "apply"]);
    assert_eq!(answer["views"][0]["passes"], 1);
    assert_eq!(answer["views"][0]["stopped"], "deadline");
    assert_eq!(answer["views"][1]["status"], "timeout");
    assert!(host.requests(2).is_empty());
    assert_eq!(answer["outOfTime"], true);
    let mut host = contact_host();
    let answer = run(&mut host, r#"{"seed":5}"#, Some(Mode::Valid));
    assert!(answer["views"][0].get("stopped").is_none());
    assert_eq!(answer["outOfTime"], false);
    // `fillBudgetSeconds` (code review w33, item 9): with 300 s the slow page gets its views.
    let (first, _) = contact();
    let mut host = contact_host();
    host.view(2, "Desktop", "chromium").collect(2, first);
    host.set("fillBudgetSeconds", "300");
    host.tick = 20;
    let answer = run(&mut host, r#"{"seed":5}"#, Some(Mode::Valid));
    assert_ne!(answer["views"][1]["status"], "timeout");
    assert!(!host.requests(2).is_empty());
}
