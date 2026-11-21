use schema_org::rich;
use schema_org::vocab::Vocabulary;

use super::*;
use crate::page::read;

fn issues(body: &str, rich: bool) -> Vec<Issue> {
    let page = read(
        "https://example.test/",
        &format!("<html><head>{body}</head><body></body></html>"),
    );
    validate(&page, &Vocabulary::load(), &rich::table(), Options { rich })
}

fn json_ld(json: &str) -> String {
    format!(r#"<script type="application/ld+json">{json}</script>"#)
}

fn named(found: &[Issue]) -> Vec<String> {
    found
        .iter()
        .map(|issue| {
            format!(
                "{} {} {}",
                issue.rule,
                issue.node.as_deref().unwrap_or("-"),
                issue.property.as_deref().unwrap_or("-")
            )
        })
        .collect()
}

#[test]
fn a_complete_page_has_no_issue() {
    let body = json_ld(
        r##"{"@context": "https://schema.org", "@graph": [
  {"@type": "Organization", "@id": "#org", "name": "Example", "url": "https://example.test/", "logo": "https://example.test/logo.png"},
  {"@type": "BreadcrumbList", "itemListElement": [{"@type": "ListItem", "position": 1, "name": "Home", "item": "https://example.test/"}]},
  {"@type": "Article", "headline": "Hello", "author": {"@type": "Person", "name": "Ada"}, "publisher": {"@id": "#org"},
   "datePublished": "2026-01-02", "dateModified": "2026-01-03T10:00:00+01:00", "image": "https://example.test/a.png"}
]}"##,
    );
    assert!(
        issues(&body, true).is_empty(),
        "{:?}",
        named(&issues(&body, true))
    );
}

#[test]
fn a_misspelled_property_is_an_error_at_its_node() {
    let body = json_ld(
        r#"{"@context": "https://schema.org", "@type": "Organization", "name": "Example", "urll": "x", "url": "https://example.test/", "logo": "l.png"}"#,
    );
    let found = issues(&body, true);
    assert_eq!(named(&found), ["property.unknown n0 urll"]);
    assert_eq!(
        found[0].params,
        [
            ("type", "Organization".to_owned()),
            ("property", "urll".to_owned())
        ]
    );
    assert_eq!(found[0].place, "jsonld/0#/urll");
    assert_eq!(found[0].element, "html > head > script");
}

#[test]
fn unknown_types_foreign_properties_and_misplaced_ones_are_told_apart() {
    let body = json_ld(
        r#"{"@context": "https://schema.org", "@graph": [
  {"@type": "Organisation", "name": "x"},
  {"@type": "Person", "name": "Ada", "headline": "not for a person", "https://example.org/own": "kept"}
]}"#,
    );
    assert_eq!(
        named(&issues(&body, false)),
        ["type.unknown n0 -", "property.domain n1 headline"]
    );
}

#[test]
fn values_of_the_wrong_kind_are_named() {
    let body = json_ld(
        r#"{"@context": "https://schema.org", "@type": "Event", "name": "Fair",
  "startDate": "next friday", "isAccessibleForFree": "maybe", "maximumAttendeeCapacity": "many",
  "eventStatus": "on", "organizer": "Example", "location": {"@type": "Place", "name": "Hall"},
  "about": 7, "duration": "PT2H", "eventAttendanceMode": "https://schema.org/OfflineEventAttendanceMode",
  "description": {"@type": "Person", "name": "no text"}}"#,
    );
    assert_eq!(
        named(&issues(&body, false)),
        [
            "property.range n0 startDate",
            "property.range n0 isAccessibleForFree",
            "property.range n0 maximumAttendeeCapacity",
            "property.range n0 eventStatus",
            "property.range n0 description",
        ]
    );
}

#[test]
fn a_reference_is_held_to_the_range_by_the_node_it_names() {
    let body = json_ld(
        r##"{"@context": "https://schema.org", "@graph": [
  {"@type": "Person", "@id": "#ada", "name": "Ada"},
  {"@type": "Product", "name": "Lamp", "offers": {"@id": "#ada"}, "brand": {"@id": "https://example.test/#elsewhere"}}
]}"##,
    );
    assert_eq!(named(&issues(&body, false)), ["property.range n1 offers"]);
}

#[test]
fn a_superseded_property_is_a_hint_naming_its_successor() {
    let body = json_ld(
        r#"{"@context": "https://schema.org", "@type": "Place", "name": "Hall", "map": "https://example.test/map"}"#,
    );
    let found = issues(&body, false);
    assert_eq!(named(&found), ["property.superseded n0 map"]);
    assert_eq!(found[0].params[1], ("replacement", "hasMap".to_owned()));
}

#[test]
fn rich_results_name_what_is_missing_when_switched_on() {
    let body = json_ld(r#"{"@context": "https://schema.org", "@type": "Product", "sku": "A-1"}"#);
    assert!(issues(&body, false).is_empty());
    let found = issues(&body, true);
    assert_eq!(
        named(&found),
        [
            "rich.required n0 name",
            "rich.required_any n0 -",
            "rich.recommended n0 image",
            "rich.recommended n0 description",
            "rich.recommended n0 brand",
        ]
    );
    assert_eq!(
        found[1].params[1],
        ("properties", "offers, review, aggregateRating".to_owned())
    );
}

#[test]
fn broken_json_a_missing_type_and_another_context_are_named() {
    let body = format!(
        "{}{}{}",
        json_ld(r#"{"@context": "https://schema.org", "name": "no type"}"#),
        json_ld(r#"{"@context": "https://example.org/", "@type": "Thing"}"#),
        json_ld("{broken"),
    );
    let found = issues(&body, true);
    assert_eq!(
        named(&found),
        [
            "jsonld.invalid - -",
            "type.missing n0 -",
            "jsonld.context n1 -"
        ]
    );
    assert_eq!(found[0].place, "jsonld/2");
    assert_eq!(found[0].params[0], ("block", "3".to_owned()));
}

#[test]
fn microdata_items_are_held_to_the_same_rules() {
    let page = read(
        "https://example.test/",
        r#"<html><body><div itemscope itemtype="https://schema.org/Product"><span itemprop="name">Lamp</span><span itemprop="colour">red</span>
<div itemprop="offers" itemscope itemtype="https://schema.org/Offer"><meta itemprop="price" content="9.5"></div></div>
<div itemscope><span itemprop="name">untyped</span></div></body></html>"#,
    );
    let found = validate(
        &page,
        &Vocabulary::load(),
        &rich::table(),
        Options { rich: false },
    );
    assert_eq!(
        named(&found),
        ["property.unknown n0 colour", "type.missing n2 -"]
    );
    assert_eq!(
        found[0].place,
        "microdata:html > body > div:nth-of-type(1)#/colour"
    );
}
