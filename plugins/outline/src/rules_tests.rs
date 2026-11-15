use rb_plugin_api::check::valid_rule_id;
use serde_json::json;

use super::*;
use crate::links::State;

fn at(path: &str) -> serde_json::Value {
    json!({ "selector": path, "path": path, "visible": true, "hidden": false })
}

fn with(mut entry: serde_json::Value, fields: serde_json::Value) -> serde_json::Value {
    if let (Some(entry), Some(fields)) = (entry.as_object_mut(), fields.as_object()) {
        entry.extend(fields.clone());
    }
    entry
}

fn page(value: serde_json::Value) -> PageOutline {
    PageOutline::parse(&value.to_string()).expect("outline")
}

fn short(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .map(|f| format!("{} {}", f.rule, f.path.as_deref().unwrap_or("-")))
        .collect()
}

#[test]
fn the_rules_are_described_with_valid_ids_and_their_shared_ids() {
    let described = describe();
    assert_eq!(described.len(), RULES.len());
    for rule in &described {
        assert!(valid_rule_id(&rule.id), "{}", rule.id);
        assert_eq!(rule.title, format!("plugin.outline.rules.{}", rule.id));
    }
    let shared = |id: &str| {
        described
            .iter()
            .find(|rule| rule.id == id)
            .and_then(|rule| rule.shared_id.clone())
    };
    assert_eq!(shared(IMG_ALT_MISSING).as_deref(), Some("img-alt-missing"));
    assert_eq!(shared(LINK_EMPTY).as_deref(), Some("link-name-missing"));
    assert_eq!(
        shared(FORM_LABEL_MISSING).as_deref(),
        Some("form-label-missing")
    );
    assert_eq!(shared(HEADING_SKIP), None);
}

#[test]
fn a_skipped_heading_level_and_a_second_h1_are_found_hidden_ones_ignored() {
    let outline = page(json!({
        "url": "https://site.test/",
        "headings": [
            with(at("h1:nth-of-type(1)"), json!({ "level": 1, "text": "Title" })),
            with(at("h2"), json!({ "level": 2, "text": "Intro" })),
            with(at("h4"), json!({ "level": 4, "text": "Details" })),
            with(at("h5"), json!({ "level": 5, "text": "Hidden", "hidden": true })),
            with(at("h1:nth-of-type(2)"), json!({ "level": 1, "text": "Again" })),
        ],
        "landmarks": [with(at("main"), json!({ "role": "main" }))]
    }));
    let found = findings(&[], 3, &outline, &[], &[]);
    assert_eq!(
        short(&found),
        ["heading-skip h4", "multiple-h1 h1:nth-of-type(2)"]
    );
    assert_eq!(found[0].params[0], ("from".to_owned(), "h2".to_owned()));
    assert_eq!(found[0].params[1], ("to".to_owned(), "h4".to_owned()));
    assert_eq!(found[0].view, Some(3));
    assert_eq!(found[0].url.as_deref(), Some("https://site.test/"));
    assert_eq!(found[0].message, "plugin.outline.findings.heading-skip");
}

#[test]
fn a_page_without_an_exposed_main_is_found_at_html() {
    let outline = page(json!({
        "landmarks": [with(at("main"), json!({ "role": "main", "hidden": true }))]
    }));
    let found = findings(&[], 1, &outline, &["landmarks".to_owned()], &[]);
    assert_eq!(short(&found), ["landmark-main-missing html"]);
}

#[test]
fn empty_links_missing_alts_and_unlabelled_fields_are_found() {
    let outline = page(json!({
        "landmarks": [with(at("main"), json!({ "role": "main" }))],
        "links": [
            with(at("a:nth-of-type(1)"), json!({ "href": "https://site.test/a", "text": "" })),
            with(at("a:nth-of-type(2)"), json!({ "href": "https://site.test/b", "text": "B" })),
        ],
        "images": [
            with(at("img:nth-of-type(1)"), json!({ "src": "a.png", "alt": null })),
            with(at("img:nth-of-type(2)"), json!({ "src": "b.png", "alt": "" })),
            with(at("img:nth-of-type(3)"), json!({ "src": "c.png", "alt": null, "decorative": true })),
        ],
        "fields": [
            with(at("input:nth-of-type(1)"), json!({ "name": "nickname", "label": "" })),
            with(at("input:nth-of-type(2)"), json!({ "name": "email", "label": "Email" })),
        ]
    }));
    let found = findings(&[], 1, &outline, &[], &[]);
    assert_eq!(
        short(&found),
        [
            "link-empty a:nth-of-type(1)",
            "img-alt-missing img:nth-of-type(1)",
            "form-label-missing input:nth-of-type(1)"
        ]
    );
}

#[test]
fn a_broken_link_is_found_at_every_link_to_it() {
    let outline = page(json!({
        "landmarks": [with(at("main"), json!({ "role": "main" }))],
        "links": [
            with(at("a:nth-of-type(1)"), json!({ "href": "https://site.test/gone#x", "text": "Old" })),
            with(at("a:nth-of-type(2)"), json!({ "href": "https://site.test/ok", "text": "Ok" })),
        ]
    }));
    let checked = vec![
        LinkResult {
            url: "https://site.test/gone".to_owned(),
            state: State::Broken,
            status: Some(404),
            reason: None,
            final_url: None,
        },
        LinkResult {
            url: "https://site.test/ok".to_owned(),
            state: State::Ok,
            status: Some(200),
            reason: None,
            final_url: None,
        },
    ];
    let found = findings(&[], 1, &outline, &[], &checked);
    assert_eq!(short(&found), ["link-broken a:nth-of-type(1)"]);
    assert!(
        found[0]
            .params
            .contains(&("status".to_owned(), "404".to_owned()))
    );
}

#[test]
fn only_the_rules_the_run_asks_for_report() {
    let outline = page(json!({ "headings": [] }));
    let found = findings(&[LINK_EMPTY.to_owned()], 1, &outline, &[], &[]);
    assert!(found.is_empty(), "landmark-main-missing was not asked for");
}
