use crate::testing::{check, rendered, serve, short};

const URL: &str = "https://shop.test/";

fn found(html: &str) -> Vec<String> {
    let mut host = serve(URL, html, &[]);
    short(&check(&mut host, URL, None), "head.")
}

#[test]
fn a_well_ordered_head_has_no_finding() {
    let html = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width"><title>T</title>
<link rel="preconnect" href="https://cdn.test"><link rel="stylesheet" href="https://cdn.test/a.css">
<script src="/app.js" defer></script></head><body><svg><title>Logo</title></svg>
<meta itemprop="price" content="1"></body></html>"#;
    assert!(found(html).is_empty(), "{:?}", found(html));
}

#[test]
fn a_late_charset_is_found_past_1024_bytes_and_behind_another_element() {
    let late = format!(
        "<html><head><!-- {} --><meta charset=utf-8><title>T</title></head></html>",
        "x".repeat(1100)
    );
    assert_eq!(found(&late), ["head.charset_late head.charset_late"]);
    let second = "<html><head><title>T</title><meta charset=utf-8></head></html>";
    assert_eq!(
        found(second),
        ["head.charset_not_first head.charset_not_first"]
    );
}

#[test]
fn viewport_and_title_after_a_script_and_a_late_preconnect_are_found() {
    let html = r#"<html><head><meta charset=utf-8><script src="https://cdn.test/a.js" async></script>
<meta name="viewport" content="width=device-width"><title>T</title>
<link rel="preconnect" href="https://cdn.test/"></head></html>"#;
    assert_eq!(
        found(html),
        [
            "head.viewport_late head.viewport_late",
            "head.title_late head.title_late",
            "head.preconnect_late head.preconnect_late",
        ]
    );
}

#[test]
fn a_synchronous_script_before_css_is_found_once() {
    let html = r#"<html><head><meta charset=utf-8><title>T</title><script src="/a.js"></script>
<script type="module" src="/m.js"></script><link rel="stylesheet" href="/a.css">
<link rel="stylesheet" href="/b.css"></head></html>"#;
    assert_eq!(
        found(html),
        ["head.script_before_css head.script_before_css"]
    );
}

#[test]
fn head_elements_in_the_body_are_named_with_what_ended_the_head() {
    let html = r#"<html><head><meta charset=utf-8><title>T</title><img src="/pixel.gif">
<link rel="canonical" href="/"><meta name="robots" content="noindex"></head><body></body></html>"#;
    let mut host = serve(URL, html, &[]);
    let findings = check(&mut host, URL, None);
    let stray: Vec<_> = findings
        .iter()
        .filter(|f| f.rule == "head.in_body")
        .collect();
    assert_eq!(stray.len(), 2);
    assert!(
        stray
            .iter()
            .all(|f| f.message == "plugin.seo.findings.head.in_body_after")
    );
    assert!(
        stray[0]
            .params
            .contains(&("breaker".to_owned(), "img".to_owned()))
    );
    let plain = "<html><head><title>T</title></head><body><title>Again</title></body></html>";
    assert_eq!(found(plain), ["head.in_body head.in_body"]);
}

#[test]
fn a_rendered_head_in_another_order_is_a_hint() {
    let raw = "<html><head><meta charset=utf-8><title>T</title></head><body></body></html>";
    let moved = "<html><head><title>T</title><meta charset=utf-8></head><body></body></html>";
    let mut host = serve(URL, raw, &[]);
    let view = rendered(URL, moved);
    let findings = check(&mut host, URL, Some(&view));
    assert_eq!(
        short(&findings, "head."),
        ["head.rendered_differs head.rendered_differs"]
    );
    let mut same = serve(URL, raw, &[]);
    let view = rendered(URL, raw);
    assert!(short(&check(&mut same, URL, Some(&view)), "head.").is_empty());
}
