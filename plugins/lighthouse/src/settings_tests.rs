use super::*;
use crate::fake::FakeHost;

#[test]
fn throttling_is_simulated_unless_set() {
    assert_eq!(throttling(&mut FakeHost::default()), "simulate");
    let mut host = FakeHost::default().set("throttling", "\"devtools\"");
    assert_eq!(throttling(&mut host), "devtools");
}

#[test]
fn a_value_outside_the_schema_is_the_default() {
    let mut host = FakeHost::default().set("throttling", "\"fast\"");
    assert_eq!(throttling(&mut host), "simulate");
    let mut host = FakeHost::default().set("throttling", "3");
    assert_eq!(throttling(&mut host), "simulate");
}

#[test]
fn a_report_opens_in_the_one_window_unless_set() {
    assert_eq!(report_window(&mut FakeHost::default()), "single");
    let mut host = FakeHost::default().set("reportWindow", "\"perReport\"");
    assert_eq!(report_window(&mut host), "perReport");
    let mut host = FakeHost::default().set("reportWindow", "\"grid\"");
    assert_eq!(report_window(&mut host), "single");
}

#[test]
fn every_run_is_kept_unless_a_number_is_set() {
    // Security review M1: an update never prunes an existing history on its own.
    assert_eq!(keep_runs(&mut FakeHost::default()), 0);
    let mut host = FakeHost::default().set("keepRuns", "5");
    assert_eq!(keep_runs(&mut host), 5);
    for bad in ["-1", "501", "\"5\"", "2.5"] {
        let mut host = FakeHost::default().set("keepRuns", bad);
        assert_eq!(keep_runs(&mut host), 0, "{bad}");
    }
}

#[test]
fn the_retention_is_protected_and_keeps_every_run_by_default() {
    // Security review M1: MCP cannot set a retention that deletes the history.
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("the schema");
    let keep = &schema["properties"]["keepRuns"];
    assert_eq!(keep["x-protected"], true);
    assert_eq!(keep["default"], serde_json::json!(KEEP_RUNS));
    assert_eq!(KEEP_RUNS, 0);
}
