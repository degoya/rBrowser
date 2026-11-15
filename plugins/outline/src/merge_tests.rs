use serde_json::json;

use super::*;
use crate::page::PageOutline;

fn link(href: &str, text: &str, visible: bool) -> serde_json::Value {
    json!({
        "href": href, "text": text, "selector": format!("a[href='{href}']"),
        "path": "html > body > a", "visible": visible, "hidden": !visible
    })
}

fn outline(links: &[serde_json::Value]) -> PageOutline {
    PageOutline::parse(&json!({ "links": links }).to_string()).expect("outline")
}

#[test]
fn an_entry_shown_only_in_some_views_names_them() {
    let desktop = outline(&[link("/docs", "Docs", true), link("/a", "A", true)]);
    let mobile = outline(&[link("/docs", "Docs", false), link("/a", "A", true)]);
    let value = section("links", &[(1, &desktop), (2, &mobile)]);
    let rows = value["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["item"]["href"], "/docs");
    assert_eq!(rows[0]["only"], json!([1]));
    assert_eq!(rows[0]["views"]["2"]["visible"], false);
    assert_eq!(rows[0]["views"]["1"]["selector"], "a[href='/docs']");
    assert!(rows[1].get("only").is_none(), "visible everywhere");
    assert!(rows[0]["item"].get("selector").is_none());
}

#[test]
fn an_entry_a_view_lacks_has_no_cell_there_and_keeps_its_place() {
    let desktop = outline(&[link("/a", "A", true), link("/c", "C", true)]);
    let mobile = outline(&[
        link("/a", "A", true),
        link("/b", "B", true),
        link("/c", "C", true),
    ]);
    let value = section("links", &[(1, &desktop), (2, &mobile)]);
    let rows = value["rows"].as_array().expect("rows");
    let hrefs: Vec<&str> = rows
        .iter()
        .map(|r| r["item"]["href"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(hrefs, ["/a", "/b", "/c"]);
    assert!(rows[1]["views"].get("1").is_none());
    assert_eq!(rows[1]["only"], json!([2]));
}

#[test]
fn duplicates_in_one_view_stay_separate_rows() {
    let desktop = outline(&[link("/docs", "Docs", true), link("/docs", "Docs", false)]);
    let mobile = outline(&[link("/docs", "Docs", false), link("/docs", "Docs", false)]);
    let value = section("links", &[(1, &desktop), (2, &mobile)]);
    let rows = value["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["only"], json!([1]));
    assert_eq!(rows[1]["only"], json!([]), "visible in no view");
}

#[test]
fn custom_selectors_count_per_view() {
    let hit = |text: &str, visible: bool| json!({ "text": text, "visible": visible, "selector": "div", "path": "html > body > div" });
    let parse = |hits: Vec<serde_json::Value>| {
        PageOutline::parse(
            &json!({ "custom": [{ "label": "CMS", "selector": "[data-cms-block]", "count": hits.len(), "hits": hits }] })
                .to_string(),
        )
        .expect("outline")
    };
    let desktop = parse(vec![hit("Intro", true), hit("Promo", true)]);
    let mobile = parse(vec![hit("Intro", true), hit("Promo", false)]);
    let value = section("custom", &[(1, &desktop), (2, &mobile)]);
    let selector = &value["selectors"][0];
    assert_eq!(selector["label"], "CMS");
    assert_eq!(selector["counts"], json!({ "1": 2, "2": 2 }));
    assert_eq!(selector["rows"][1]["only"], json!([1]));
}

#[test]
fn a_truncated_list_is_named_and_an_unknown_section_is_null() {
    let mut desktop = outline(&[]);
    desktop.truncated = vec!["links".to_owned()];
    assert_eq!(
        section("links", &[(4, &desktop)])["truncatedIn"],
        json!([4])
    );
    assert_eq!(
        section("nonsense", &[(4, &desktop)]),
        serde_json::Value::Null
    );
}
