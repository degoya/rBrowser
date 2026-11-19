use super::*;
use crate::flows::result::{ErrorInfo, RESULT_VERSION, StepResult, ViewResult};

fn view(id: u32, status: ViewStatus, error: Option<(&str, Option<&str>)>) -> ViewResult {
    ViewResult {
        view_id: id,
        label: format!("View {id}"),
        breakpoint: Some(format!("bp{id}")),
        engine: "chromium".to_owned(),
        status,
        duration_ms: 10,
        steps: vec![StepResult {
            name: "Step 1: navigate".to_owned(),
            status: "passed".to_owned(),
            duration_ms: 5,
        }],
        error: error.map(|(message, step)| ErrorInfo {
            message: message.to_owned(),
            stack: "at login.ts:3:1".to_owned(),
            step: step.map(str::to_owned),
            step_id: None,
            summary: None,
            line: Some(3),
            column: Some(1),
            file: "login.ts".to_owned(),
        }),
        screenshot: None,
        reason: None,
    }
}

fn result(views: Vec<ViewResult>) -> RunResult {
    RunResult {
        version: RESULT_VERSION,
        run_id: "20261118-185842-a1b2c3".to_owned(),
        flow: "recording-20261118-195756".to_owned(),
        file: ".rbrowser/out/recordings/recording-20261118-195756.json".to_owned(),
        started_at: "2026-11-18T18:58:42Z".to_owned(),
        duration_ms: 31_000,
        passed: !RunResult::failed(&views),
        views,
        skipped_breakpoints: Vec::new(),
        error: None,
    }
}

#[test]
fn a_failed_view_names_its_step_its_reason_and_whether_it_was_hidden() {
    let run = result(vec![
        view(1, ViewStatus::Passed, None),
        view(
            2,
            ViewStatus::Failed,
            Some((
                "locator.hover: Timeout 10000ms exceeded.\nCall log:\n  - waiting for getByRole('link')",
                Some("Step 3: hover"),
            )),
        ),
        view(
            3,
            ViewStatus::TimedOut,
            Some(("the flow ran longer than 30000 ms", None)),
        ),
        view(4, ViewStatus::Skipped, None),
    ]);
    let lines = failures(&run, |view| Shown {
        off_screen: view == 2,
        reported_hidden: view == 2,
    });
    assert_eq!(
        lines,
        [
            Failure {
                view_id: 2,
                label: "View 2".to_owned(),
                breakpoint: Some("bp2".to_owned()),
                status: ViewStatus::Failed,
                step: Some("Step 3: hover".to_owned()),
                reason: "locator.hover: Timeout 10000ms exceeded.".to_owned(),
                shown: Shown {
                    off_screen: true,
                    reported_hidden: true,
                },
            },
            Failure {
                view_id: 3,
                label: "View 3".to_owned(),
                breakpoint: Some("bp3".to_owned()),
                status: ViewStatus::TimedOut,
                step: None,
                reason: "the flow ran longer than 30000 ms".to_owned(),
                shown: Shown::default(),
            },
        ]
    );
    assert!(
        failures(&result(vec![view(1, ViewStatus::Passed, None)]), |_| {
            Shown::default()
        })
        .is_empty()
    );
}

#[test]
fn a_failed_view_without_an_error_still_has_a_line() {
    let run = result(vec![view(5, ViewStatus::Failed, None)]);
    let lines = failures(&run, |_| Shown::default());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].reason, "no reason given");
}

#[test]
fn the_reason_is_the_first_line_without_colours_queries_or_secrets() {
    // Only the first line: the lines after an `expect` failure hold the expected and the
    // received value - a form field's text.
    assert_eq!(
        reason(
            "\n  expect(locator).toHaveValue(expected) failed\n\nExpected: \"alice@example.org\""
        ),
        "expect(locator).toHaveValue(expected) failed"
    );
    assert_eq!(
        reason("\u{1b}[31mTimeout\u{1b}[39m 5000ms exceeded"),
        "Timeout 5000ms exceeded"
    );
    // A URL keeps its path, never its query or fragment.
    assert_eq!(
        reason(
            "page.goto: net::ERR_ABORTED at https://shop.test/cart?coupon=X1&mail=a@b.test#pay; then"
        ),
        "page.goto: net::ERR_ABORTED at https://shop.test/cart; then"
    );
    assert_eq!(
        reason("expected 'http://a.test/x?q=1' got 'http://a.test/y'"),
        "expected 'http://a.test/x' got 'http://a.test/y'"
    );
    assert_eq!(
        reason("Authorization: Bearer abcdef123456"),
        "Authorization: ***"
    );
    let long = "x".repeat(MAX_REASON + 50);
    assert_eq!(reason(&long).chars().count(), MAX_REASON + 1);
    assert!(reason(&long).ends_with('\u{2026}'));
    assert_eq!(reason("  \n \n"), "no reason given");
}

#[test]
fn a_run_that_skipped_what_the_layout_hid_says_so_with_the_setting() {
    // RB-090-84, `automation.flows.hiddenViews: skip`.
    let mut hidden = view(2, ViewStatus::Skipped, None);
    hidden.reason = Some(HIDDEN_REASON.to_owned());
    let mut run = result(vec![view(1, ViewStatus::Passed, None), hidden]);
    run.skipped_breakpoints = vec!["mobile".to_owned()];
    let notices: Vec<_> = skip_notices(&run)
        .into_iter()
        .map(|notice| match notice {
            CoreToShell::Notification(notice) => notice,
            other => panic!("a notice: {other:?}"),
        })
        .collect();
    // Hidden by the layout: the setting can change it.
    assert_eq!(notices[0].message_key, HIDDEN_NOTICE);
    assert_eq!(
        notices[0].args.get("breakpoints").map(String::as_str),
        Some("bp2")
    );
    assert_eq!(notices[0].settings_key.as_deref(), Some(HIDDEN_SETTING));
    // In no view of the grid: no layout setting shows it, so no link to one.
    assert_eq!(notices[1].message_key, NOT_IN_GRID_NOTICE);
    assert_eq!(
        notices[1].args.get("breakpoints").map(String::as_str),
        Some("mobile")
    );
    assert_eq!(notices[1].settings_key, None);
    assert_eq!(notices.len(), 2);
    assert!(skip_notices(&result(vec![view(1, ViewStatus::Passed, None)])).is_empty());
}

#[test]
fn the_log_takes_the_sidecars_summary_when_it_has_one() {
    // RB-090-84: the sidecar shortens a flow's error once; the log only cleans it.
    let mut failed = view(
        3,
        ViewStatus::Failed,
        Some(("locator.click: Timeout\nCall log:", None)),
    );
    if let Some(error) = failed.error.as_mut() {
        error.summary = Some("locator.click: Timeout 10000ms exceeded.".to_owned());
    }
    let lines = failures(&result(vec![failed]), |_| Shown::default());
    assert_eq!(lines[0].reason, "locator.click: Timeout 10000ms exceeded.");
}
