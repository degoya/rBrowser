use std::path::Path;

use clap::Parser;
use rb_plugin_types::FindingsSummary;
use serde_json::json;

use super::*;
use crate::checks::export::ExportFormat;

#[derive(Debug, Parser)]
struct Probe {
    #[command(flatten)]
    args: CheckCliArgs,
}

fn parse(line: &[&str]) -> Result<CheckCliArgs, clap::Error> {
    let mut words = vec!["rbrowser"];
    words.extend_from_slice(line);
    Probe::try_parse_from(words).map(|probe| probe.args)
}

fn env(cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        windows: false,
        distro: None,
        recent: Vec::new(),
        open_last_project: true,
        focus_on_open: true,
    }
}

fn answer(summary: FindingsSummary, fail_on: FindingsFailOn) -> CheckAnswer {
    CheckAnswer {
        path: "/site/.rbrowser/out/exports/run-1.sarif".to_owned(),
        run_id: "run-1".to_owned(),
        format: ExportFormat::Sarif,
        fail_on,
        summary,
        failed_plugins: Vec::new(),
        cancelled: false,
    }
}

fn summary(error: usize, warning: usize, info: usize) -> FindingsSummary {
    FindingsSummary {
        total: error + warning + info,
        error,
        warning,
        info,
        by_plugin: Default::default(),
    }
}

fn answered(answer: &CheckAnswer) -> Outcome {
    Outcome::Answered(serde_json::to_value(answer).expect("serializes"))
}

#[test]
fn the_flags_are_parsed_and_checked() {
    let args = parse(&[
        "--plugins",
        "seo, a11y-axe",
        "--format",
        "junit",
        "--fail-on",
        "warning",
        "--out",
        "reports/checks.xml",
    ])
    .expect("valid");
    assert_eq!(args.plugins, ["seo", " a11y-axe"]);
    assert!(parse(&["--format", "pdf"]).is_err());
    assert!(parse(&["--fail-on", "never"]).is_err());
    // An absolute folder on this platform (`/work` is none on Windows).
    let work = if cfg!(windows) { r"C:\work" } else { "/work" };
    let Request::Check { args: request } =
        instance_request(&args, &env(Path::new(work))).expect("a request")
    else {
        panic!("not a check request");
    };
    assert_eq!(request.plugins, ["seo", "a11y-axe"]);
    assert_eq!(request.format, Some(ExportFormat::Junit));
    assert_eq!(request.fail_on, Some(FindingsFailOn::Warning));
    let expected = if cfg!(windows) {
        r"C:\work\reports\checks.xml"
    } else {
        "/work/reports/checks.xml"
    };
    assert_eq!(request.out.as_deref(), Some(expected));
}

#[test]
fn without_flags_the_settings_decide() {
    let Request::Check { args: request } =
        instance_request(&CheckCliArgs::default(), &env(Path::new("/work"))).expect("a request")
    else {
        panic!("not a check request");
    };
    assert_eq!(request, CheckRequest::default());
}

#[test]
fn an_odd_plugin_or_out_is_refused_before_anything_is_sent() {
    let bad = CheckCliArgs {
        plugins: vec!["../seo".to_owned()],
        ..CheckCliArgs::default()
    };
    assert!(instance_request(&bad, &env(Path::new("/work"))).is_err());
    let folder = CheckCliArgs {
        out: Some("/work/reports/".into()),
        ..CheckCliArgs::default()
    };
    assert!(instance_request(&folder, &env(Path::new("/work"))).is_err());
}

#[test]
fn an_error_finding_fails_with_exit_1() {
    let ending = ending(
        answered(&answer(summary(1, 2, 0), FindingsFailOn::Error)),
        Path::new("/site"),
    );
    assert_eq!(ending.code, code::FINDINGS);
    assert_eq!(
        ending.stdout.as_deref(),
        Some("/site/.rbrowser/out/exports/run-1.sarif")
    );
    assert!(
        ending
            .stderr
            .expect("counts")
            .starts_with("3 findings: 1 errors")
    );
}

#[test]
fn only_warnings_pass_at_error_and_fail_at_warning() {
    let at_error = ending(
        answered(&answer(summary(0, 2, 1), FindingsFailOn::Error)),
        Path::new("/site"),
    );
    assert_eq!(at_error.code, code::CLEAN);
    let at_warning = ending(
        answered(&answer(summary(0, 2, 1), FindingsFailOn::Warning)),
        Path::new("/site"),
    );
    assert_eq!(at_warning.code, code::FINDINGS);
    let info_only = ending(
        answered(&answer(summary(0, 0, 1), FindingsFailOn::Info)),
        Path::new("/site"),
    );
    assert_eq!(info_only.code, code::FINDINGS);
    let clean = ending(
        answered(&answer(summary(0, 0, 0), FindingsFailOn::Info)),
        Path::new("/site"),
    );
    assert_eq!(clean.code, code::CLEAN);
}

#[test]
fn a_failed_plugin_or_a_cancelled_run_is_exit_2() {
    let mut crashed = answer(summary(0, 0, 0), FindingsFailOn::Error);
    crashed.failed_plugins = vec!["seo".to_owned()];
    let ending_of_crash = ending(answered(&crashed), Path::new("/site"));
    assert_eq!(ending_of_crash.code, code::ERROR);
    assert!(ending_of_crash.stderr.expect("why").contains("seo"));
    assert!(ending_of_crash.stdout.is_some(), "the file is still named");
    let mut cancelled = answer(summary(1, 0, 0), FindingsFailOn::Error);
    cancelled.cancelled = true;
    assert_eq!(
        ending(answered(&cancelled), Path::new("/site")).code,
        code::ERROR
    );
}

#[test]
fn without_a_running_instance_it_is_exit_2_with_how_to_start_one() {
    let ending = ending(Outcome::NotRunning, Path::new("/site"));
    assert_eq!(ending.code, code::ERROR);
    assert_eq!(ending.stdout, None);
    let message = ending.stderr.expect("a message");
    assert!(message.contains("no rBrowser runs for /site"), "{message}");
    assert!(
        message.contains("rbrowser --project \"/site\""),
        "{message}"
    );
}

#[test]
fn a_refusal_a_failure_or_an_odd_answer_is_exit_2() {
    for outcome in [
        Outcome::Handed,
        Outcome::Refused(Refusal::Failed, Some("checks.error.busy".to_owned())),
        Outcome::Refused(Refusal::BadNonce, None),
        Outcome::Failed("timed out".to_owned()),
        Outcome::Answered(json!({ "shots": [] })),
    ] {
        assert_eq!(ending(outcome, Path::new("/site")).code, code::ERROR);
    }
}

#[test]
fn the_project_comes_from_the_command_line_or_the_working_folder() {
    let cli = Cli::try_parse_from(["rbrowser", "--url", "http://localhost:5173", "check"])
        .expect("parses");
    assert!(project_root(&cli, &env(Path::new("/nowhere"))).is_err());
    let dir = tempfile::tempdir().expect("temp dir");
    let cli = Cli::try_parse_from(["rbrowser", "check"]).expect("parses");
    let message = project_root(&cli, &env(dir.path())).expect_err("no project");
    assert!(message.contains("--project"), "{message}");
}
