use rb_api_model::verdict::{Problem, Verdict};
use rb_plugin_types::{Failure, FailureKind};

use super::*;
use crate::api_client::collection::Tally;

fn ran(id: &str, checks: Vec<Verdict>, error: Option<Failure>) -> Ran {
    Ran {
        id: id.into(),
        name: String::new(),
        method: "GET".into(),
        url: "{{baseUrl}}/x".into(),
        sent_url: Some("https://staging.example.com/x".into()),
        status: error.is_none().then_some(200),
        total_ms: None,
        passed: error.is_none() && checks.iter().all(|v| v.passed),
        error,
        checks,
    }
}

fn verdict(check: &str, passed: bool, actual: Option<&str>, problem: Option<Problem>) -> Verdict {
    Verdict {
        check: check.into(),
        passed,
        actual: actual.map(str::to_owned),
        problem,
    }
}

fn report(requests: Vec<Ran>) -> Report {
    Report {
        at: 0,
        folder: None,
        environment: None,
        tally: Tally::default(),
        requests,
        cut: false,
    }
}

#[test]
fn only_failures_become_findings_each_with_a_stable_element() {
    let report = report(vec![
        ran(
            "users/list.http",
            vec![
                verdict("status 2xx", true, Some("200"), None),
                verdict("json $.a == 1", false, Some("2"), None),
                verdict("json $.b == 1", false, None, Some(Problem::Missing)),
            ],
            None,
        ),
        ran(
            "users/ok.http",
            vec![verdict("status 2xx", true, None, None)],
            None,
        ),
        ran(
            "down.http",
            Vec::new(),
            Some(Failure::coded(
                FailureKind::Io,
                "plugin.api-client.errors.send_failed",
            )),
        ),
    ]);
    let findings = of(&report);
    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0].rule, CHECK_FAILED);
    assert_eq!(
        findings[0].message,
        "plugin.api-client.findings.check_failed"
    );
    assert_eq!(findings[0].path.as_deref(), Some("users/list.http#2"));
    assert_eq!(
        findings[0].url.as_deref(),
        Some("https://staging.example.com/x")
    );
    assert!(
        findings[0]
            .params
            .contains(&("actual".to_owned(), "2".to_owned()))
    );
    assert_eq!(findings[1].path.as_deref(), Some("users/list.http#3"));
    assert_eq!(
        findings[1].message,
        "plugin.api-client.findings.check_missing"
    );
    assert_eq!(findings[1].params.len(), 2);
    assert_eq!(findings[2].rule, REQUEST_FAILED);
    assert_eq!(findings[2].severity, Severity::Error);
    assert!(
        findings[2]
            .data
            .as_deref()
            .is_some_and(|data| data.contains("plugin.api-client.errors.send_failed"))
    );
}

#[test]
fn a_clean_run_has_no_findings() {
    let report = report(vec![ran(
        "a.http",
        vec![verdict("time < 100", true, Some("5 ms"), None)],
        None,
    )]);
    assert!(of(&report).is_empty());
}
