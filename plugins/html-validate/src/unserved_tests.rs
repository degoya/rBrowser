use rb_plugin_api::{Failure, FailureKind};
use serde_json::json;

use super::{PageState, STATE_SCRIPT, Why, classify};
use crate::fake::{FakeHost, PAGE};
use crate::source::served;

/// What Chromium answered for a document it did not keep (owner, diag40).
const NOT_CACHED: &str = "CDP Page.getResourceContent failed (-32000): Content unavailable. \
                          Resource was not cached";

fn code(failure: &Failure) -> &str {
    failure.code().unwrap_or_default()
}

fn methods(host: &FakeHost) -> Vec<&str> {
    host.cdp.iter().map(|(_, m, _)| m.as_str()).collect()
}

/// A view whose document the browser does not hold, failing with `reason`, its page reporting
/// `state` to the probe.
fn unserved_host(reason: &str, state: Result<String, Failure>) -> FakeHost {
    let mut host = FakeHost::with_views(&[(1, "mobile")], "", "{}");
    host.sources.insert(
        1,
        (
            PAGE.to_owned(),
            Err(
                Failure::coded(FailureKind::Other, "plugins.error.host_failed")
                    .with_param("reason", reason),
            ),
        ),
    );
    host.states.insert(1, state);
    host
}

fn state(status: u16, ready: &str) -> Result<String, Failure> {
    Ok(json!({ "status": status, "ready": ready }).to_string())
}

#[test]
fn a_page_that_asks_for_a_sign_in_says_so_with_its_status() {
    for status in [401_u16, 407] {
        let mut host = unserved_host(NOT_CACHED, state(status, "complete"));
        let failure = served(&mut host, 1).expect_err("sign-in");
        assert_eq!(code(&failure), "plugin.html-validate.errors.sign_in");
        assert_eq!(failure.param("view"), Some("1"));
        assert_eq!(failure.param("status"), Some(status.to_string().as_str()));
        assert_eq!(failure.kind, FailureKind::AccessDenied);
        // The sign-in page's body is never validated in the page's place.
        assert_eq!(
            methods(&host),
            ["Page.getFrameTree", "Page.getResourceContent"]
        );
        assert_eq!(host.evaluated, [(1, STATE_SCRIPT.to_owned())]);
    }
}

#[test]
fn a_page_still_loading_says_it_is_not_loaded_yet() {
    let mut host = unserved_host(NOT_CACHED, state(200, "loading"));
    let failure = served(&mut host, 1).expect_err("loading");
    assert_eq!(code(&failure), "plugin.html-validate.errors.not_loaded");
    assert_eq!(failure.param("view"), Some("1"));
}

#[test]
fn a_document_the_view_did_not_keep_says_so_with_the_browsers_words_as_detail() {
    let mut host = unserved_host(NOT_CACHED, state(200, "complete"));
    let failure = served(&mut host, 1).expect_err("not kept");
    assert_eq!(code(&failure), "plugin.html-validate.errors.not_kept");
    assert_eq!(failure.param("view"), Some("1"));
    assert_eq!(failure.param("reason"), Some(NOT_CACHED));
    // The view's own network log was asked, by the document's request - nothing went out.
    assert_eq!(
        methods(&host),
        [
            "Page.getFrameTree",
            "Page.getResourceContent",
            "Network.getResponseBody"
        ]
    );
}

#[test]
fn the_body_the_views_network_log_holds_is_validated_without_a_second_request() {
    let mut host = unserved_host(NOT_CACHED, state(200, "complete"));
    host.bodies.insert(
        1,
        Ok(json!({ "body": "<!DOCTYPE html><p>kept</p>", "base64Encoded": false }).to_string()),
    );
    let got = served(&mut host, 1).expect("the kept body");
    assert_eq!(got.url, PAGE);
    assert_eq!(got.text, "<!DOCTYPE html><p>kept</p>");
    let (_, _, params) = host.cdp.last().expect("asked");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(params).expect("json")["requestId"],
        "L1"
    );
}

#[test]
fn a_binary_kept_body_is_no_source_to_validate() {
    let mut host = unserved_host(NOT_CACHED, state(200, "complete"));
    host.bodies.insert(
        1,
        Ok(json!({ "body": "PGI+", "base64Encoded": true }).to_string()),
    );
    let failure = served(&mut host, 1).expect_err("binary");
    assert_eq!(code(&failure), "plugin.html-validate.errors.not_kept");
}

#[test]
fn a_kept_body_is_not_validated_when_the_page_status_is_unknown_or_no_success() {
    // Without the probe the body may be a sign-in page (a 401 behind a cancelled prompt).
    for page in [
        Err(Failure::new(FailureKind::Io)),
        state(500, "complete"),
        state(0, "complete"),
    ] {
        let mut host = unserved_host(NOT_CACHED, page);
        host.bodies.insert(
            1,
            Ok(json!({ "body": "<p>401</p>", "base64Encoded": false }).to_string()),
        );
        let failure = served(&mut host, 1).expect_err("no body of an unknown status");
        assert_eq!(code(&failure), "plugin.html-validate.errors.not_kept");
        assert!(!methods(&host).contains(&"Network.getResponseBody"));
    }
}

#[test]
fn a_probe_that_fails_still_names_the_document_not_kept() {
    let mut host = unserved_host(NOT_CACHED, Err(Failure::new(FailureKind::Io)));
    let failure = served(&mut host, 1).expect_err("not kept");
    assert_eq!(code(&failure), "plugin.html-validate.errors.not_kept");
}

#[test]
fn any_other_reason_stays_no_source_with_the_reason_as_detail() {
    let mut host = unserved_host("timeout", state(200, "complete"));
    let failure = served(&mut host, 1).expect_err("other");
    assert_eq!(code(&failure), "plugin.html-validate.errors.no_source");
    assert_eq!(failure.param("reason"), Some("timeout"));
}

#[test]
fn the_reasons_are_told_apart() {
    let page = |status, loading| Some(PageState { status, loading });
    assert_eq!(classify(page(401, false), NOT_CACHED), Why::SignIn(401));
    assert_eq!(classify(page(407, true), "timeout"), Why::SignIn(407));
    assert_eq!(classify(page(200, true), NOT_CACHED), Why::NotLoaded);
    assert_eq!(classify(page(200, false), NOT_CACHED), Why::NotKept);
    assert_eq!(classify(None, NOT_CACHED), Why::NotKept);
    assert_eq!(
        classify(None, "No resource with given URL found"),
        Why::NotKept
    );
    assert_eq!(classify(page(0, false), "timeout"), Why::Other);
    assert_eq!(
        classify(page(403, false), "Agent is not enabled."),
        Why::Other
    );
}

#[test]
fn the_probe_reads_the_navigation_status_and_the_ready_state_as_an_expression() {
    assert!(STATE_SCRIPT.contains("getEntriesByType('navigation')"));
    assert!(STATE_SCRIPT.contains("responseStatus"));
    assert!(STATE_SCRIPT.contains("document.readyState"));
    assert!(STATE_SCRIPT.starts_with("(() =>") && STATE_SCRIPT.ends_with(")()"));
}

#[test]
fn a_probe_answer_reads_as_an_object_or_a_json_string() {
    let object = json!({ "status": 401, "ready": "complete" }).to_string();
    let string = serde_json::Value::String(object.clone()).to_string();
    for answer in [object, string] {
        assert_eq!(
            super::read_state(&answer),
            Some(PageState {
                status: 401,
                loading: false
            })
        );
    }
    assert_eq!(super::read_state("not json"), None);
    assert_eq!(
        super::read_state("{}"),
        Some(PageState {
            status: 0,
            loading: false
        })
    );
}
