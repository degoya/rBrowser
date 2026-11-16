use serde_json::{Value, json};

use super::*;
use crate::ProjectConfig;
use crate::bookmarks::{Bookmark, is_bookmark_path};

fn project(value: Value) -> ProjectConfig {
    let mut value = value;
    value["version"] = json!(1);
    serde_json::from_value(value).expect("a config")
}

fn three() -> ProjectConfig {
    project(json!({ "environments": [
        { "id": "local", "url": "http://localhost:5173" },
        { "id": "staging", "url": "https://staging.example.com/" },
        { "id": "production", "url": "https://www.example.com" }
    ] }))
}

fn opens(config: &ProjectConfig, environment: Option<&str>, path: &str) -> Option<String> {
    let bookmark = Bookmark {
        id: "x".to_owned(),
        label: "X".to_owned(),
        path: Some(path.to_owned()),
        url: None,
        group: None,
        shortcut: None,
    };
    bookmark.url_with(crate::bookmarks::base_url(config, environment).as_deref())
}

#[test]
fn the_page_of_any_environment_with_the_same_base_path_has_a_path() {
    let config = three();
    for environment in [Some("local"), Some("staging"), None] {
        for url in [
            "http://localhost:5173/shop/cart?x=1#top",
            "https://staging.example.com/shop/cart?x=1#top",
            "https://www.example.com/shop/cart?x=1#top",
        ] {
            assert_eq!(
                path_in(&config, environment, url).as_deref(),
                Some("/shop/cart?x=1#top"),
                "{url} with {environment:?} in effect"
            );
        }
    }
    assert_eq!(
        path_in(&config, Some("local"), "https://staging.example.com").as_deref(),
        Some("/")
    );
    for foreign in [
        "https://docs.example.com/guide",
        "http://localhost:51730/x",
        "https://staging.example.com.evil.example/x",
        "https://staging.example.com@evil.example/x",
        "https://user@staging.example.com/x",
        "file:///C:/x",
        "https://staging.example.com//evil.example",
    ] {
        assert_eq!(path_in(&config, Some("local"), foreign), None, "{foreign}");
    }
}

/// Review point 3: the origin is compared as an origin, not as text.
#[test]
fn an_origin_matches_in_any_case_and_with_its_default_port() {
    let config = three();
    for url in [
        "https://Staging.Example.com/Shop",
        "HTTPS://staging.example.com:443/Shop",
        "https://www.example.com:443/Shop",
        "http://LOCALHOST:5173/Shop",
    ] {
        assert_eq!(
            path_in(&config, Some("local"), url).as_deref(),
            Some("/Shop"),
            "{url}"
        );
    }
    assert_eq!(
        path_in(&config, Some("local"), "https://staging.example.com:8443/x"),
        None
    );
}

/// Review point 1: the path is put after the environment in effect, so that base decides.
#[test]
fn the_stored_path_reopens_the_same_page_in_the_environment_in_effect() {
    // The start URL's origin holds the page, the environment in effect (a base path) does not.
    let app = project(json!({
        "startUrl": "http://localhost:3000/app/",
        "environments": [{ "id": "local", "url": "http://localhost:3000/app" }]
    }));
    assert_eq!(
        path_in(&app, Some("local"), "http://localhost:3000/other"),
        None,
        "/other would open /app/other"
    );
    let page = "http://localhost:3000/app/cart";
    let path = path_in(&app, Some("local"), page).expect("a path");
    assert_eq!(path, "/cart");
    assert_eq!(opens(&app, Some("local"), &path).as_deref(), Some(page));
    // Nested bases: the one in effect wins.
    let nested = project(json!({ "environments": [
        { "id": "a", "url": "https://example.com" },
        { "id": "b", "url": "https://example.com/shop" },
        { "id": "c", "url": "https://c.example.com/shop/" }
    ] }));
    let page = "https://example.com/shop/cart";
    for (environment, stored) in [("a", "/shop/cart"), ("b", "/cart")] {
        let path = path_in(&nested, Some(environment), page).expect("a path");
        assert_eq!(path, stored, "{environment}");
        assert_eq!(
            opens(&nested, Some(environment), &path).as_deref(),
            Some(page)
        );
    }
    assert_eq!(
        path_in(&nested, Some("b"), "https://example.com/other"),
        None,
        "under a only, whose base has another path"
    );
    assert_eq!(
        path_in(&nested, Some("b"), "https://c.example.com/shop/cart").as_deref(),
        Some("/cart"),
        "c has the base path of b"
    );
    assert_eq!(
        path_in(&nested, Some("a"), "https://c.example.com/shop/cart"),
        None
    );
    assert_eq!(
        path_in(&nested, Some("b"), "https://example.com/shopping"),
        None,
        "a longer word is not under /shop"
    );
}

/// Review point 2: a project with only a start URL has bookmarks that follow it.
#[test]
fn with_only_a_start_url_the_page_under_its_origin_has_a_path() {
    let config = project(json!({ "startUrl": "http://127.0.0.1:8080/app/index.html?x=1" }));
    assert_eq!(
        path_in(&config, None, "http://127.0.0.1:8080/other?y=2").as_deref(),
        Some("/other?y=2")
    );
    assert_eq!(path_in(&config, None, "https://docs.example.com/"), None);
    assert_eq!(
        path_in(&ProjectConfig::default(), None, "http://127.0.0.1/"),
        None
    );
}

/// `bookmarks.make_relative`: a URL is converted only when its path leads to the same page
/// under every base that holds it.
#[test]
fn an_existing_url_has_a_path_only_when_every_base_that_holds_it_agrees() {
    let config = three();
    assert_eq!(
        stored_path(&config, "https://staging.example.com/a?b#c").as_deref(),
        Some("/a?b#c")
    );
    assert_eq!(stored_path(&config, "https://docs.example.com/a"), None);
    let nested = config_nested();
    assert_eq!(
        stored_path(&nested, "https://example.com/shop/cart"),
        None,
        "/shop/cart under a, /cart under b"
    );
    assert_eq!(
        stored_path(&nested, "https://example.com/other"),
        None,
        "b names the host too and would open /shop/other"
    );
    let app = project(json!({
        "startUrl": "http://localhost:3000/app/",
        "environments": [{ "id": "local", "url": "http://localhost:3000/app" }]
    }));
    assert_eq!(stored_path(&app, "http://localhost:3000/app/x"), None);
    assert_eq!(
        stored_path(&app, "http://localhost:3000/x"),
        None,
        "local names the host too and would open /app/x"
    );
    let start_only = project(json!({ "startUrl": "http://localhost:3000/app/" }));
    assert_eq!(
        stored_path(&start_only, "http://localhost:3000/x").as_deref(),
        Some("/x")
    );
}

fn config_nested() -> ProjectConfig {
    project(json!({ "environments": [
        { "id": "a", "url": "https://example.com" },
        { "id": "b", "url": "https://example.com/shop" }
    ] }))
}

/// Review point 10: whatever a path holds, what opens has the origin of its base - or nothing
/// opens.
#[test]
fn no_path_leaves_the_origin_of_its_base() {
    let config = three();
    let refused = [
        "//host.example",
        "/\\host.example",
        "\\\\host.example",
        "/a\\b",
        "/..\\\\host.example",
        "/\t/host.example",
        "/\n/host.example",
        "/a b",
        "https://host.example/",
        "host.example",
        "",
    ];
    for path in refused {
        assert!(!is_bookmark_path(path), "{path:?}");
        assert_eq!(opens(&config, Some("staging"), path), None, "{path:?}");
    }
    let kept = [
        "/%2F%2Fhost.example",
        "/..//host.example",
        "/@host.example",
        "/:80@host.example/x",
        "/x?next=//host.example#@host.example",
        "/%5Chost.example",
    ];
    for path in kept {
        assert!(is_bookmark_path(path), "{path}");
        for (environment, origin) in [
            ("local", "http://localhost:5173"),
            ("staging", "https://staging.example.com"),
        ] {
            let url = opens(&config, Some(environment), path).expect("opens");
            assert_eq!(url, format!("{origin}{path}"));
            assert_eq!(
                rb_util::url::origin_of(&url).as_deref(),
                Some(origin),
                "{path}"
            );
        }
    }
    // A base that would let the path finish the host opens nothing.
    for base in [
        "https://user@",
        "https://",
        "file:///C:",
        "https://a.example?x=",
    ] {
        let bookmark: Bookmark =
            serde_json::from_value(json!({ "id": "x", "label": "X", "path": "/x" }))
                .expect("a bookmark");
        let url = bookmark.url_with(Some(base));
        assert!(
            url.is_none() || base == "https://a.example?x=",
            "{base}: {url:?}"
        );
    }
}

#[test]
fn the_shared_helpers_split_and_join_a_url_under_a_base() {
    assert_eq!(
        split("HTTPS://Example.com:443/a?b"),
        Some(("https://example.com".to_owned(), "/a?b"))
    );
    assert_eq!(split("about:blank"), None);
    assert_eq!(
        rest_under("https://example.com/app/x", "https://example.com/app/"),
        Some("/x")
    );
    assert_eq!(
        rest_under("https://example.com/app?x", "https://example.com/app"),
        Some("?x")
    );
    assert_eq!(
        rest_under("https://example.com/apple", "https://example.com/app"),
        None
    );
    assert_eq!(as_path(""), "/");
    assert_eq!(as_path("?x"), "/?x");
    assert_eq!(
        url_in("https://example.com/app/", "/x"),
        "https://example.com/app/x"
    );
}
