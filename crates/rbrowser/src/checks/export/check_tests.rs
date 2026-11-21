use serde_json::json;

use super::*;

fn request(plugins: &[&str], out: Option<&str>) -> CheckRequest {
    CheckRequest {
        plugins: plugins.iter().map(|plugin| (*plugin).to_owned()).collect(),
        out: out.map(str::to_owned),
        ..CheckRequest::default()
    }
}

#[test]
fn a_plain_request_is_in_bounds() {
    assert_eq!(request(&[], None).check(), Ok(()));
    assert_eq!(request(&["seo", "a11y-axe"], None).check(), Ok(()));
}

#[cfg(not(windows))]
#[test]
fn the_out_file_is_a_plain_absolute_file() {
    assert_eq!(request(&[], Some("/tmp/report.sarif")).check(), Ok(()));
    for out in [
        "report.sarif",
        "./report.sarif",
        "/tmp/../etc/report.sarif",
        "/tmp/reports/",
        "",
        "/tmp/a\nb",
    ] {
        assert!(request(&[], Some(out)).check().is_err(), "{out:?}");
    }
}

#[test]
fn too_many_or_odd_plugin_ids_are_refused() {
    let many: Vec<String> = (0..=MAX_PLUGINS).map(|index| format!("p{index}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    assert!(request(&refs, None).check().is_err());
    assert!(request(&["../seo"], None).check().is_err());
    assert!(request(&["seo plugin"], None).check().is_err());
}

#[test]
fn every_plugin_runs_when_none_is_named() {
    let args = request(&[], None).run_args().expect("args");
    assert_eq!(args.plugins, None);
    let args = request(&["seo"], None).run_args().expect("args");
    assert_eq!(args.plugins, Some(vec!["seo".to_owned()]));
}

#[test]
fn the_request_is_camel_case_and_refuses_unknown_fields() {
    let parsed: CheckRequest = serde_json::from_value(json!({
        "plugins": ["seo"],
        "format": "junit",
        "out": "/tmp/x.xml",
        "failOn": "warning"
    }))
    .expect("a request");
    assert_eq!(parsed.format, Some(ExportFormat::Junit));
    assert_eq!(parsed.fail_on, Some(FindingsFailOn::Warning));
    assert!(serde_json::from_value::<CheckRequest>(json!({ "command": "rm" })).is_err());
    assert!(serde_json::from_value::<CheckRequest>(json!({ "format": "pdf" })).is_err());
}

#[test]
fn the_answer_round_trips() {
    let answer = CheckAnswer {
        path: "/site/.rbrowser/out/exports/run-1.sarif".to_owned(),
        run_id: "run-1".to_owned(),
        format: ExportFormat::Sarif,
        fail_on: FindingsFailOn::Error,
        summary: FindingsSummary::default(),
        failed_plugins: vec!["seo".to_owned()],
        cancelled: false,
    };
    let value = serde_json::to_value(&answer).expect("serializes");
    assert_eq!(value["failOn"], "error");
    assert_eq!(value["failedPlugins"], json!(["seo"]));
    let back: CheckAnswer = serde_json::from_value(value).expect("parses");
    assert_eq!(back, answer);
}
