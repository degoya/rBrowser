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

#[test]
fn the_analysis_fields_are_read_and_the_content_outside_capped() {
    let answer = json!({
        "headings": [{ "level": 2, "text": "", "name": "Logo", "length": 1e12, "image": true }],
        "focus": [{ "tag": "a", "fixed": true }],
        "outside": vec![json!({ "path": "html > body > p", "text": "Loose" }); MAX_OUTSIDE + 5]
    });
    let outline = PageOutline::parse(&answer.to_string()).expect("an outline");
    let heading = &outline.headings[0];
    assert_eq!(heading.name, "Logo");
    assert!(heading.image);
    assert_eq!(heading.length, 10_000_000);
    assert!(outline.focus[0].fixed);
    assert_eq!(outline.outside.len(), MAX_OUTSIDE);
    assert_eq!(outline.outside[0].text, "Loose");
    assert_eq!(outline.outside[0].at.path, "html > body > p");
}

#[test]
fn the_place_in_the_document_and_the_writing_direction_are_read() {
    let answer = json!({
        "dir": "rtl",
        "headings": [{ "level": 1, "text": "Title", "order": 4 }],
        "focus": [{ "tag": "a", "order": 7.6 }, { "tag": "a" }]
    });
    let outline = PageOutline::parse(&answer.to_string()).expect("an outline");
    assert!(outline.is_rtl());
    assert_eq!(outline.headings[0].order, 4);
    assert_eq!(outline.focus[0].order, 8);
    assert_eq!(outline.focus[1].order, 0, "a missing place is the default");
    for dir in ["ltr", "", "RTL-ish", "sideways"] {
        let outline = PageOutline::parse(&json!({ "dir": dir }).to_string()).expect("an outline");
        assert!(!outline.is_rtl(), "{dir:?} reads left to right");
    }
}
