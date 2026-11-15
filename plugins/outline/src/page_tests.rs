use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

#[test]
fn an_answer_is_read_and_its_untrusted_parts_held_to_the_limits() {
    let answer = json!({
        "url": "https://example.test/",
        "lang": "en",
        "headings": [{
            "level": 2, "text": "x".repeat(MAX_TEXT + 50), "selector": "h2", "path": "html > body > h2",
            "hidden": false, "visible": true, "box": [1.4, "nonsense", 1e300, -5]
        }],
        "links": vec![json!({ "href": "https://example.test/a" }); MAX_ENTRIES + 10],
        "images": [{ "src": "a.png", "alt": null }, { "src": "b.png", "alt": "" }]
    });
    let outline = PageOutline::parse(&answer.to_string()).expect("an outline");
    let heading = &outline.headings[0];
    assert_eq!(heading.level, 2);
    assert_eq!(heading.text.chars().count(), MAX_TEXT);
    assert_eq!(heading.at.rect, [1, 0, 10_000_000, -5]);
    assert!(heading.at.visible);
    assert_eq!(outline.links.len(), MAX_ENTRIES);
    assert_eq!(outline.images[0].alt, None);
    assert_eq!(outline.images[1].alt.as_deref(), Some(""));
}

#[test]
fn a_malformed_answer_is_bad_output() {
    let failure = PageOutline::parse("[1, 2]").expect_err("no outline");
    assert_eq!(failure.code(), Some(rb_plugin_api::codes::BAD_OUTPUT));
}

#[test]
fn the_script_embeds_the_collector_and_the_request_as_data() {
    let custom = vec![CustomSelector {
        label: "Quote\"".to_owned(),
        selector: "[data-x='</script>']".to_owned(),
    }];
    let script = script(&["headings".to_owned()], &custom);
    assert!(script.contains("var OutlineCollect"));
    assert!(script.contains("OutlineCollect.run({"));
    assert!(script.contains(r#""label":"Quote\"""#));
    assert!(script.starts_with("(() => {") && script.ends_with("})()"));
}

#[test]
fn collect_evaluates_in_the_view() {
    let mut host = MemoryHost::new().grant(Capability::Evaluate);
    host.evaluate_answer = json!({ "url": "https://example.test/", "headings": [] }).to_string();
    let outline = collect(&mut host, 3, &[], &[]).expect("collected");
    assert_eq!(outline.url, "https://example.test/");
    assert_eq!(host.evaluated[0].0, 3);
}
