use serde_json::{Value, json};

use super::*;
use crate::fake::FakeHost;

const OLD: &str = "20261101-090000";
const MID: &str = "20261105-120000";
const NEW: &str = "20261112-100000";

fn three_runs() -> FakeHost {
    FakeHost::with_views(&[(1, "mobile", 375), (2, "desktop", 1280)])
        .with_run(OLD, &[("mobile", 40), ("desktop", 70)])
        .with_run(MID, &[("mobile", 55)])
        .with_run(NEW, &[("mobile", 80), ("desktop", 90)])
}

#[test]
fn a_run_id_is_the_start_time() {
    assert!(is_run_id(NEW));
    for bad in [
        "",
        "latest.json",
        "20261112-10000",
        "20261112_100000",
        "../20261112-100000",
        "2026111a-100000",
        "20261112-1000000",
    ] {
        assert!(!is_run_id(bad), "{bad}");
    }
}

#[test]
fn the_runs_are_listed_newest_first_with_their_breakpoints() {
    let mut host = three_runs();
    let runs = list(&mut host);
    let ids: Vec<&str> = runs.iter().map(|run| run.id.as_str()).collect();
    assert_eq!(ids, vec![NEW, MID, OLD]);
    assert_eq!(runs[0].breakpoints.len(), 2);
    assert_eq!(runs[0].breakpoints[0].name, "mobile");
    assert_eq!(runs[0].breakpoints[0].report, format!("{NEW}/mobile.html"));
    assert_eq!(runs[0].breakpoints[0].scores["performance"], json!(80));
}

#[test]
fn a_folder_without_a_readable_summary_or_with_another_name_is_left_out() {
    let mut host = three_runs();
    host.written
        .insert("20261102-080000/mobile.html".to_owned(), b"x".to_vec());
    host.written
        .insert("20261103-080000/run.json".to_owned(), b"not json".to_vec());
    host.written
        .insert("notes/run.json".to_owned(), b"{}".to_vec());
    let ids: Vec<String> = list(&mut host).into_iter().map(|run| run.id).collect();
    assert_eq!(ids, vec![NEW, MID, OLD]);
}

#[test]
fn a_report_path_outside_its_run_is_not_offered() {
    let mut host = FakeHost::default();
    let summary = json!({ "runId": NEW, "breakpoints": [
        { "breakpoint": "a", "report": ".rbrowser/out/lighthouse/20261101-090000/a.html" },
        { "breakpoint": "b", "report": "../../secret.html" },
        { "breakpoint": "c", "report": format!(".rbrowser/out/lighthouse/{NEW}/c.html") },
    ] });
    host.written
        .insert(format!("{NEW}/run.json"), summary.to_string().into_bytes());
    let runs = list(&mut host);
    let names: Vec<&str> = runs[0]
        .breakpoints
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, vec!["c"]);
}

#[test]
fn the_runs_past_the_retention_are_the_oldest() {
    let mut host = three_runs();
    assert_eq!(beyond(&mut host, 2), vec![OLD.to_owned()]);
    assert_eq!(beyond(&mut host, 1), vec![MID.to_owned(), OLD.to_owned()]);
    assert!(beyond(&mut host, 3).is_empty());
    assert!(beyond(&mut host, 0).is_empty(), "0 keeps every run");
}

#[test]
fn the_list_answer_names_every_run() {
    let mut host = three_runs();
    let answer = list_answer(&mut host);
    let runs = answer["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0]["runId"], NEW);
    assert_eq!(runs[0]["breakpoints"][1]["breakpoint"], "desktop");
    assert_eq!(
        runs[0]["breakpoints"][1]["report"],
        format!("{NEW}/desktop.html")
    );
}

#[test]
fn two_runs_of_a_breakpoint_compare_by_scores_and_metrics() {
    let mut host = three_runs();
    let answer = compare(
        &mut host,
        Some("mobile"),
        Some(&[NEW.to_owned(), OLD.to_owned()]),
    )
    .expect("compared");
    assert_eq!(answer["breakpoint"], "mobile");
    assert_eq!(answer["before"], OLD, "the older run comes first");
    assert_eq!(answer["after"], NEW);
    let performance = row(&answer["scores"], "category", "performance");
    assert_eq!(performance["before"], json!(40));
    assert_eq!(performance["after"], json!(80));
    assert_eq!(performance["delta"], json!(40));
    let lcp = row(&answer["metrics"], "metric", "largest-contentful-paint");
    assert_eq!(lcp["before"], json!(400.0));
    assert_eq!(lcp["after"], json!(800.0));
    assert_eq!(lcp["delta"], json!(400.0));
    assert_eq!(lcp["unit"], "ms");
    let fcp = row(&answer["metrics"], "metric", "first-contentful-paint");
    assert_eq!(fcp["before"], Value::Null);
    assert_eq!(fcp["delta"], Value::Null);
}

#[test]
fn without_runs_the_two_newest_with_the_breakpoint_compare() {
    let mut host = three_runs();
    let answer = compare(&mut host, Some("desktop"), None).expect("compared");
    assert_eq!(answer["before"], OLD, "the middle run has no desktop");
    assert_eq!(answer["after"], NEW);
    // The active view's breakpoint without one named.
    let answer = compare(&mut host, None, None).expect("compared");
    assert_eq!(answer["breakpoint"], "mobile");
    assert_eq!(answer["before"], MID);
}

#[test]
fn a_comparison_needs_two_runs_with_the_breakpoint() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375)]).with_run(NEW, &[("mobile", 80)]);
    let failure = compare(&mut host, Some("mobile"), None).expect_err("one run");
    assert_eq!(
        failure.code(),
        Some("plugin.lighthouse.errors.no_comparison")
    );
    let mut host = three_runs();
    let failure = compare(&mut host, Some("tablet"), None).expect_err("no tablet");
    assert_eq!(
        failure.code(),
        Some("plugin.lighthouse.errors.no_comparison")
    );
    let failure = compare(
        &mut host,
        Some("mobile"),
        Some(&[NEW.to_owned(), "x".to_owned()]),
    )
    .expect_err("a bad id");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.bad_args"));
}

#[test]
fn metrics_are_read_from_the_lhr_of_a_run_that_has_none_in_its_summary() {
    let lhr = json!({ "audits": {
        "largest-contentful-paint": { "numericValue": 1234.5 },
        "cumulative-layout-shift": { "numericValue": 0.02 },
        "speed-index": { "displayValue": "1 s" },
    } });
    let found = metrics_of_lhr(&lhr);
    assert_eq!(found["largest-contentful-paint"], json!(1234.5));
    assert_eq!(found["cumulative-layout-shift"], json!(0.02));
    assert_eq!(found.get("speed-index"), None);
    assert_eq!(found.get("first-contentful-paint"), None);
}

fn row<'a>(rows: &'a Value, key: &str, name: &str) -> &'a Value {
    rows.as_array()
        .and_then(|rows| rows.iter().find(|row| row[key] == name))
        .unwrap_or_else(|| panic!("a row {name}"))
}
