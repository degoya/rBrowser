use rb_plugin_api::codes;
use rb_plugin_api::feature::Feature;
use serde_json::{Value, json};

use super::Snippets;
use crate::fake_host::{FakeHost, evaluated, view};
use crate::last::LAST_PATH;

fn host() -> FakeHost {
    let mut host = FakeHost::with_views(vec![view(1, "Mobile"), view(2, "Desktop")]);
    host.active = Some(2);
    host.set(
        "library",
        &json!([{ "name": "Title", "code": "return document.title" }]).to_string(),
    );
    host.file(
        ".rbrowser/snippets/flag.js",
        "// @name Beta flag\n// @world main\nwindow.beta = true;",
    );
    for id in [1, 2] {
        host.answer(
            id,
            "Page.getFrameTree",
            Ok(r#"{"frameTree":{"frame":{"id":"F"}}}"#.to_owned()),
        );
        host.answer(
            id,
            "Page.createIsolatedWorld",
            Ok(r#"{"executionContextId":7}"#.to_owned()),
        );
        host.answer(
            id,
            "Runtime.evaluate",
            Ok(evaluated(
                &json!({ "ok": true, "type": "string", "text": "\"Home\"", "value": "Home" }),
            )),
        );
    }
    host
}

fn tool(host: &mut FakeHost, name: &str, args: &Value) -> Result<Value, rb_plugin_api::Failure> {
    Snippets
        .call_tool(host, name, &args.to_string())
        .map(|answer| serde_json::from_str(&answer).expect("JSON"))
}

fn command(host: &mut FakeHost, name: &str) -> Result<Value, rb_plugin_api::Failure> {
    Snippets
        .run_command(host, name, "{}")
        .map(|answer| serde_json::from_str(&answer).expect("JSON"))
}

#[test]
fn a_user_snippet_gives_the_title_of_every_view_as_one_result() {
    let mut host = host();
    let answer = tool(
        &mut host,
        "run",
        &json!({ "name": "Title", "target": "all" }),
    )
    .expect("runs");
    assert_eq!(
        answer["snippet"],
        json!({ "source": "user", "name": "Title" })
    );
    assert_eq!(answer["results"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["results"][0]["value"], "Home");
    assert_eq!(answer["results"][0]["views"], json!([1, 2]));
    assert_eq!(answer["world"], "isolated");
    assert_eq!(answer["timeoutMs"], 5000);
}

#[test]
fn a_run_without_target_takes_the_snippets_own() {
    let mut host = host();
    host.set(
        "library",
        &json!([{ "name": "Here", "code": "// @target active\nreturn 1" }]).to_string(),
    );
    let answer = tool(&mut host, "run", &json!({ "name": "Here" })).expect("runs");
    assert_eq!(answer["views"], json!([2]));
}

#[test]
fn a_project_snippet_runs_in_the_page_world_once_trusted() {
    let mut host = host();
    let answer = tool(&mut host, "run", &json!({ "name": "Beta flag" })).expect("runs");
    assert_eq!(answer["world"], "main");
    assert!(
        host.methods()
            .iter()
            .all(|method| method == "Runtime.evaluate")
    );
    host.trusted = false;
    host.sent.clear();
    let failure = tool(&mut host, "run", &json!({ "name": "Beta flag" })).expect_err("untrusted");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.project_untrusted")
    );
    assert!(host.sent.is_empty(), "nothing ran");
    let listed = tool(&mut host, "list", &json!({})).expect("lists");
    assert_eq!(listed["project"], "untrusted");
    assert_eq!(listed["snippets"].as_array().map(Vec::len), Some(1));
}

#[test]
fn code_runs_ad_hoc_with_its_header_and_the_arguments_over_it() {
    let mut host = host();
    let answer = tool(
        &mut host,
        "run",
        &json!({ "code": "// @world main\nreturn location.href", "target": 1, "timeoutMs": 800 }),
    )
    .expect("runs");
    assert_eq!(answer["world"], "main");
    assert_eq!(answer["timeoutMs"], 800);
    assert_eq!(answer["views"], json!([1]));
    assert!(answer.get("snippet").is_none());
    assert!(
        !host.files.contains_key(LAST_PATH),
        "an ad-hoc run is not repeated"
    );
    for (args, reason) in [
        (json!({ "code": "1", "world": "page" }), "world"),
        (json!({ "code": "1", "timeoutMs": 10 }), "timeoutMs"),
        (json!({ "code": "1", "target": "some" }), "target"),
        (json!({}), "name"),
        (json!({ "name": "Title", "source": "web" }), "source"),
    ] {
        let failure = tool(&mut host, "run", &args).expect_err("refused");
        assert_eq!(
            failure.code.as_deref(),
            Some("plugin.snippets.errors.bad_args"),
            "{args}"
        );
        assert_eq!(
            failure.params.get("reason").map(String::as_str),
            Some(reason),
            "{args}"
        );
    }
}

#[test]
fn run_last_repeats_the_last_snippet_in_the_same_views() {
    let mut host = host();
    let failure = command(&mut host, "run_last").expect_err("nothing ran yet");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.no_last")
    );
    // The panel runs view by view and names the views of the whole run.
    tool(
        &mut host,
        "run",
        &json!({ "name": "Title", "target": 2, "remember": [1, 2] }),
    )
    .expect("runs");
    host.sent.clear();
    let answer = command(&mut host, "run_last").expect("repeats");
    assert_eq!(answer["snippet"]["name"], "Title");
    assert_eq!(answer["views"], json!([1, 2]));
    let evaluated_in: Vec<u32> = host
        .sent
        .iter()
        .filter(|sent| sent.1 == "Runtime.evaluate")
        .map(|sent| sent.0)
        .collect();
    assert_eq!(evaluated_in, [1, 2]);
    assert_eq!(answer["$post"][0]["target"], "panel");
    assert_eq!(
        answer["$post"][0]["message"]["ran"]["snippet"]["name"],
        "Title"
    );
}

#[test]
fn run_last_of_a_project_snippet_needs_trust_again() {
    let mut host = host();
    tool(&mut host, "run", &json!({ "name": "Beta flag" })).expect("runs");
    host.trusted = false;
    let failure = command(&mut host, "run_last").expect_err("untrusted now");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.project_untrusted")
    );
}

#[test]
fn the_panel_command_asks_for_the_panel_and_unknown_names_are_refused() {
    let mut host = host();
    assert_eq!(
        command(&mut host, "panel").expect("answers"),
        json!({ "$open": { "panel": "snippets" } })
    );
    let failure = command(&mut host, "nope").expect_err("unknown");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = tool(&mut host, "nope", &json!({})).expect_err("unknown");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = Snippets
        .call_tool(&mut host, "list", "[1]")
        .expect_err("no object");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.bad_args")
    );
}

#[test]
fn save_then_list_with_code() {
    let mut host = host();
    tool(
        &mut host,
        "save",
        &json!({ "name": "Saved", "code": "return 2" }),
    )
    .expect("saved");
    let listed = tool(&mut host, "list", &json!({ "withCode": true })).expect("lists");
    let saved = listed["snippets"]
        .as_array()
        .expect("list")
        .iter()
        .find(|entry| entry["source"] == "local")
        .expect("the saved one")
        .clone();
    assert_eq!(saved["name"], "Saved");
    assert_eq!(saved["code"], "return 2");
    assert_eq!(
        listed["views"][1],
        json!({ "id": 2, "label": "Desktop", "breakpoint": "desktop", "engine": "chromium" })
    );
    assert_eq!(listed["active"], 2);
}
