use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

const OLD: &str = "20261101-090000";
const NEW: &str = "20261112-100000";

fn two_runs() -> FakeHost {
    FakeHost::with_views(&[(1, "mobile", 375), (2, "desktop", 1280)])
        .with_run(OLD, &[("mobile", 40), ("desktop", 70)])
        .with_run(NEW, &[("desktop", 90), ("mobile", 80)])
}

fn answer(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

#[test]
fn open_asks_the_host_for_the_newest_report_of_the_active_breakpoint() {
    let mut host = two_runs();
    let opened = answer(&tool(&mut host, r#"{"action":"open"}"#).expect("opened"));
    assert_eq!(opened["run"], NEW);
    assert_eq!(opened["breakpoint"], "mobile");
    assert_eq!(
        opened["$report"],
        json!({ "open": format!("{NEW}/mobile.html"), "window": "single" })
    );
}

#[test]
fn open_takes_a_run_a_breakpoint_and_the_window_setting() {
    let mut host = two_runs().set("reportWindow", "\"perReport\"");
    let args = json!({ "action": "open", "run": OLD, "breakpoint": "desktop" }).to_string();
    let opened = answer(&tool(&mut host, &args).expect("opened"));
    assert_eq!(opened["$report"]["open"], format!("{OLD}/desktop.html"));
    assert_eq!(opened["$report"]["window"], "perReport");
}

#[test]
fn the_target_view_names_the_breakpoint() {
    let mut host = two_runs();
    let opened = answer(&tool(&mut host, r#"{"action":"open","target":2}"#).expect("opened"));
    assert_eq!(opened["breakpoint"], "desktop");
}

#[test]
fn open_without_a_run_or_report_fails_with_the_reason() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375)]);
    let failure = tool(&mut host, r#"{"action":"open"}"#).expect_err("no runs");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_reports"));
    let mut host = two_runs();
    let failure =
        tool(&mut host, r#"{"action":"open","breakpoint":"tablet"}"#).expect_err("no tablet");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_report"));
    let failure =
        tool(&mut host, r#"{"action":"open","run":"20991231-000000"}"#).expect_err("no such run");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_run"));
}

#[test]
fn arguments_outside_the_schema_are_refused() {
    let mut host = two_runs();
    for bad in [
        r#"{"action":"delete","run":"20261101-090000"}"#,
        r#"{"action":"open","run":"../x"}"#,
        r#"{"action":"open","path":"x.html"}"#,
        r#"{"action":"list","breakpoint":""}"#,
        "not json",
    ] {
        let failure = tool(&mut host, bad).expect_err(bad);
        assert_eq!(
            failure.code(),
            Some("plugin.lighthouse.errors.bad_args"),
            "{bad}"
        );
    }
}

#[test]
fn list_is_the_default_action() {
    let mut host = two_runs();
    let listed = answer(&tool(&mut host, "").expect("listed"));
    assert_eq!(listed["runs"][0]["runId"], NEW);
    assert_eq!(listed["runs"][1]["runId"], OLD);
    assert!(listed.get("$report").is_none());
}

#[test]
fn compare_answers_two_runs() {
    let mut host = two_runs();
    let compared = answer(
        &tool(&mut host, r#"{"action":"compare","breakpoint":"desktop"}"#).expect("compared"),
    );
    assert_eq!(compared["before"], OLD);
    assert_eq!(compared["after"], NEW);
}

#[test]
fn delete_asks_the_host_to_remove_the_run() {
    let mut host = two_runs();
    let deleted =
        answer(&delete_command(&mut host, &json!({ "run": OLD }).to_string()).expect("deleted"));
    assert_eq!(deleted["deleted"], OLD);
    assert_eq!(deleted["$report"], json!({ "remove": [OLD] }));
    // The newest run stays the latest.
    let latest = answer(&String::from_utf8_lossy(&host.written["latest.json"]));
    assert_eq!(latest["runId"], NEW);
}

#[test]
fn deleting_the_latest_run_makes_the_one_before_it_the_latest() {
    let mut host = two_runs();
    delete_command(&mut host, &json!({ "run": NEW }).to_string()).expect("deleted");
    let latest = answer(&String::from_utf8_lossy(&host.written["latest.json"]));
    assert_eq!(latest["runId"], OLD);
    let mut host = FakeHost::default().with_run(NEW, &[("mobile", 80)]);
    delete_command(&mut host, &json!({ "run": NEW }).to_string()).expect("deleted");
    assert_eq!(host.written["latest.json"], b"null".to_vec());
}

#[test]
fn delete_without_a_run_opens_the_panel_and_deletes_nothing() {
    let mut host = two_runs();
    let answered = answer(&delete_command(&mut host, "{}").expect("answered"));
    assert_eq!(answered, json!({ "$open": { "panel": "lighthouse" } }));
    let failure = delete_command(&mut host, r#"{"run":"20991231-000000"}"#).expect_err("unknown");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_run"));
    let failure = delete_command(&mut host, r#"{"run":".."}"#).expect_err("bad");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.bad_args"));
}

#[test]
fn the_runs_past_the_retention_go_with_the_run_answer() {
    let mut host = two_runs().set("keepRuns", "1");
    let mut summary = json!({ "runId": NEW });
    retain(&mut host, &mut summary);
    assert_eq!(summary["$report"], json!({ "remove": [OLD] }));
    let mut host = two_runs().set("keepRuns", "0");
    let mut summary = json!({ "runId": NEW });
    retain(&mut host, &mut summary);
    assert!(summary.get("$report").is_none(), "0 keeps every run");
    // Security review M1: without a number set, nothing is pruned.
    let mut host = two_runs();
    let mut summary = json!({ "runId": NEW });
    retain(&mut host, &mut summary);
    assert!(
        summary.get("$report").is_none(),
        "the default keeps every run"
    );
}
