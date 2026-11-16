use std::time::Duration;

use clap::Parser;

use super::*;
use crate::cli::Command;
use crate::flows::result::{ErrorInfo, RESULT_VERSION, ViewResult, write};

fn view(id: u32, label: &str, status: ViewStatus) -> ViewResult {
    ViewResult {
        view_id: id,
        label: label.to_owned(),
        breakpoint: Some(label.to_lowercase()),
        engine: "chromium".to_owned(),
        status,
        duration_ms: 42,
        steps: Vec::new(),
        error: None,
        screenshot: None,
        reason: None,
    }
}

fn result(views: Vec<ViewResult>) -> RunResult {
    RunResult {
        version: RESULT_VERSION,
        run_id: "20261111-101500-a1b2c3".to_owned(),
        flow: "login".to_owned(),
        file: ".rbrowser/flows/login.ts".to_owned(),
        started_at: "2026-11-11T10:15:00Z".to_owned(),
        duration_ms: 1200,
        passed: !RunResult::failed(&views),
        views,
        error: None,
    }
}

#[test]
fn flow_run_takes_a_name_views_and_json() {
    let cli = Cli::try_parse_from([
        "rbrowser",
        "flow",
        "run",
        "login",
        "--views",
        "mobile,desktop",
        "--json",
    ])
    .expect("parses");
    match cli.command {
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::Run { name, views, json },
        })) => {
            assert_eq!(name, "login");
            assert_eq!(views, ["mobile", "desktop"]);
            assert!(json);
        }
        other => panic!("{other:?}"),
    }
    let list =
        Cli::try_parse_from(["rbrowser", "--project", "C:\\site", "flow", "list"]).expect("parses");
    assert!(matches!(
        list.command,
        Some(Command::Flow(FlowArgs {
            command: FlowCommand::List { json: false }
        }))
    ));
    assert!(Cli::try_parse_from(["rbrowser", "flow", "run"]).is_err());
}

#[test]
fn a_failed_view_makes_exit_code_one() {
    let passed = result(vec![
        view(1, "Mobile", ViewStatus::Passed),
        view(2, "Firefox", ViewStatus::Skipped),
    ]);
    assert_eq!(exit_code(&passed), exit::OK);
    let timed = result(vec![view(1, "Mobile", ViewStatus::TimedOut)]);
    assert_eq!(exit_code(&timed), exit::FAILED);
}

#[test]
fn the_text_names_each_view_its_error_line_step_and_screenshot() {
    let mut failed = view(2, "Tablet", ViewStatus::Failed);
    failed.error = Some(ErrorInfo {
        message: "expect(received).toBe(expected)\n\nExpected: 2".to_owned(),
        stack: String::new(),
        step: Some("check".to_owned()),
        line: Some(3),
        column: Some(5),
        file: "/home/u/site/.rbrowser/flows/login.ts".to_owned(),
    });
    failed.screenshot = Some("tablet-2.png".to_owned());
    let mut skipped = view(3, "Desktop", ViewStatus::Skipped);
    skipped.engine = "firefox".to_owned();
    skipped.reason = Some("Firefox views are not driven by flows".to_owned());
    let text = text(&result(vec![
        view(1, "Mobile", ViewStatus::Passed),
        failed,
        skipped,
    ]));
    assert!(text.starts_with("login: 1 passed, 1 failed, 1 skipped (1200 ms)"));
    assert!(text.contains("  FAILED    Tablet [chromium] 42 ms\n"));
    assert!(text.contains(
        "expect(received).toBe(expected) in step \"check\" (/home/u/site/.rbrowser/flows/login.ts:3)"
    ));
    assert!(text.contains("screenshot: .rbrowser/out/flows/20261111-101500-a1b2c3/tablet-2.png"));
    assert!(text.contains("Firefox views are not driven by flows"));
}

#[test]
fn the_cli_waits_for_the_result_and_gives_up_after_its_budget() {
    let dir = tempfile::tempdir().expect("a project");
    let id = "20261111-101500-a1b2c3";
    assert_eq!(wait_for(dir.path(), id, Duration::from_millis(10)), None);
    let written = result(vec![view(1, "Mobile", ViewStatus::Passed)]);
    write(&run_dir(dir.path(), id), &written).expect("writes");
    assert_eq!(
        wait_for(dir.path(), id, Duration::from_millis(10)),
        Some(written)
    );
}

#[test]
fn a_run_that_did_not_start_says_why() {
    let mut broken = result(Vec::new());
    broken.passed = false;
    broken.error = Some("no flow \"cart\"".to_owned());
    let shown = text(&broken);
    assert!(shown.contains("did not run: no flow \"cart\""), "{shown}");
    assert_eq!(exit_code(&broken), exit::FAILED);
}
