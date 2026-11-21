use super::*;
use crate::load_test::abort::Abort;
use crate::load_test::engine::End;
use crate::load_test::stats::{Failed, Hit, Recorder};

fn meta(started_at: u64) -> Meta {
    Meta {
        scenario: "check out.json".into(),
        name: "Check out".into(),
        started_at,
        profile: Profile {
            users: 2,
            ramp_up_s: 0,
            hold_s: 2,
            ramp_down_s: 0,
            rps: None,
        },
        rps_limit: 200.0,
        environment: Some("staging".into()),
        targets: vec![("localhost".into(), Standing::Local)],
        steps: vec!["shop/list.http".into()],
    }
}

fn finished(end: End, ms: &[f64]) -> Finished {
    let mut recorder = Recorder::new(1, 10);
    for value in ms {
        recorder.record(&Hit {
            step: 0,
            ms: *value,
            status: Some(200),
            failed: None,
        });
    }
    recorder.record(&Hit {
        step: 0,
        ms: 1.0,
        status: Some(500),
        failed: Some(Failed::Http5xx),
    });
    recorder.tick(1, 2);
    Finished {
        end,
        elapsed_s: 2.0,
        recorder,
    }
}

#[test]
fn a_time_reads_as_iso() {
    assert_eq!(iso(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso(1_763_733_901), "2025-11-21T14:05:01Z");
}

#[test]
fn the_report_holds_the_runs_numbers() {
    let report = build(
        &meta(1_763_733_901),
        &finished(End::Completed, &[10.0, 20.0, 30.0]),
    );
    assert_eq!(report["id"], "1763733901-check_out");
    assert_eq!(report["end"], "completed");
    assert_eq!(report["abort"], Value::Null);
    assert_eq!(report["totals"]["requests"], 4);
    assert_eq!(report["totals"]["errors"], 1);
    assert_eq!(report["errors"]["http5xx"], 1);
    assert_eq!(report["statuses"]["200"], 3);
    assert_eq!(report["steps"][0]["request"], "shop/list.http");
    assert_eq!(report["series"].as_array().map(Vec::len), Some(1));
    assert_eq!(report["targets"][0]["standing"], "local");
}

#[test]
fn an_aborted_run_names_its_rule() {
    let abort = Abort {
        rule: "p95Ms",
        value: 2500.0,
        threshold: 2000.0,
    };
    let report = build(&meta(1), &finished(End::Aborted(abort), &[1.0]));
    assert_eq!(report["end"], "aborted");
    assert_eq!(report["abort"]["rule"], "p95Ms");
}

#[test]
fn reports_are_kept_listed_newest_first_and_read() {
    let dir = tempfile::tempdir().expect("temp");
    let reports = Reports::new(dir.path());
    assert!(reports.list().expect("none").is_empty());
    assert_eq!(
        reports.read(None).expect_err("none").code(),
        Some("plugin.load-test.errors.no_reports")
    );
    let older = build(&meta(1_000_000_000), &finished(End::Completed, &[10.0]));
    let newer = build(&meta(1_000_000_100), &finished(End::Stopped, &[20.0]));
    reports.save(&older, "<html></html>").expect("saved");
    let id = reports.save(&newer, "<html></html>").expect("saved");
    assert!(
        dir.path()
            .join(OUT_DIR)
            .join(format!("{id}.html"))
            .is_file()
    );
    let rows = reports.list().expect("listed");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], newer["id"]);
    assert_eq!(rows[0]["end"], "stopped");
    assert_eq!(reports.read(None).expect("newest")["id"], newer["id"]);
    assert_eq!(
        reports.read(Some("../x")).expect_err("bad").code(),
        Some("plugin.load-test.errors.bad_id")
    );
    assert_eq!(
        reports.read(Some("123-none")).expect_err("none").code(),
        Some("plugin.load-test.errors.report_not_found")
    );
}

#[test]
fn two_runs_compare_side_by_side() {
    let a = build(&meta(1), &finished(End::Completed, &[10.0, 10.0, 10.0]));
    let b = build(&meta(2), &finished(End::Completed, &[30.0, 30.0, 30.0]));
    let compared = compare(&a, &b);
    assert_eq!(compared["a"]["id"], a["id"]);
    assert_eq!(compared["b"]["id"], b["id"]);
    assert_eq!(compared["delta"]["requests"], 0.0);
    let p50 = compared["delta"]["p50"].as_f64().expect("a number");
    assert!(p50 > 15.0, "{p50}");
}
