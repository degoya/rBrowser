use rb_plugin_api::HttpResponse;

use super::*;
use crate::testing::{check, serve, short};

const URL: &str = "https://shop.test/a";

fn head(canonical: &str) -> String {
    format!("<html><head>{canonical}</head><body></body></html>")
}

#[test]
fn link_headers_are_read_with_their_rel() {
    let found = link_header_canonicals(
        [
            r#"<https://a.test/x>; rel="canonical", <https://a.test/en>; rel="alternate"; hreflang="en""#,
            "<https://a.test/y>; rel=canonical",
        ]
        .into_iter(),
    );
    assert_eq!(found, ["https://a.test/x", "https://a.test/y"]);
}

#[test]
fn a_self_canonical_is_quiet_and_a_missing_one_is_reported() {
    let mut host = serve(
        URL,
        &head(r#"<link rel="canonical" href="https://shop.test/a">"#),
        &[],
    );
    assert!(short(&check(&mut host, URL, None), "canonical").is_empty());
    let mut host = serve(URL, &head(""), &[]);
    assert_eq!(
        short(&check(&mut host, URL, None), "canonical"),
        ["canonical.missing canonical.missing"]
    );
}

#[test]
fn two_canonicals_a_relative_one_and_a_header_count() {
    let mut host = serve(
        URL,
        &head(r#"<link rel="canonical" href="/a">"#),
        &[("link", "<https://shop.test/b>; rel=\"canonical\"")],
    );
    assert_eq!(
        short(&check(&mut host, URL, None), "canonical"),
        [
            "canonical.multiple canonical.multiple",
            "canonical.relative canonical.relative"
        ]
    );
}

#[test]
fn a_canonical_elsewhere_is_a_hint_and_its_target_is_held_to_200() {
    let html = head(r#"<link rel="canonical" href="https://shop.test/b">"#);
    let mut host = serve(URL, &html, &[]);
    host.responses.insert(
        "https://shop.test/b".to_owned(),
        HttpResponse {
            status: 301,
            headers: vec![("location".to_owned(), "/c".to_owned())],
            body: Vec::new(),
        },
    );
    assert_eq!(
        short(&check(&mut host, URL, None), "canonical"),
        [
            "canonical.not_self canonical.not_self",
            "canonical.target canonical.target_redirect"
        ]
    );
    let mut host = serve(URL, &html, &[]);
    host.responses.insert(
        "https://shop.test/b".to_owned(),
        HttpResponse {
            status: 200,
            headers: vec![("x-robots-tag".to_owned(), "noindex".to_owned())],
            body: Vec::new(),
        },
    );
    assert_eq!(
        short(&check(&mut host, URL, None), "canonical.target"),
        ["canonical.target canonical.target_noindex"]
    );
}

#[test]
fn an_empty_canonical_is_invalid() {
    let mut host = serve(URL, &head(r#"<link rel="canonical" href="">"#), &[]);
    assert_eq!(
        short(&check(&mut host, URL, None), "canonical"),
        [
            "canonical.relative canonical.relative",
            "canonical.target canonical.target_invalid"
        ]
    );
}
