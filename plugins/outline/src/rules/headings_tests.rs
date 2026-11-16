use serde_json::json;

use crate::rules::test_util::{heading, landmark, page, param, short, with};
use crate::rules::*;
use crate::settings::Settings;

fn run(headings: serde_json::Value) -> Vec<rb_plugin_api::Finding> {
    let outline = page(json!({ "url": "https://site.test/", "headings": headings }));
    findings(
        &[],
        1,
        &outline,
        &["headings".to_owned()],
        &[],
        &Settings::default(),
    )
}

#[test]
fn a_page_without_an_exposed_h1_is_found_at_html_with_the_seo_and_axe_id() {
    let found = run(json!([
        heading("h2", 2, "Intro"),
        with(heading("h1", 1, "Hidden title"), json!({ "hidden": true })),
    ]));
    assert_eq!(short(&found), ["h1-missing html"]);
    let none = run(json!([]));
    assert_eq!(short(&none), ["h1-missing html"]);
    let rule = RULES.iter().find(|r| r.id == H1_MISSING).expect("rule");
    assert_eq!(rule.shared, Some("h1-missing"));
}

#[test]
fn a_first_heading_that_is_no_h1_is_found_only_when_an_h1_follows() {
    let found = run(json!([
        heading("p > h3", 3, "Teaser"),
        heading("h1", 1, "Title"),
    ]));
    assert_eq!(short(&found), ["heading-first-not-h1 p > h3"]);
    assert_eq!(param(&found[0], "level"), Some("h3"));
    assert_eq!(param(&found[0], "text"), Some("Teaser"));
    assert!(run(json!([heading("h1", 1, "Title"), heading("h2", 2, "A")])).is_empty());
}

#[test]
fn an_empty_heading_and_one_of_an_image_without_alt_are_found() {
    let found = run(json!([
        heading("h1", 1, "Title"),
        heading("h2:nth-of-type(1)", 2, ""),
        with(
            heading("h2:nth-of-type(2)", 2, ""),
            json!({ "image": true })
        ),
        with(
            heading("h2:nth-of-type(3)", 2, ""),
            json!({ "name": "Logo" })
        ),
        with(
            heading("h2:nth-of-type(4)", 2, ""),
            json!({ "hidden": true })
        ),
    ]));
    assert_eq!(
        short(&found),
        [
            "heading-empty h2:nth-of-type(1)",
            "heading-image-no-alt h2:nth-of-type(2)"
        ]
    );
    for id in [HEADING_EMPTY, HEADING_IMAGE_NO_ALT] {
        let rule = RULES.iter().find(|r| r.id == id).expect("rule");
        assert_eq!(rule.shared, Some("heading-empty"), "{id}");
    }
}

#[test]
fn a_heading_longer_than_the_setting_is_found_with_its_full_length() {
    let long = "x".repeat(130);
    let found = run(json!([
        heading("h1", 1, "Title"),
        heading("h2:nth-of-type(1)", 2, &long),
        with(
            heading("h2:nth-of-type(2)", 2, &"y".repeat(100)),
            json!({ "length": 900 })
        ),
        heading("h2:nth-of-type(3)", 2, &"z".repeat(120)),
    ]));
    assert_eq!(
        short(&found),
        [
            "heading-too-long h2:nth-of-type(1)",
            "heading-too-long h2:nth-of-type(2)"
        ]
    );
    assert_eq!(param(&found[0], "length"), Some("130"));
    assert_eq!(param(&found[0], "max"), Some("120"));
    assert_eq!(param(&found[1], "length"), Some("900"));
    assert_eq!(param(&found[1], "text").map(str::len), Some(80));

    let outline = page(json!({ "headings": [heading("h1", 1, &long)] }));
    let wider = Settings {
        heading_max_chars: 200,
        ..Settings::default()
    };
    assert!(
        findings(&[], 1, &outline, &[], &[], &wider)
            .iter()
            .all(|f| f.rule != HEADING_TOO_LONG)
    );
}

#[test]
fn the_heading_rules_stay_quiet_without_the_headings_section() {
    let outline = page(json!({
        "headings": [heading("h2", 2, "")],
        "landmarks": [landmark("main", "main", "")]
    }));
    let found = findings(
        &[],
        1,
        &outline,
        &["landmarks".to_owned()],
        &[],
        &Settings::default(),
    );
    assert!(found.is_empty(), "{:?}", short(&found));
}
