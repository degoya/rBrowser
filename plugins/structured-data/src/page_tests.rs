use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, codes};
use serde_json::json;

use super::*;
use crate::tree::{Syntax, Val};

const PAGE: &str = r##"<html><head>
<script type="application/ld+json">{"@context": "https://schema.org", "@type": "Organization", "@id": "#org", "name": "Example"}</script>
<script type="Application/LD+JSON">{"@context": "https://schema.org", "@type": "Article", "headline": "Hi", "publisher": {"@id": "#org"}}</script>
<script type="application/ld+json">{"@type": "Thing", broken</script>
</head><body>
<div itemscope itemtype="https://schema.org/Product"><span itemprop="name">Lamp</span></div>
<p vocab="https://schema.org/" typeof="Person"><span property="name">Ada</span></p>
</body></html>"##;

#[test]
fn every_block_and_item_of_the_page_is_read_in_order() {
    let page = read("https://example.test/", PAGE);
    assert_eq!(page.blocks.len(), 3);
    assert_eq!(page.blocks[0].path, "html > head > script:nth-of-type(1)");
    assert!(
        page.blocks
            .iter()
            .take(2)
            .all(|block| block.error.is_none())
    );
    assert!(page.blocks[2].error.is_some());
    assert_eq!(
        page.blocks[2].text.as_deref(),
        Some(r#"{"@type": "Thing", broken"#)
    );
    let kinds: Vec<(Syntax, &str)> = page
        .nodes
        .iter()
        .map(|node| (node.source.syntax, node.types[0].as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (Syntax::JsonLd, "Organization"),
            (Syntax::JsonLd, "Article"),
            (Syntax::Microdata, "Product"),
            (Syntax::Rdfa, "Person"),
        ]
    );
    assert_eq!(
        page.nodes[1].values("publisher"),
        [Val::Ref {
            at_id: "#org".to_owned(),
            target: Some("n0".to_owned())
        }]
    );
    assert!(!page.truncated);
}

#[test]
fn a_long_block_keeps_its_start_and_says_it_was_cut() {
    let long = format!(
        r#"<script type="application/ld+json">{{"@type": "Thing", "description": "{}"}}</script>"#,
        "x".repeat(MAX_RAW)
    );
    let page = read("https://example.test/", &long);
    assert!(page.blocks[0].text_cut);
    assert_eq!(
        page.blocks[0]
            .text
            .as_deref()
            .map(|text| text.chars().count()),
        Some(MAX_RAW)
    );
}

fn host(answer: serde_json::Value) -> MemoryHost {
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = answer.to_string();
    host
}

#[test]
fn the_collector_s_document_is_read() {
    let mut host =
        host(json!({"url": "https://example.test/a", "length": PAGE.len(), "html": PAGE}));
    let page = collect(&mut host, 3).expect("page");
    assert_eq!(page.url, "https://example.test/a");
    assert_eq!(page.nodes.len(), 4);
    assert_eq!(host.evaluated, [(3, COLLECTOR.to_owned())]);
}

#[test]
fn a_document_too_long_to_send_is_named() {
    let mut host = host(json!({"url": "https://example.test/", "length": 9_000_000, "html": null}));
    let failure = collect(&mut host, 1).expect_err("too large");
    assert_eq!(failure.code(), Some(codes::TOO_LARGE));
    assert_eq!(failure.param("length"), Some("9000000"));
}

#[test]
fn an_answer_that_is_no_document_is_bad_output() {
    let mut host = host(json!(null));
    let failure = collect(&mut host, 1).expect_err("bad output");
    assert_eq!(failure.code(), Some(codes::BAD_OUTPUT));
}
