use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

#[test]
fn the_collector_is_one_expression_that_only_reads() {
    let script = COLLECT_JS.trim_end();
    assert!(script.ends_with("})()"), "an expression, evaluated as is");
    for forbidden in [
        "scrollTo",
        "scrollBy",
        "scrollIntoView",
        ".click(",
        "innerHTML =",
        "fetch(",
    ] {
        assert!(
            !script.contains(forbidden),
            "the collector never {forbidden}"
        );
    }
}

#[test]
fn the_answer_is_parsed_and_sizes_kept_by_path() {
    let answer = json!({
        "url": "https://a.test/",
        "html": "<html><head><link rel=canonical href=/x></head><body><img src=a></body></html>",
        "truncated": false,
        "images": [{"path": "html > body > img", "width": 10, "height": 12}],
        "backgrounds": [{"path": "html > body > div", "url": "hero.jpg", "width": 800, "height": 400}]
    });
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = answer.to_string();
    let rendered = Rendered::collect(&mut host, 7).expect("rendered");
    assert_eq!(host.evaluated, [(7, COLLECT_JS.to_owned())]);
    assert_eq!(rendered.view, 7);
    assert_eq!(rendered.url, "https://a.test/");
    assert_eq!(rendered.sizes["html > body > img"], (10, 12));
    assert_eq!(rendered.backgrounds[0].url, "hero.jpg");
    assert_eq!(crate::page::facts(&rendered.html).canonicals, ["/x"]);
}

#[test]
fn a_broken_answer_is_a_coded_failure() {
    let failure = Rendered::parse(1, "null").expect_err("no collection");
    assert_eq!(failure.code(), Some("plugin.seo.errors.bad_render"));
}
