use rb_plugin_api::{CheckContext, Failure, FailureKind, codes};
use serde_json::{Value, json};

use super::*;
use crate::fake::{FakeHost, NOW, Outcome};

fn two_breakpoints() -> FakeHost {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375), (2, "desktop", 1440)]);
    let mut firefox = host.views[1].clone();
    firefox.id = 3;
    firefox.breakpoint = "firefox".to_owned();
    firefox.engine = "firefox".to_owned();
    host.views.push(firefox);
    host.done().done()
}

fn written_json(host: &FakeHost, path: &str) -> Value {
    serde_json::from_slice(&host.written[path]).expect("written JSON")
}

#[test]
fn run_ids_are_utc_timestamps() {
    assert_eq!(run_id(NOW), "20261112-100000");
    assert_eq!(run_id(0), "19700101-000000");
    assert_eq!(run_id(951_782_400 + 3_661), "20000229-010101");
}

#[test]
fn measures_each_breakpoint_saves_both_reports_and_a_summary() {
    let mut host = two_breakpoints();
    let summary = run(&mut host, &Request::default()).expect("runs");
    let id = "20261112-100000";
    for name in ["mobile", "desktop"] {
        assert!(host.written.contains_key(&format!("{id}/{name}.html")));
        assert!(host.written.contains_key(&format!("{id}/{name}.json")));
    }
    assert_eq!(written_json(&host, &format!("{id}/run.json")), summary);
    assert_eq!(written_json(&host, LATEST), summary);
    let mobile = &summary["breakpoints"][0];
    assert_eq!(mobile["breakpoint"], "mobile");
    assert_eq!(mobile["width"], 375);
    assert_eq!(mobile["formFactor"], "mobile");
    assert_eq!(mobile["scores"]["accessibility"], 62);
    assert_eq!(
        mobile["report"],
        ".rbrowser/out/lighthouse/20261112-100000/mobile.html"
    );
    assert_eq!(summary["breakpoints"][1]["formFactor"], "desktop");
    assert_eq!(summary["skipped"][0]["breakpoint"], "firefox");
    assert_eq!(summary["skipped"][0]["reason"], "engine");
    assert_eq!(summary["categories"].as_array().map(Vec::len), Some(4));
    assert_eq!(summary["cancelled"], false);
    assert_eq!(host.progress, [(1, 2), (2, 2)]);
    // One at a time: the second start comes after the first job's release.
    let methods = host.methods();
    let second_start = methods
        .iter()
        .rposition(|method| *method == "lighthouse.start")
        .expect("two starts");
    let first_release = methods
        .iter()
        .position(|method| *method == "lighthouse.release")
        .expect("a release");
    assert!(first_release < second_start);
}

#[test]
fn each_job_gets_its_breakpoints_emulation_and_the_project_categories() {
    let config = r#"{ "checks": { "lighthouse": { "categories": ["performance"] } } }"#;
    let mut host = two_breakpoints()
        .with_file(".rbrowser/config.json", config)
        .set("throttling", "\"devtools\"");
    let summary = run(&mut host, &Request::default()).expect("runs");
    assert_eq!(summary["categories"], json!(["performance"]));
    let starts: Vec<&Value> = host
        .calls
        .iter()
        .filter(|(method, _)| method == "lighthouse.start")
        .map(|(_, params)| params)
        .collect();
    assert_eq!(starts[0]["settings"]["screenEmulation"]["width"], 375);
    assert_eq!(starts[0]["settings"]["formFactor"], "mobile");
    assert_eq!(starts[1]["settings"]["screenEmulation"]["width"], 1440);
    assert_eq!(
        starts[1]["settings"]["onlyCategories"],
        json!(["performance"])
    );
    assert_eq!(starts[1]["settings"]["throttlingMethod"], "devtools");
    assert_eq!(starts[1]["url"], "http://localhost:5173/");
}

#[test]
fn the_command_reports_the_findings_with_their_breakpoints_views() {
    let mut host = two_breakpoints();
    let request = Request {
        report: true,
        ..Request::default()
    };
    let summary = run(&mut host, &request).expect("runs");
    assert_eq!(
        summary["findings"].as_u64(),
        Some(host.reported.len() as u64)
    );
    let image_alt: Vec<_> = host
        .reported
        .iter()
        .filter(|finding| finding.rule == "image-alt")
        .collect();
    assert_eq!(
        image_alt.iter().map(|f| f.view).collect::<Vec<_>>(),
        [Some(1), Some(2)]
    );
    let mut quiet = two_breakpoints();
    run(&mut quiet, &Request::default()).expect("runs");
    assert!(quiet.reported.is_empty(), "the MCP tool only answers");
}

#[test]
fn a_breakpoint_that_fails_is_listed_and_the_run_goes_on() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375), (2, "desktop", 1440)])
        .then(Outcome::Failed {
            code: -32014,
            message: "net::ERR_CONNECTION_REFUSED".to_owned(),
        })
        .done();
    let summary = run(&mut host, &Request::default()).expect("runs");
    assert_eq!(summary["failures"][0]["breakpoint"], "mobile");
    assert_eq!(
        summary["failures"][0]["reason"],
        "net::ERR_CONNECTION_REFUSED"
    );
    assert_eq!(summary["breakpoints"][0]["breakpoint"], "desktop");
}

#[test]
fn a_cancelled_job_ends_the_run() {
    let mut host = FakeHost::with_views(&[(1, "mobile", 375), (2, "desktop", 1440)])
        .done()
        .then(Outcome::Cancelled);
    let summary = run(&mut host, &Request::default()).expect("runs");
    assert_eq!(summary["cancelled"], true);
    assert_eq!(summary["breakpoints"].as_array().map(Vec::len), Some(1));
}

#[test]
fn without_a_sidecar_the_run_fails_with_the_plugins_message() {
    let mut host = two_breakpoints();
    host.sidecar_failure = Some(Failure::coded(
        FailureKind::NotSupported,
        codes::NOT_AVAILABLE,
    ));
    let failure = run(&mut host, &Request::default()).expect_err("no sidecar");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_sidecar"));
    assert!(host.written.is_empty());
}

#[test]
fn only_firefox_views_is_no_run() {
    let mut host = FakeHost::with_views(&[(1, "firefox", 1440)]);
    host.views[0].engine = "firefox".to_owned();
    let failure = run(&mut host, &Request::default()).expect_err("nothing to run");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_targets"));
}

#[test]
fn the_summary_keeps_the_previous_runs_scores() {
    let mut host = two_breakpoints();
    run(&mut host, &Request::default()).expect("first run");
    host.now = NOW + 3600;
    host.outcomes = two_breakpoints().outcomes;
    let second = run(&mut host, &Request::default()).expect("second run");
    assert_eq!(second["runId"], "20261112-110000");
    assert_eq!(second["previous"]["runId"], "20261112-100000");
    assert_eq!(
        second["previous"]["breakpoints"][0]["scores"]["accessibility"],
        62
    );
}

#[test]
fn a_check_run_measures_the_contexts_view_with_the_rules_switched_on() {
    let mut host = two_breakpoints();
    let ctx = CheckContext {
        view: Some(2),
        url: None,
        rules: vec!["image-alt".to_owned(), "charset".to_owned()],
        args: String::new(),
    };
    let findings = check(&mut host, &ctx).expect("checks");
    let mut rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    rules.sort_unstable();
    assert_eq!(rules, ["charset", "image-alt"]);
    assert!(findings.iter().all(|f| f.view == Some(2)));
    assert!(host.written.is_empty(), "a check run saves nothing");
    let firefox = CheckContext {
        view: Some(3),
        ..ctx
    };
    // Skipped for its engine, not "nothing found" (RB-080-98).
    let failure = check(&mut host, &firefox).expect_err("skipped");
    assert_eq!(failure.unserved_engine_in(3), Some("firefox"));
}
