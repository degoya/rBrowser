use serde_json::{Value, json};

use super::{CATEGORY_NOT_FOUND, Filter, KEY_NOT_FOUND, SECTION_NOT_FOUND, deep_link, entry, view};

fn key(key: &str, category: &str, section: &str, label: &str, description: &str) -> Value {
    json!({
        "key": key, "category": category, "section": section, "type": "boolean",
        "value": true, "origin": "default", "default": true,
        "label": label, "description": description,
    })
}

fn document() -> Value {
    json!({
        "categories": [
            { "id": "general", "title": "General", "sections": [
                { "id": "startup", "title": "Startup" },
                { "id": "updates", "title": "Updates" },
            ]},
            { "id": "sync", "title": "Sync", "sections": [
                { "id": "scroll", "title": "Scroll" },
            ]},
            { "id": "automation", "title": "Automation", "sections": [
                { "id": "mcp", "title": "MCP" },
            ]},
        ],
        "keys": [
            key("startup.openLastProject", "general", "startup", "Open the last project", "At start."),
            key("sync.scroll", "sync", "scroll", "Scroll sync", "Scroll every view together."),
            key("mcp.enabled", "automation", "mcp", "MCP server", "Let AI agents control rBrowser."),
        ],
    })
}

fn ids(answer: &Value) -> Vec<String> {
    answer["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|category| {
            category["sections"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|section| section["keys"].as_array().into_iter().flatten())
                .filter_map(|entry| entry["key"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn without_arguments_every_category_and_section_in_the_window_s_order() {
    let answer = view(&document(), &Filter::default()).expect("the structure");
    let categories: Vec<&str> = answer["categories"]
        .as_array()
        .expect("categories")
        .iter()
        .filter_map(|category| category["id"].as_str())
        .collect();
    assert_eq!(categories, ["general", "sync", "automation"]);
    let general = &answer["categories"][0];
    assert_eq!(general["title"], "General");
    assert_eq!(
        general["sections"][1]["id"], "updates",
        "an empty section stays"
    );
    assert_eq!(general["sections"][1]["keys"], json!([]));
    assert_eq!(
        ids(&answer),
        ["startup.openLastProject", "sync.scroll", "mcp.enabled"]
    );
    assert_eq!(answer["count"], 3);
    assert_eq!(
        answer["categories"][1]["sections"][0]["keys"][0]["deepLink"],
        "rbrowser://settings/sync.scroll"
    );
}

#[test]
fn one_key_with_its_deep_link_or_a_clear_error() {
    let one = view(
        &document(),
        &Filter {
            key: Some("sync.scroll"),
            ..Filter::default()
        },
    )
    .expect("the key");
    assert_eq!(one["label"], "Scroll sync");
    assert_eq!(one["deepLink"], deep_link("sync.scroll"));
    let missing = view(
        &document(),
        &Filter {
            key: Some("sync.nope"),
            ..Filter::default()
        },
    )
    .expect_err("unknown");
    assert_eq!(missing.code, KEY_NOT_FOUND);
    assert_eq!(
        entry(&document(), "mcp.enabled").expect("there")["origin"],
        "default"
    );
}

#[test]
fn category_and_section_narrow_and_unknown_ones_are_named() {
    let automation = view(
        &document(),
        &Filter {
            category: Some("automation"),
            ..Filter::default()
        },
    )
    .expect("a category");
    assert_eq!(ids(&automation), ["mcp.enabled"]);
    assert_eq!(automation["categories"].as_array().map(Vec::len), Some(1));
    let scroll = view(
        &document(),
        &Filter {
            section: Some("scroll"),
            ..Filter::default()
        },
    )
    .expect("a section");
    assert_eq!(ids(&scroll), ["sync.scroll"]);
    let unknown = view(
        &document(),
        &Filter {
            category: Some("nope"),
            ..Filter::default()
        },
    )
    .expect_err("unknown category");
    assert_eq!(unknown.code, CATEGORY_NOT_FOUND);
    assert!(
        unknown.message.contains("automation"),
        "{}",
        unknown.message
    );
    let wrong = view(
        &document(),
        &Filter {
            category: Some("sync"),
            section: Some("mcp"),
            ..Filter::default()
        },
    )
    .expect_err("the section is in another category");
    assert_eq!(wrong.code, SECTION_NOT_FOUND);
}

#[test]
fn the_search_needs_every_word_in_label_key_or_description_ignoring_case() {
    let search = |query: &str| {
        ids(&view(
            &document(),
            &Filter {
                query: Some(query),
                ..Filter::default()
            },
        )
        .expect("a search"))
    };
    assert_eq!(search("SCROLL"), ["sync.scroll"]);
    assert_eq!(search("agents control"), ["mcp.enabled"]);
    assert_eq!(search("mcp.enabled"), ["mcp.enabled"]);
    assert!(search("scroll agents").is_empty());
    assert_eq!(search("  ").len(), 3, "an empty query matches all");
}
