use rb_config::ProjectConfig;
use serde_json::json;

use super::*;

fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "environments": [
            { "id": "local", "url": "http://localhost:5173" },
            { "id": "staging", "url": "https://staging.example.com/" }
        ]
    }))
    .expect("a config")
}

fn run(project: &mut Value, local: &mut Value) -> Converted {
    let config = config();
    convert(project, local, |url| {
        rb_config::bookmark_paths::stored_path(&config, url)
    })
}

#[test]
fn absolute_bookmarks_of_an_environment_become_paths_and_the_rest_stays() {
    let mut project = json!({ "version": 1, "bookmarks": [
        { "id": "cart", "label": "Cart", "url": "http://localhost:5173/shop/cart?x=1#top",
          "group": "Shop" },
        { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" },
        { "id": "home", "label": "Home", "path": "/" },
        { "id": "pdp", "label": "Product", "url": "https://staging.example.com/products/42",
          "shortcut": "Alt+F7" }
    ] });
    let mut local = json!({ "bookmarks": [
        { "id": "mine", "label": "Mine", "url": "https://staging.example.com" },
        { "id": "cart", "label": "My cart" }
    ] });
    let converted = run(&mut project, &mut local);
    assert_eq!(converted.ids, ["cart", "pdp", "mine"]);
    assert!(converted.project && converted.local);
    assert_eq!(
        project,
        json!({ "version": 1, "bookmarks": [
            { "id": "cart", "label": "Cart", "path": "/shop/cart?x=1#top", "group": "Shop" },
            { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" },
            { "id": "home", "label": "Home", "path": "/" },
            { "id": "pdp", "label": "Product", "path": "/products/42", "shortcut": "Alt+F7" }
        ] })
    );
    // The path takes the place of the URL: the file's key order stays.
    let keys: Vec<&String> = project["bookmarks"][0]
        .as_object()
        .expect("an object")
        .keys()
        .collect();
    assert_eq!(keys, ["id", "label", "path", "group"]);
    assert_eq!(
        local,
        json!({ "bookmarks": [
            { "id": "mine", "label": "Mine", "path": "/" },
            { "id": "cart", "label": "My cart" }
        ] })
    );
    // A second run finds nothing.
    let (before_project, before_local) = (project.clone(), local.clone());
    assert_eq!(run(&mut project, &mut local), Converted::default());
    assert_eq!((project, local), (before_project, before_local));
}

/// `config.local.json` changes a bookmark of the team by id. One file's `path` beside the
/// other's `url` would be a bookmark with both, which the loader drops: such a pair is
/// converted together or not at all.
#[test]
fn a_bookmark_both_files_give_a_target_is_converted_in_both_or_in_neither() {
    let mut project = json!({ "bookmarks": [
        { "id": "both", "label": "Both", "url": "http://localhost:5173/a" },
        { "id": "mixed", "label": "Mixed", "url": "http://localhost:5173/b" },
        { "id": "other", "label": "Other", "url": "https://docs.example.com/c" },
        { "id": "path", "label": "Path", "path": "/d" }
    ] });
    let mut local = json!({ "bookmarks": [
        { "id": "both", "url": "https://staging.example.com/a2" },
        { "id": "mixed", "url": "https://docs.example.com/b2" },
        { "id": "other", "url": "http://localhost:5173/c2" },
        { "id": "path", "url": "http://localhost:5173/d2" }
    ] });
    assert_eq!(run(&mut project, &mut local).ids, ["both"]);
    assert_eq!(project["bookmarks"][0]["path"], "/a");
    assert_eq!(
        local["bookmarks"][0],
        json!({ "id": "both", "path": "/a2" })
    );
    for index in 1..3 {
        assert!(project["bookmarks"][index].get("path").is_none(), "{index}");
        assert!(local["bookmarks"][index].get("path").is_none(), "{index}");
    }
    assert_eq!(
        local["bookmarks"][3],
        json!({ "id": "path", "url": "http://localhost:5173/d2" }),
        "beside the team's path the URL stays what it is"
    );
}

#[test]
fn documents_without_a_bookmark_list_are_left_alone() {
    for (mut project, mut local) in [
        (json!({}), json!({})),
        (json!({ "bookmarks": {} }), json!({ "bookmarks": "x" })),
        (
            json!({ "bookmarks": ["text", { "id": 7, "url": "http://localhost:5173/" }] }),
            json!([]),
        ),
        (
            json!({ "bookmarks": [{ "id": "a", "url": 7 }, { "id": "b", "url": "javascript:x" }] }),
            json!({}),
        ),
    ] {
        let before = (project.clone(), local.clone());
        assert_eq!(run(&mut project, &mut local), Converted::default());
        assert_eq!((project, local), before);
    }
}

/// Review point 8: the files that changed are said, so nothing is compared afterwards.
#[test]
fn only_the_file_a_bookmark_was_converted_in_counts_as_changed() {
    let mut project = json!({ "bookmarks": [{ "id": "a", "label": "A", "path": "/a" }] });
    let mut local = json!({ "bookmarks": [
        { "id": "mine", "label": "Mine", "url": "http://localhost:5173/me" }
    ] });
    let converted = run(&mut project, &mut local);
    assert_eq!(
        converted,
        Converted {
            ids: vec!["mine".to_owned()],
            project: false,
            local: true
        }
    );
}

/// Review point 7: a JSON `null` is no target, as the config's reader has it.
#[test]
fn a_null_path_or_url_is_no_target() {
    let mut project = json!({ "bookmarks": [
        { "id": "a", "label": "A", "path": null, "url": "http://localhost:5173/a" },
        { "id": "b", "label": "B", "url": "http://localhost:5173/b" },
        { "id": "c", "label": "C", "url": null, "path": "/c" }
    ] });
    let mut local = json!({ "bookmarks": [{ "id": "b", "url": null, "path": null }] });
    assert_eq!(run(&mut project, &mut local).ids, ["a", "b"]);
    assert_eq!(
        project["bookmarks"],
        json!([
            { "id": "a", "label": "A", "path": "/a" },
            { "id": "b", "label": "B", "path": "/b" },
            { "id": "c", "label": "C", "url": null, "path": "/c" }
        ])
    );
    assert_eq!(
        local["bookmarks"][0],
        json!({ "id": "b", "url": null, "path": null })
    );
}

#[test]
fn the_bookmarks_the_check_refuses_are_named() {
    let document = json!({ "bookmarks": [
        { "id": "ok", "label": "Ok", "path": "/ok" },
        { "id": "cdn", "label": "Cdn", "path": "//cdn.example.com/x" },
        { "id": "js", "label": "Js", "url": "javascript:alert(1)" },
        { "id": "both", "label": "Both", "path": "/a", "url": "https://a.example" },
        { "id": "null", "label": "Null", "path": null, "url": "https://a.example" },
        { "id": "seven", "label": "Seven", "path": 7 },
        "text"
    ] });
    assert_eq!(invalid_ids(&document), ["cdn", "js", "both", "seven"]);
    assert!(invalid_ids(&json!({})).is_empty());
}
