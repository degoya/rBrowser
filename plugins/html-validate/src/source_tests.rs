use rb_plugin_api::{Failure, FailureKind};
use serde_json::{Value, json};

use super::{MAX_SOURCE_BYTES, from_answer, render_script, rendered, served};
use crate::fake::{FakeHost, PAGE, fixture};

fn code(failure: &Failure) -> &str {
    failure.code().unwrap_or_default()
}

#[test]
fn reads_the_main_frame_document_as_the_view_loaded_it() {
    let source = fixture("invalid.html");
    let mut host = FakeHost::with_views(&[(1, "mobile")], &source, "{}");
    let served = served(&mut host, 1).expect("served");
    assert_eq!(served.url, PAGE);
    assert_eq!(served.text, source);
    let methods: Vec<&str> = host.cdp.iter().map(|(_, m, _)| m.as_str()).collect();
    assert_eq!(methods, ["Page.getFrameTree", "Page.getResourceContent"]);
}

#[test]
fn a_view_of_another_engine_stays_the_hosts_refusal() {
    let mut host = FakeHost::default();
    let failure = served(&mut host, 7).expect_err("webkit");
    assert_eq!(failure.unserved_engine_in(7), Some("webkit"));
}

#[test]
fn a_page_that_is_no_web_page_is_refused_with_its_url() {
    let mut host = FakeHost::with_views(&[(1, "mobile")], "<p>", "{}");
    host.sources
        .insert(1, ("about:blank".to_owned(), Ok("<p>".to_owned())));
    let failure = served(&mut host, 1).expect_err("about:blank");
    assert_eq!(code(&failure), "plugin.html-validate.errors.not_web");
    assert_eq!(failure.param("url"), Some("about:blank"));
    assert_eq!(host.cdp.len(), 1, "no resource asked for");
}

#[test]
fn a_source_the_view_does_not_have_is_no_source_with_the_reason() {
    let mut host = FakeHost::with_views(&[(1, "mobile")], "", "{}");
    host.sources.insert(
        1,
        (
            PAGE.to_owned(),
            Err(Failure::coded(
                FailureKind::NotFound,
                "plugins.error.not_found",
            )),
        ),
    );
    let failure = served(&mut host, 1).expect_err("missing");
    assert_eq!(code(&failure), "plugin.html-validate.errors.no_source");
    assert_eq!(failure.param("view"), Some("1"));
    assert_eq!(failure.param("reason"), Some("plugins.error.not_found"));
}

/// RB-094-13 (owner, diag33): the host's own reason - the browser's words - reaches the text,
/// not only its code.
#[test]
fn the_hosts_reason_is_the_reason() {
    let mut host = FakeHost::with_views(&[(1, "mobile")], "", "{}");
    host.sources.insert(
        1,
        (
            PAGE.to_owned(),
            Err(
                Failure::coded(FailureKind::Other, "plugins.error.host_failed")
                    .with_param("reason", "Agent is not enabled."),
            ),
        ),
    );
    let failure = served(&mut host, 1).expect_err("failed");
    assert_eq!(failure.param("reason"), Some("Agent is not enabled."));
}

#[test]
fn a_source_over_the_limit_is_refused_with_both_sizes() {
    let big = "a".repeat(MAX_SOURCE_BYTES + 1);
    let mut host = FakeHost::with_views(&[(1, "mobile")], &big, "{}");
    let failure = served(&mut host, 1).expect_err("too large");
    assert_eq!(code(&failure), "plugin.html-validate.errors.too_large");
    assert_eq!(failure.param("limit"), Some("1024"));
    assert_eq!(failure.param("size"), Some("1025"));
}

#[test]
fn a_binary_answer_is_no_source() {
    let failure = from_answer(1, r#"{"content":"PGI+","base64Encoded":true}"#).expect_err("base64");
    assert_eq!(code(&failure), "plugin.html-validate.errors.no_source");
    assert_eq!(failure.param("reason"), Some("not a text document"));
}

#[test]
fn an_answer_without_content_is_no_source() {
    let failure = from_answer(1, r#"{"base64Encoded":false}"#).expect_err("no content");
    assert_eq!(code(&failure), "plugin.html-validate.errors.no_source");
}

// RB-094-06: the rendered DOM, serialized in the view's isolated world.

fn rendered_host(answer: Result<String, Failure>) -> FakeHost {
    let mut host = FakeHost::with_views(&[(1, "mobile")], "<p>served</p>", "{}");
    host.rendered.insert(1, answer);
    host
}

#[test]
fn reads_the_rendered_dom_through_the_render_script() {
    let html = "<!DOCTYPE html>\n<html><head></head><body><p>rendered</p></body></html>";
    let mut host = rendered_host(Ok(json!({ "html": html }).to_string()));
    let got = rendered(&mut host, 1).expect("rendered");
    assert_eq!(got.url, PAGE);
    assert_eq!(got.text, html);
    // The frame tree for the URL, never the served resource.
    let methods: Vec<&str> = host.cdp.iter().map(|(_, m, _)| m.as_str()).collect();
    assert_eq!(methods, ["Page.getFrameTree"]);
    assert_eq!(host.evaluated, [(1, render_script())]);
}

#[test]
fn the_rendered_answer_may_come_as_a_json_string() {
    let inner = json!({ "html": "<html></html>" }).to_string();
    let mut host = rendered_host(Ok(Value::String(inner).to_string()));
    assert_eq!(
        rendered(&mut host, 1).expect("rendered").text,
        "<html></html>"
    );
}

#[test]
fn the_render_script_serializes_the_doctype_and_the_document_element_within_the_limit() {
    let script = render_script();
    assert!(script.contains("document.doctype"));
    assert!(script.contains("document.documentElement") && script.contains("outerHTML"));
    assert!(script.contains(&MAX_SOURCE_BYTES.to_string()));
    // An expression, nothing left in the world's globals.
    assert!(script.starts_with("(() =>") && script.ends_with(")()"));
}

#[test]
fn a_rendered_dom_over_the_limit_is_too_large() {
    let mut host = rendered_host(Ok(json!({ "tooLarge": 2_097_152 }).to_string()));
    let failure = rendered(&mut host, 1).expect_err("too large");
    assert_eq!(code(&failure), "plugin.html-validate.errors.too_large");
    assert_eq!(failure.param("size"), Some("2048"));
    // The page script counts characters; the bytes are held to the limit again.
    let big = "\u{e9}".repeat(MAX_SOURCE_BYTES / 2 + 1);
    let mut host = rendered_host(Ok(json!({ "html": big }).to_string()));
    let failure = rendered(&mut host, 1).expect_err("too many bytes");
    assert_eq!(code(&failure), "plugin.html-validate.errors.too_large");
}

#[test]
fn a_rendered_answer_that_does_not_read_is_no_source() {
    for answer in [
        Ok("[1]".to_owned()),
        Ok("{}".to_owned()),
        Ok("not json".to_owned()),
        Err(Failure::new(FailureKind::Io)),
    ] {
        let mut host = rendered_host(answer.clone());
        let failure = rendered(&mut host, 1).expect_err("no source");
        assert_eq!(
            code(&failure),
            "plugin.html-validate.errors.no_source",
            "{answer:?}"
        );
    }
}

#[test]
fn the_rendered_dom_of_a_page_that_is_no_web_page_is_refused() {
    let mut host = rendered_host(Ok(json!({ "html": "<p>" }).to_string()));
    host.sources
        .insert(1, ("about:blank".to_owned(), Ok(String::new())));
    let failure = rendered(&mut host, 1).expect_err("about:blank");
    assert_eq!(code(&failure), "plugin.html-validate.errors.not_web");
    assert!(host.evaluated.is_empty());
}
