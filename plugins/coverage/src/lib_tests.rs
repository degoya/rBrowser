use rb_plugin_api::CheckContext;
use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;
use serde_json::Value;

use super::Coverage;
use crate::fake::{FakeHost, Sheet};

fn host() -> FakeHost {
    let sheet = |used: Vec<(u32, u32)>| Sheet {
        id: "s1",
        href: Some("http://localhost:8080/coverage/site.css"),
        text: "body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n"
            .to_owned(),
        used,
    };
    FakeHost::new()
        .view(1, "desktop", 1280, vec![sheet(vec![(0, 18), (48, 71)])])
        .view(2, "mobile", 375, vec![sheet(vec![(0, 18)])])
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

#[test]
fn run_load_reports_its_findings_and_opens_the_panel() {
    let mut host = host();
    let answer = Coverage
        .run_command(&mut host, "run_load", "")
        .expect("ran");
    let answer = json(&answer);
    assert_eq!(answer["$open"]["panel"], "coverage");
    assert_eq!(answer["$post"][0]["target"], "panel");
    assert_eq!(answer["findings"], 1);
    assert_eq!(host.reported.len(), 1);
    assert_eq!(host.reported[0].rule, "rule-partial");
}

#[test]
fn the_check_run_reports_the_last_measurement_for_its_view() {
    let mut host = host();
    let ctx = |view| CheckContext {
        view,
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    assert!(
        Coverage
            .run(&mut host, &ctx(Some(1)))
            .expect("no run")
            .is_empty()
    );
    Coverage
        .call_tool(&mut host, "get", r#"{"mode":"load"}"#)
        .expect("measured");
    let wide = Coverage.run(&mut host, &ctx(Some(1))).expect("findings");
    assert_eq!(wide.len(), 1);
    assert_eq!(wide[0].view, Some(1));
    assert!(
        Coverage
            .run(&mut host, &ctx(Some(2)))
            .expect("none")
            .is_empty()
    );
    // A view that shows another page since gets nothing.
    host.views[0].info.url = "http://localhost:8080/other.html".to_owned();
    assert!(
        Coverage
            .run(&mut host, &ctx(Some(1)))
            .expect("stale")
            .is_empty()
    );
    assert_eq!(Coverage.describe().len(), 3);
}

#[test]
fn a_webkit_view_named_alone_is_skipped_for_its_engine_once_something_was_measured() {
    let mut host = host();
    host.views[1].info.engine = "webkit".to_owned();
    let ctx = CheckContext {
        view: Some(2),
        url: None,
        rules: Vec::new(),
        args: "{}".to_owned(),
    };
    // Nothing measured yet: nothing to report, before the views are even asked.
    assert!(
        Coverage
            .run(&mut host, &ctx)
            .expect("nothing measured")
            .is_empty()
    );
    Coverage
        .call_tool(&mut host, "get", r#"{"mode":"load"}"#)
        .expect("measured");
    let failure = Coverage.run(&mut host, &ctx).expect_err("skipped");
    assert_eq!(failure.unserved_engine_in(2), Some("webkit"));
}

#[test]
fn start_stop_and_export_are_commands_too() {
    let mut host = host();
    let started = json(
        &Coverage
            .run_command(&mut host, "start", "{}")
            .expect("started"),
    );
    assert_eq!(started["views"], 2);
    let stopped = json(
        &Coverage
            .run_command(&mut host, "stop", "{}")
            .expect("stopped"),
    );
    assert_eq!(stopped["$open"]["panel"], "coverage");
    assert!(!host.tracking());
    let exported = json(
        &Coverage
            .run_command(&mut host, "export", "{}")
            .expect("exported"),
    );
    assert_eq!(
        exported["$post"][0]["message"]["exported"][1],
        ".rbrowser/out/coverage/coverage.csv"
    );
    let unknown = Coverage
        .run_command(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(unknown.code(), Some("plugins.error.unknown_contribution"));
    assert!(Coverage.call_tool(&mut host, "nope", "{}").is_err());
}
