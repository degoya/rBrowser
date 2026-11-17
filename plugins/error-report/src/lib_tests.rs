use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::config::{STATE_FILE, STATE_PATH};
use crate::feed::FEED_KEY;

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .grant(Capability::Findings);
    host.views = ["mobile", "desktop"]
        .iter()
        .zip(1..)
        .map(|(name, id)| ViewInfo {
            id,
            label: (*name).to_owned(),
            breakpoint: (*name).to_owned(),
            engine: "chromium".to_owned(),
            url: "https://a.test/".to_owned(),
            ..ViewInfo::default()
        })
        .collect();
    host.now = 1_731_300_000;
    host
}

/// The host serves the out folder below `.rbrowser/`: the next call reads the state back.
fn carry_state(host: &mut MemoryHost) {
    if let Some(state) = host.out.get(STATE_FILE).cloned() {
        host.project.insert(STATE_PATH.to_owned(), state);
    }
}

fn args(extra: &Value) -> String {
    let mut args = json!({ FEED_KEY: { "views": [
        { "view": 1, "errors": [{ "text": "Uncaught Error: x", "time": 1_731_299_999_000.0 }],
          "network": [{ "url": "https://a.test/hero-375.png", "status": 404,
                        "resourceType": "Image", "time": 1_731_299_999_100.0 }] },
        { "view": 2, "errors": [{ "text": "Uncaught Error: x", "time": 1_731_299_999_000.0 }] }
    ] } });
    if let (Some(args), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
        args.extend(extra.clone());
    }
    args.to_string()
}

fn call(host: &mut MemoryHost, tool: &str, extra: &Value) -> Value {
    let answer = ErrorReport
        .call_tool(host, tool, &args(extra))
        .expect("answers");
    serde_json::from_str(&answer).expect("JSON")
}

#[test]
fn get_answers_the_report_over_the_views() {
    let mut host = host();
    let answer = call(&mut host, GET, &json!({}));
    assert_eq!(answer["total"], 2);
    assert_eq!(answer["entries"][0]["views"], json!([1, 2]));
    assert_eq!(answer["entries"][1]["onlyIn"], json!(["mobile"]));
    assert!(answer.get("published").is_none());
    assert!(host.out.is_empty(), "a plain get writes nothing");
    let narrowed = call(&mut host, GET, &json!({ "target": 2 }));
    assert_eq!(narrowed["total"], 1);
}

#[test]
fn clear_empties_the_report_until_new_errors_come() {
    let mut host = host();
    let cleared = call(&mut host, CLEAR, &json!({}));
    assert_eq!(cleared["clearedAt"], 1_731_299_999_100.0);
    carry_state(&mut host);
    assert_eq!(call(&mut host, GET, &json!({}))["total"], 0);
}

#[test]
fn run_publishes_files_and_findings_and_shows_the_panel() {
    let mut host = host();
    let answer = ErrorReport
        .run_command(&mut host, RUN, &args(&json!({})))
        .expect("runs");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer[OPEN_KEY], json!({ "panel": PANEL_ID }));
    assert_eq!(answer["published"]["findings"], 3);
    assert_eq!(answer["published"]["findingsReported"], true);
    assert_eq!(answer["total"], 2);
    assert!(answer.get("entries").is_none());
    let report: Value =
        serde_json::from_slice(host.out.get(REPORT_JSON).expect("json written")).expect("JSON");
    assert_eq!(report["total"], 2);
    assert_eq!(report["generatedAt"], 1_731_300_000);
    let page =
        String::from_utf8(host.out.get(REPORT_HTML).cloned().unwrap_or_default()).expect("UTF-8");
    assert!(page.contains("Only in views: mobile"));
    assert_eq!(host.findings.len(), 3);
}

#[test]
fn a_reload_reloads_and_the_next_call_publishes_the_fresh_load() {
    let mut host = host();
    host.evaluate_answer = r#"{"settled":true}"#.to_owned();
    let answer = call(&mut host, GET, &json!({ "reload": true, "target": 1 }));
    assert_eq!(answer["pending"], true);
    assert_eq!(answer["reloaded"][0]["view"], 1);
    assert_eq!(answer["reloaded"][0]["settled"], true);
    assert!(host.evaluated.iter().all(|(view, _)| *view == 1));
    carry_state(&mut host);
    let next = call(&mut host, GET, &json!({}));
    assert_eq!(next["published"]["written"], true);
    assert!(host.out.contains_key(REPORT_HTML));
    carry_state(&mut host);
    host.out.remove(REPORT_HTML);
    let after = call(&mut host, GET, &json!({}));
    assert!(after.get("published").is_none(), "published once");
}

#[test]
fn the_reload_setting_makes_report_now_reload_every_chromium_view() {
    let mut host = host();
    host.views[1].engine = "firefox".to_owned();
    host.settings
        .insert("reloadBeforeReport".to_owned(), "true".to_owned());
    let answer = ErrorReport
        .run_command(&mut host, RUN, &args(&json!({})))
        .expect("runs");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["reloaded"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer[OPEN_KEY]["panel"], PANEL_ID);
}

#[test]
fn checks_run_reports_the_findings_of_its_view() {
    let mut host = host();
    let rules: Vec<String> = ErrorReport
        .describe()
        .into_iter()
        .map(|rule| rule.id)
        .collect();
    let ctx = CheckContext {
        view: Some(1),
        url: Some("https://a.test/".to_owned()),
        rules,
        args: args(&json!({})),
    };
    let found = ErrorReport.run(&mut host, &ctx).expect("runs");
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|f| f.view == Some(1)));
}

#[test]
fn the_panel_command_and_bad_calls() {
    let mut host = host();
    let answer = ErrorReport
        .run_command(&mut host, PANEL, "")
        .expect("answers");
    assert_eq!(
        answer,
        json!({ OPEN_KEY: { "panel": PANEL_ID } }).to_string()
    );
    assert!(ErrorReport.call_tool(&mut host, "nope", "{}").is_err());
    assert!(ErrorReport.run_command(&mut host, "nope", "{}").is_err());
    assert!(ErrorReport.call_tool(&mut host, GET, "[1]").is_err());
    let bad = ErrorReport
        .call_tool(&mut host, GET, r#"{"limit": 0}"#)
        .expect_err("a bad limit");
    assert_eq!(
        bad.code.as_deref(),
        Some("plugin.error-report.errors.bad_args")
    );
    assert!(
        ErrorReport
            .call_tool(&mut host, GET, r#"{"reload": "yes"}"#)
            .is_err()
    );
    assert!(
        ErrorReport
            .call_tool(&mut host, GET, r#"{"$console": {"views": 3}}"#)
            .is_err()
    );
}

#[test]
fn without_a_capture_the_report_says_so() {
    let mut host = host();
    let answer: Value = serde_json::from_str(
        &ErrorReport
            .call_tool(&mut host, GET, "{}")
            .expect("answers"),
    )
    .expect("JSON");
    assert_eq!(answer["captured"], false);
    assert_eq!(answer["total"], 0);
}
