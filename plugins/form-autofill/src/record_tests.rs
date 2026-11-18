use super::*;
use crate::args::{Args, Mode};
use crate::fake::FakeHost;
use crate::fill::{answer_with_post, fill};
use serde_json::json;

fn page() -> Value {
    json!({
        "locale": "en-US",
        "url": "http://127.0.0.1:18767/forms/register.html",
        "fields": [
            { "index": 0, "key": "input:email", "sig": "s0", "tag": "input", "type": "email",
              "name": "email", "required": true },
            { "index": 1, "key": "input:password", "sig": "s1", "tag": "input", "type": "password",
              "name": "password" },
            { "index": 2, "key": "input:terms", "sig": "s2", "tag": "input", "type": "checkbox",
              "name": "terms" }
        ]
    })
}

fn target(kind: &str, path: &str) -> Value {
    json!({ "element": { "kind": kind, "path": path }, "offset": { "x": 0.5, "y": 0.5 } })
}

/// What the page script answers `apply` with: each field done, with what the recorder takes.
fn applied() -> Value {
    json!({ "formValid": true, "blockedSubmits": 0, "fields": [
        { "index": 0, "status": "done", "value": "ada@example.com", "valid": true, "errors": [],
          "record": { "target": target("css", "#email"), "kind": "fill", "value": "ada@example.com" } },
        { "index": 1, "status": "done", "value": "", "valid": true, "errors": [],
          "record": { "target": target("css", "#password"), "kind": "fill", "sensitive": true } },
        { "index": 2, "status": "changed", "record": { "target": target("css", "#terms"), "kind": "check" } }
    ] })
}

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "chromium")
        .view(2, "Desktop", "chromium")
        .view(3, "Firefox", "firefox")
        .collect(1, page())
        .collect(2, page());
    host.applied.insert(1, applied());
    host.applied.insert(2, applied());
    host
}

fn posted(host: &mut FakeHost) -> Value {
    let args = Args::parse(r#"{"seed":7}"#).expect("args");
    answer_with_post(fill(host, &args, Some(Mode::Valid)).expect("fills"))
}

fn records(posted: &Value) -> Vec<Value> {
    posted["$post"]
        .as_array()
        .expect("posts")
        .iter()
        .filter(|post| post["target"] == "host")
        .map(|post| post["message"].clone())
        .collect()
}

#[test]
fn every_filled_view_reports_its_set_fields_to_the_recorder_in_a_host_post() {
    let mut host = host();
    let posted = posted(&mut host);
    let records = records(&posted);
    // The Firefox view filled nothing; a field the page found changed is not reported.
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["action"], RECORD_FILL);
    assert_eq!(records[0]["view"], 1);
    assert_eq!(records[1]["view"], 2);
    let fields = records[0]["fields"].as_array().expect("fields");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["value"], "ada@example.com");
    assert_eq!(fields[0]["target"]["element"]["path"], "#email");
    assert_eq!(fields[1]["sensitive"], true);
    assert!(fields[1].get("value").is_none());
}

#[test]
fn the_answer_and_the_panel_see_no_record() {
    let mut host = host();
    let posted = posted(&mut host);
    assert_eq!(posted["$post"][0]["target"], "panel");
    for view in posted["views"].as_array().expect("views") {
        assert!(view.get("record").is_none(), "{view}");
    }
    let result = &posted["$post"][0]["message"]["result"];
    assert!(!result.to_string().contains("\"record\""), "{result}");
}

#[test]
fn a_view_with_many_fields_is_split_into_posts_under_the_hosts_limit() {
    let long = "x".repeat(9_000);
    let fields: Vec<Value> = (0..40)
        .map(|at| json!({ "target": target("css", &format!("#f{at}")), "kind": "fill", "value": long }))
        .collect();
    let mut answer = json!({ "views": [ { "view": 4, "record": fields },
                                          { "view": 5, "record": [ { "kind": "fill" }, 7 ] } ] });
    let posts = take_posts(&mut answer);
    assert!(answer["views"][0].get("record").is_none());
    let sizes: Vec<usize> = posts
        .iter()
        .map(|post| post["message"].to_string().len())
        .collect();
    assert!(sizes.iter().all(|size| *size <= MAX_MESSAGE), "{sizes:?}");
    let total: usize = posts
        .iter()
        .filter(|post| post["message"]["view"] == 4)
        .map(|post| post["message"]["fields"].as_array().map_or(0, Vec::len))
        .sum();
    assert_eq!(total, 40);
    // An entry that is no object is not passed on.
    let five: Vec<&Value> = posts
        .iter()
        .filter(|post| post["message"]["view"] == 5)
        .collect();
    assert_eq!(five.len(), 1);
    assert_eq!(
        five[0]["message"]["fields"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn a_field_over_the_limit_and_posts_past_the_most_are_left_out() {
    let huge = "x".repeat(MAX_MESSAGE + 1);
    let mut answer = json!({ "views": [ { "view": 1, "record": [
        { "kind": "fill", "value": huge }, { "kind": "check" } ] } ] });
    let posts = take_posts(&mut answer);
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0]["message"]["fields"][0]["kind"], "check");
    let views: Vec<Value> = (0..40)
        .map(|view| json!({ "view": view, "record": [ { "kind": "check" } ] }))
        .collect();
    let mut answer = json!({ "views": views });
    assert_eq!(take_posts(&mut answer).len(), MAX_POSTS);
}
