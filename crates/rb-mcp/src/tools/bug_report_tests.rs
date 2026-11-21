use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{BugReportParams, paths_only};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

const FOLDER: &str = "C:/site/.rbrowser/out/reports/2026-11-20_14-03-07";

fn written() -> serde_json::Value {
    json!({
        "path": FOLDER,
        "markdown": format!("{FOLDER}/report.md"),
        "zip": format!("{FOLDER}/report.zip"),
    })
}

#[tokio::test]
async fn bug_report_runs_report_create_with_the_views_and_the_note() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "desktop", "http://127.0.0.1:18766/"),
        view(2, "mobile", "http://127.0.0.1:18766/"),
    ]));
    fake.answer("report.create", Ok(written()));
    let server = server(&fake);
    let (error, answer) = answer_json(
        &server
            .bug_report(Parameters(BugReportParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer, written());
    let call = BugReportParams {
        target: Some(TargetArg::Name("mobile".to_owned())),
        note: Some("the cart is empty".to_owned()),
    };
    server.bug_report(Parameters(call)).await.expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            ("report.create".to_owned(), json!({})),
            (
                "report.create".to_owned(),
                json!({ "target": [2], "note": "the cart is empty" })
            ),
        ]
    );
}

/// The answer is the report's paths: whatever else the command answers stays out.
#[tokio::test]
async fn the_answer_is_paths_never_content() {
    let fake = Arc::new(FakeBackend::default());
    let mut answer = written();
    answer["markdownText"] = json!("# Bug report ...");
    answer["console"] = json!(["Boom"]);
    fake.answer("report.create", Ok(answer));
    let (error, answer) = answer_json(
        &server(&fake)
            .bug_report(Parameters(BugReportParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer, written());
    assert!(
        answer["path"]
            .as_str()
            .is_some_and(|path| path.contains(".rbrowser/out/reports/"))
    );
    assert_eq!(
        paths_only(&json!({ "path": 3, "zip": "z" })),
        json!({ "zip": "z" })
    );
}

#[tokio::test]
async fn an_unknown_target_never_runs_the_command() {
    let fake = Arc::new(FakeBackend::default());
    let call = BugReportParams {
        target: Some(TargetArg::Id(42)),
        note: None,
    };
    let (error, _) = answer_json(
        &server(&fake)
            .bug_report(Parameters(call))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert!(fake.calls().is_empty());
}

#[test]
fn unknown_fields_are_refused() {
    assert!(serde_json::from_value::<BugReportParams>(json!({ "content": true })).is_err());
    let parsed: BugReportParams =
        serde_json::from_value(json!({ "target": "all", "note": "x" })).expect("parses");
    assert_eq!(parsed.note.as_deref(), Some("x"));
}
