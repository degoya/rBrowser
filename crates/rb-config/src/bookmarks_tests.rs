use super::*;
use serde_json::{Value, json};

fn config(extra: Value) -> ProjectConfig {
    let mut value = json!({
        "version": 1,
        "environments": [
            { "id": "local", "url": "http://localhost:5173" },
            { "id": "staging", "url": "https://staging.example.com/" }
        ]
    });
    if let (Some(base), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        for (key, item) in extra {
            base.insert(key.clone(), item.clone());
        }
    }
    serde_json::from_value(value).expect("a config")
}

fn keys(issues: &[ConfigIssue]) -> Vec<(&str, &str)> {
    issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect()
}

fn path_bookmark(id: &str, path: &str) -> Bookmark {
    Bookmark {
        id: id.to_owned(),
        label: id.to_owned(),
        path: Some(path.to_owned()),
        url: None,
        group: None,
        shortcut: None,
    }
}

#[test]
fn a_path_follows_the_environment_and_a_url_stays() {
    let config = config(json!({ "bookmarks": [
        { "id": "checkout", "label": "Checkout", "path": "/checkout" },
        { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" }
    ] }));
    assert_eq!(
        resolve(&config, Some("local")),
        [
            "http://localhost:5173/checkout",
            "https://docs.example.com/guide"
        ]
    );
    assert_eq!(
        resolve(&config, Some("staging")),
        [
            "https://staging.example.com/checkout",
            "https://docs.example.com/guide"
        ]
    );
}

#[test]
fn without_an_environment_a_path_lies_under_the_start_url_or_the_first_environment() {
    let mut config = config(json!({
        "startUrl": "http://127.0.0.1:8080/app/index.html?x=1",
        "bookmarks": [{ "id": "a", "label": "A", "path": "/a?q=1#top" }]
    }));
    assert_eq!(resolve(&config, None), ["http://127.0.0.1:8080/a?q=1#top"]);
    assert_eq!(
        resolve(&config, Some("unknown")),
        ["http://127.0.0.1:8080/a?q=1#top"]
    );
    config.start_url = None;
    assert_eq!(resolve(&config, None), ["http://localhost:5173/a?q=1#top"]);
    config.environments.clear();
    assert!(resolve(&config, None).is_empty(), "nothing to lie under");
    assert!(resolved(&config, None).is_empty());
}

#[test]
fn resolved_carries_label_group_and_url_in_config_order() {
    let config = config(json!({ "bookmarks": [
        { "id": "home", "label": "Home", "path": "/" },
        { "id": "pdp", "label": "Product", "path": "/products/42", "group": "Shop",
          "shortcut": "Alt+F7" }
    ] }));
    let list = resolved(&config, Some("staging"));
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].url, "https://staging.example.com/");
    assert_eq!(list[1].id, "pdp");
    assert_eq!(list[1].label, "Product");
    assert_eq!(list[1].group.as_deref(), Some("Shop"));
    assert_eq!(list[1].url, "https://staging.example.com/products/42");
}

#[test]
fn exactly_one_target_and_safe_forms_are_required() {
    let config = config(json!({ "bookmarks": [
        { "id": "both", "label": "Both", "path": "/a", "url": "https://x.example" },
        { "id": "none", "label": "None" },
        { "id": "rel", "label": "Relative", "path": "checkout" },
        { "id": "js", "label": "Script", "url": "javascript:alert(1)" },
        { "id": "file", "label": "File", "url": "file:///C:/Windows" },
        { "id": "nohost", "label": "No host", "url": "https://" },
        { "id": "space", "label": "Space", "path": "/a b" },
        { "id": "", "label": "No id", "path": "/" },
        { "id": "nolabel", "label": " ", "path": "/" },
        { "id": "ok", "label": "Fine", "url": "HTTPS://Example.com/x" },
        { "id": "ok", "label": "Again", "path": "/" }
    ] }));
    assert_eq!(
        keys(&check_bookmarks(&config)),
        [
            ("/bookmarks/0", TARGET_KEY),
            ("/bookmarks/1", TARGET_KEY),
            ("/bookmarks/2/path", PATH_KEY),
            ("/bookmarks/3/url", URL_KEY),
            ("/bookmarks/4/url", URL_KEY),
            ("/bookmarks/5/url", URL_KEY),
            ("/bookmarks/6/path", PATH_KEY),
            ("/bookmarks/7/id", ID_KEY),
            ("/bookmarks/8/label", LABEL_KEY),
            ("/bookmarks/10/id", "config.error.duplicate_id"),
        ]
    );
    let both = &check_bookmarks(&config)[0];
    assert!(both.message.contains("either `path` or `url`"), "{both:?}");
}

#[test]
fn the_config_check_reports_bookmark_problems() {
    let config = config(json!({ "bookmarks": [
        { "id": "both", "label": "Both", "path": "/a", "url": "https://x.example" }
    ] }));
    assert_eq!(keys(&crate::check(&config)), [("/bookmarks/0", TARGET_KEY)]);
}

#[test]
fn loading_drops_a_bad_bookmark_with_a_warning_and_keeps_the_rest() {
    let mut merged = json!({
        "version": 1,
        "bookmarks": [
            { "id": "good", "label": "Good", "path": "/good" },
            { "id": "both", "label": "Both", "path": "/a", "url": "https://x.example" },
            { "id": 7 },
            "text",
            { "id": "good", "label": "Twice", "path": "/twice" },
            { "id": "last", "label": "Last", "url": "https://example.com" }
        ]
    });
    let warnings = strip_invalid(&mut merged);
    assert_eq!(
        keys(&warnings),
        [
            ("/bookmarks/1", IGNORED_KEY),
            ("/bookmarks/2", IGNORED_KEY),
            ("/bookmarks/3", IGNORED_KEY),
            ("/bookmarks/4", IGNORED_KEY),
        ]
    );
    assert!(
        warnings[0].message.contains("either `path` or `url`"),
        "{:?}",
        warnings[0]
    );
    let ids: Vec<&str> = merged["bookmarks"]
        .as_array()
        .expect("still a list")
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert_eq!(ids, ["good", "last"]);
    let config = crate::validate::validate(merged).expect("the rest stays valid");
    assert_eq!(config.bookmarks.len(), 2);
}

#[test]
fn loading_leaves_a_config_without_bookmarks_alone() {
    let mut merged = json!({ "version": 1 });
    assert!(strip_invalid(&mut merged).is_empty());
    assert_eq!(merged, json!({ "version": 1 }));
    let mut not_a_list = json!({ "version": 1, "bookmarks": {} });
    assert!(strip_invalid(&mut not_a_list).is_empty());
    assert!(
        crate::validate::validate(not_a_list).is_err(),
        "the type check says so"
    );
}

#[test]
fn a_page_under_the_environment_is_suggested_as_a_path() {
    assert_eq!(
        suggest_target(
            "http://localhost:5173/products/42?x=1#a",
            Some("http://localhost:5173")
        ),
        BookmarkTarget::Path("/products/42?x=1#a".to_owned())
    );
    assert_eq!(
        suggest_target(
            "https://staging.example.com",
            Some("https://staging.example.com/")
        ),
        BookmarkTarget::Path("/".to_owned())
    );
    assert_eq!(
        suggest_target("http://localhost:51730/x", Some("http://localhost:5173")),
        BookmarkTarget::Url("http://localhost:51730/x".to_owned()),
        "another port is another origin"
    );
    assert_eq!(
        suggest_target("https://docs.example.com/a", Some("http://localhost:5173")),
        BookmarkTarget::Url("https://docs.example.com/a".to_owned())
    );
    assert_eq!(
        suggest_target("https://docs.example.com/a", None),
        BookmarkTarget::Url("https://docs.example.com/a".to_owned())
    );
}

#[test]
fn a_new_id_comes_from_the_label_and_is_unique() {
    assert_eq!(new_id("Product Detail", &[]), "product-detail");
    assert_eq!(new_id("  \u{dc}ber / Checkout!! ", &[]), "ber-checkout");
    assert_eq!(new_id("***", &[]), "bookmark");
    assert_eq!(
        new_id("Home", &["home".to_owned(), "home-2".to_owned()]),
        "home-3"
    );
}

#[test]
fn a_target_is_one_of_path_or_url_in_the_document() {
    let mut bookmark = path_bookmark("a", "/a");
    assert_eq!(
        bookmark.target(),
        Some(BookmarkTarget::Path("/a".to_owned()))
    );
    bookmark.url = Some("https://x.example".to_owned());
    assert_eq!(bookmark.target(), None);
    bookmark.path = None;
    assert_eq!(
        bookmark.target(),
        Some(BookmarkTarget::Url("https://x.example".to_owned()))
    );
}

#[test]
fn the_schema_says_exactly_one_of_path_and_url() {
    let schema = serde_json::to_value(schemars::schema_for!(Bookmark)).expect("schema");
    assert_eq!(
        schema["oneOf"],
        json!([{ "required": ["path"] }, { "required": ["url"] }])
    );
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(required, ["id", "label"]);
}

fn project_with(project: &Value, local: Option<&Value>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    crate::write_config_file(dir.path(), crate::ConfigFile::Project, project).expect("project");
    if let Some(local) = local {
        crate::write_config_file(dir.path(), crate::ConfigFile::Local, local).expect("local");
    }
    dir
}

#[test]
fn the_local_file_adds_own_bookmarks_and_changes_the_teams_by_id() {
    let project = json!({ "version": 1, "bookmarks": [
        { "id": "home", "label": "Home", "path": "/" },
        { "id": "checkout", "label": "Checkout", "path": "/checkout" }
    ] });
    let local = json!({ "bookmarks": [
        { "id": "checkout", "label": "My checkout" },
        { "id": "mine", "label": "Mine", "url": "http://localhost:6006", "group": "Storybook" }
    ] });
    let dir = project_with(&project, Some(&local));
    let loaded = crate::load_project(dir.path()).expect("loads");
    let summary: Vec<(&str, &str)> = loaded
        .config
        .bookmarks
        .iter()
        .map(|b| (b.id.as_str(), b.label.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            ("home", "Home"),
            ("checkout", "My checkout"),
            ("mine", "Mine")
        ]
    );
    assert_eq!(
        loaded.config.bookmarks[1].path.as_deref(),
        Some("/checkout"),
        "untouched fields stay"
    );
    assert_eq!(
        loaded.origins["bookmarks.checkout.label"],
        crate::Origin::Local
    );
    assert_eq!(
        loaded.origins["bookmarks.checkout.path"],
        crate::Origin::Project
    );
    assert_eq!(loaded.origins["bookmarks.mine.url"], crate::Origin::Local);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
}

#[test]
fn a_bookmark_with_path_and_url_is_refused_on_load_and_the_config_stays_valid() {
    let project = json!({ "version": 1, "startUrl": "http://localhost:5173", "bookmarks": [
        { "id": "home", "label": "Home", "path": "/" }
    ] });
    let local = json!({ "bookmarks": [
        { "id": "home", "url": "https://elsewhere.example" }
    ] });
    let dir = project_with(&project, Some(&local));
    let loaded = crate::load_project(dir.path()).expect("the config stays valid");
    assert!(loaded.config.bookmarks.is_empty());
    assert_eq!(keys(&loaded.warnings), [("/bookmarks/0", IGNORED_KEY)]);
    assert!(loaded.warnings[0].message.contains("`home`"));
    assert_eq!(
        loaded.config.start_url.as_deref(),
        Some("http://localhost:5173")
    );
}

/// A path bookmark lies under the environment's URL; when that is no `http`/`https` URL (a
/// config the check never saw), the bookmark is left out rather than opened as `file:` or
/// `data:` (charge-4 review, finding 2).
#[test]
fn a_path_under_a_base_that_is_no_web_url_is_left_out() {
    let mut config = config(json!({ "bookmarks": [
        { "id": "home", "label": "Home", "path": "/Users/me" },
        { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" }
    ] }));
    config.environments[0].url = "file:///C:".to_owned();
    config.environments[1].url = "data:text/html,x".to_owned();
    assert_eq!(
        resolve(&config, Some("local")),
        ["https://docs.example.com/guide"]
    );
    assert_eq!(
        resolve(&config, Some("staging")),
        ["https://docs.example.com/guide"]
    );
    assert_eq!(config.bookmarks[0].url_with(Some("file:///C:")), None);
    // An absolute `url` that slipped past the check is refused the same way.
    config.bookmarks[1].url = Some("file:///C:/secret".to_owned());
    assert!(resolve(&config, Some("local")).is_empty());
}

/// RB-080-48: a path that a browser could read as another host (`//host`, `/\host`) or that
/// carries a scheme is a config problem, is dropped on load and never becomes a URL.
#[test]
fn a_path_that_could_name_another_host_is_a_config_problem_and_never_opens() {
    let bad = [
        "//evil.example",
        "//evil.example/checkout",
        "/\\evil.example",
        "https://evil.example/x",
        "https:evil.example",
        "\\\\evil.example",
    ];
    for path in bad {
        let config = config(json!({ "bookmarks": [{ "id": "x", "label": "X", "path": path }] }));
        assert_eq!(
            keys(&check_bookmarks(&config)),
            [("/bookmarks/0/path", PATH_KEY)],
            "{path}"
        );
        for base in ["http://localhost:5173", "https://staging.example.com/"] {
            assert_eq!(
                config.bookmarks[0].url_with(Some(base)),
                None,
                "{path} under {base}"
            );
        }
        assert!(resolve(&config, Some("staging")).is_empty(), "{path}");
        let mut merged = json!({ "version": 1, "bookmarks": [
            { "id": "x", "label": "X", "path": path },
            { "id": "ok", "label": "Fine", "path": "/redirect?to=https://a.example//b#c" }
        ] });
        assert_eq!(
            keys(&strip_invalid(&mut merged)),
            [("/bookmarks/0", IGNORED_KEY)]
        );
        assert_eq!(merged["bookmarks"].as_array().map(Vec::len), Some(1));
    }
}

/// RB-080-48: whatever the path, the URL a relative bookmark opens has the host of its base.
#[test]
fn a_relative_bookmark_keeps_the_origin_of_its_base() {
    let bookmark = path_bookmark("a", "/a//b?next=//evil.example#@evil.example");
    assert_eq!(
        bookmark.url_with(Some("https://staging.example.com/")),
        Some("https://staging.example.com/a//b?next=//evil.example#@evil.example".to_owned())
    );
    assert_eq!(
        path_bookmark("a", "/x").url_with(Some("https://shop.example.com/app/")),
        Some("https://shop.example.com/app/x".to_owned()),
        "a base with a path keeps it"
    );
}
