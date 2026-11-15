use rb_plugin_api::{Failure, FailureKind, codes};
use serde_json::json;

use super::*;
use crate::fake::{FakeHost, Outcome, fixture};

fn host() -> FakeHost {
    FakeHost::with_views(&[(1, "mobile", 375)])
}

#[test]
fn starts_polls_and_fetches_a_report_in_slices() {
    let mut host = host().done();
    host.slice = 10_000;
    let job = start(&mut host, "http://localhost:5173/", "mobile", &json!({})).expect("starts");
    assert_eq!(job, "lh-1");
    let ended = wait(&mut host, &job, "mobile").expect("ends");
    let lhr = fixture("lhr-mobile.json");
    assert_eq!(
        ended,
        Ended::Done {
            final_url: "http://localhost:5173/".to_owned(),
            json_bytes: lhr.len() as u64,
            html_bytes: 19,
        }
    );
    assert_eq!(report(&mut host, &job, "json").expect("the LHR"), lhr);
    assert_eq!(
        report(&mut host, &job, "html").expect("the report"),
        "<html>report</html>"
    );
    release(&mut host, &job);
    let methods = host.methods();
    assert_eq!(
        &methods[..3],
        ["lighthouse.start", "lighthouse.poll", "lighthouse.poll"]
    );
    assert!(
        methods
            .iter()
            .filter(|method| **method == "lighthouse.report")
            .count()
            > 2,
        "the LHR comes in several slices"
    );
    assert_eq!(methods.last(), Some(&"lighthouse.release"));
    assert_eq!(host.calls[1].1["waitMs"], POLL_WAIT_MS);
}

#[test]
fn a_failed_or_cancelled_job_says_so() {
    let mut host = host().then(Outcome::Failed {
        code: -32014,
        message: "net::ERR_CONNECTION_REFUSED".to_owned(),
    });
    let job = start(&mut host, "http://localhost:1/", "mobile", &json!({})).expect("starts");
    assert_eq!(
        wait(&mut host, &job, "mobile").expect("ends"),
        Ended::Failed {
            message: "net::ERR_CONNECTION_REFUSED".to_owned()
        }
    );
    let mut host = FakeHost::with_views(&[]).then(Outcome::Cancelled);
    let job = start(&mut host, "http://localhost:1/", "mobile", &json!({})).expect("starts");
    assert_eq!(
        wait(&mut host, &job, "mobile").expect("ends"),
        Ended::Cancelled
    );
}

/// RB-080-58 review 1: the browser of the setting does not exist - a sentence of its own.
#[test]
fn a_missing_set_browser_is_a_failure_of_its_own() {
    let mut host = host().then(Outcome::Failed {
        code: -32015,
        message: "~\\gone\\chrome.exe does not exist".to_owned(),
    });
    let job = start(&mut host, "http://localhost:1/", "mobile", &json!({})).expect("starts");
    let failure = wait(&mut host, &job, "mobile").expect_err("missing");
    assert_eq!(
        failure.code(),
        Some("plugin.lighthouse.errors.browser_missing")
    );
    assert_eq!(
        failure.param("reason"),
        Some("~\\gone\\chrome.exe does not exist")
    );
    let refused = Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param(
        "reason",
        "`lighthouse.poll` failed (-32015): /x does not exist",
    );
    let mapped = map_failure(refused);
    assert_eq!(
        mapped.code(),
        Some("plugin.lighthouse.errors.browser_missing")
    );
    assert_eq!(mapped.param("reason"), Some("/x does not exist"));
}

#[test]
fn no_browser_is_a_failure_of_its_own() {
    let mut host = host().then(Outcome::Failed {
        code: -32010,
        message: "no browser".to_owned(),
    });
    let job = start(&mut host, "http://localhost:1/", "mobile", &json!({})).expect("starts");
    let failure = wait(&mut host, &job, "mobile").expect_err("no browser");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_browser"));
}

#[test]
fn a_job_that_hangs_is_stopped() {
    let mut host = host().then(Outcome::Hangs);
    let job = start(&mut host, "http://localhost:1/", "mobile", &json!({})).expect("starts");
    let failure = wait(&mut host, &job, "mobile").expect_err("times out");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.timed_out"));
    assert_eq!(failure.param("breakpoint"), Some("mobile"));
    assert_eq!(host.methods().last(), Some(&"lighthouse.release"));
}

#[test]
fn the_host_failures_become_the_plugins() {
    let missing = Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE)
        .with_param("capability", "sidecar");
    assert_eq!(
        map_failure(missing).code(),
        Some("plugin.lighthouse.errors.no_sidecar")
    );
    let no_node = Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param(
        "reason",
        "no Node runtime: C:\\rbrowser\\node\\node.exe does not exist and engines.nodePath is empty",
    );
    let mapped = map_failure(no_node);
    assert_eq!(mapped.code(), Some("plugin.lighthouse.errors.no_sidecar"));
    assert!(
        mapped
            .param("reason")
            .unwrap_or_default()
            .contains("no Node runtime")
    );
    let refused = Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param(
        "reason",
        "`lighthouse.start` failed (-32602): url must be http or https",
    );
    assert_eq!(
        map_failure(refused).code(),
        Some("plugin.lighthouse.errors.sidecar_failed")
    );
    let browser = Failure::coded(FailureKind::Other, codes::HOST_FAILED)
        .with_param("reason", "`lighthouse.poll` failed (-32010): no Chromium");
    assert_eq!(
        map_failure(browser).code(),
        Some("plugin.lighthouse.errors.no_browser")
    );
    let denied = Failure::coded(FailureKind::AccessDenied, codes::DENIED).with_param("method", "x");
    assert_eq!(map_failure(denied).code(), Some(codes::DENIED));
}

#[test]
fn every_call_fails_without_a_sidecar() {
    let mut host = host();
    host.sidecar_failure = Some(Failure::coded(
        FailureKind::NotSupported,
        codes::NOT_AVAILABLE,
    ));
    let failure = start(&mut host, "http://localhost/", "m", &json!({})).expect_err("no sidecar");
    assert_eq!(failure.code(), Some("plugin.lighthouse.errors.no_sidecar"));
    assert!(cancel(&mut host).is_err());
    assert!(status(&mut host).is_err());
}
