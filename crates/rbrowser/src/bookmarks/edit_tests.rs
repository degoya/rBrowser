use serde_json::json;

use super::*;

fn page(url: &str, title: &str) -> Page {
    Page {
        url: url.to_owned(),
        title: title.to_owned(),
    }
}

fn args(value: Value) -> AddArgs {
    serde_json::from_value(value).expect("arguments")
}

fn config() -> rb_config::ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "environments": [
            { "id": "local", "url": "http://localhost:5173" },
            { "id": "staging", "url": "https://staging.example.com/" }
        ]
    }))
    .expect("a config")
}

/// The path of a page under `local` (in effect) or `staging`.
fn path_of(url: &str) -> Option<String> {
    rb_config::bookmark_paths::path_in(&config(), Some("local"), url)
}

/// `bookmarks.followEnvironment` on, `local` in effect.
const FOLLOWS: Follow<'static> = Follow {
    default: true,
    path_of: &path_of,
    base: Some("http://localhost:5173"),
};
/// The same with `bookmarks.followEnvironment` off.
const ABSOLUTE: Follow<'static> = Follow {
    default: false,
    path_of: &path_of,
    base: Some("http://localhost:5173"),
};

#[test]
fn no_label_path_or_url_asks_the_shell() {
    assert!(args(json!({})).is_empty());
    assert!(args(json!({ "scope": "local" })).is_empty());
    assert!(!args(json!({ "label": "Home" })).is_empty());
    assert!(serde_json::from_value::<AddArgs>(json!({ "target": 1 })).is_err());
}

#[test]
fn the_current_page_under_the_environment_becomes_a_path_named_by_its_title() {
    let current = page("http://localhost:5173/products/42?tab=2", "Product 42");
    let bookmark = bookmark_of(
        &args(json!({ "group": "Shop" })),
        Some(&current),
        &FOLLOWS,
        &[],
    )
    .expect("a bookmark");
    assert_eq!(bookmark.id, "product-42");
    assert_eq!(bookmark.label, "Product 42");
    assert_eq!(bookmark.path.as_deref(), Some("/products/42?tab=2"));
    assert_eq!(bookmark.url, None);
    assert_eq!(bookmark.group.as_deref(), Some("Shop"));
}

#[test]
fn a_page_of_another_origin_stays_absolute() {
    let current = page("https://docs.example.com/guide", "");
    let bookmark =
        bookmark_of(&args(json!({})), Some(&current), &FOLLOWS, &[]).expect("a bookmark");
    assert_eq!(
        bookmark.url.as_deref(),
        Some("https://docs.example.com/guide")
    );
    assert_eq!(bookmark.label, "https://docs.example.com/guide");
}

/// RB-080-48: the page of any environment is stored as its path, not only one of the
/// environment in effect; with `bookmarks.followEnvironment` off it stays the whole URL.
#[test]
fn a_page_of_any_environment_becomes_a_path_unless_new_bookmarks_stay_absolute() {
    let current = page("https://staging.example.com/shop/cart?x=1#top", "Cart");
    let bookmark =
        bookmark_of(&args(json!({})), Some(&current), &FOLLOWS, &[]).expect("a bookmark");
    assert_eq!(bookmark.path.as_deref(), Some("/shop/cart?x=1#top"));
    assert_eq!(bookmark.url, None);
    let bookmark =
        bookmark_of(&args(json!({})), Some(&current), &ABSOLUTE, &[]).expect("a bookmark");
    assert_eq!(bookmark.path, None);
    assert_eq!(
        bookmark.url.as_deref(),
        Some("https://staging.example.com/shop/cart?x=1#top")
    );
    // A target the caller names is taken as named, whatever the setting.
    let named = args(json!({ "label": "Cart", "url": "http://localhost:5173/shop/cart" }));
    let bookmark = bookmark_of(&named, None, &FOLLOWS, &[]).expect("a bookmark");
    assert_eq!(
        bookmark.url.as_deref(),
        Some("http://localhost:5173/shop/cart")
    );
    let named = args(json!({ "label": "Cart", "path": "/shop/cart" }));
    let bookmark = bookmark_of(&named, None, &ABSOLUTE, &[]).expect("a bookmark");
    assert_eq!(bookmark.path.as_deref(), Some("/shop/cart"));
}

/// Review point 2: the dialog sends what was typed and its switch; the core alone decides
/// the form.
#[test]
fn the_dialogs_switch_decides_the_form_of_a_typed_target() {
    let follow = |value: Value| bookmark_of(&args(value), None, &ABSOLUTE, &[]);
    let on =
        follow(json!({ "label": "x", "url": "https://staging.example.com/a?b", "follow": true }))
            .expect("a bookmark");
    assert_eq!((on.path.as_deref(), on.url), (Some("/a?b"), None));
    let foreign =
        follow(json!({ "label": "x", "url": "https://docs.example.com/a", "follow": true }))
            .expect("a bookmark");
    assert_eq!(
        (foreign.path, foreign.url.as_deref()),
        (None, Some("https://docs.example.com/a")),
        "nothing to follow: it stays the URL"
    );
    let off = follow(json!({ "label": "x", "path": "/a?b", "follow": false })).expect("a bookmark");
    assert_eq!(
        (off.path, off.url.as_deref()),
        (None, Some("http://localhost:5173/a?b")),
        "the URL in the environment in effect"
    );
    assert!(follow(json!({ "label": "x", "path": "//evil.example", "follow": false })).is_err());
    let no_base = Follow {
        default: true,
        path_of: &path_of,
        base: None,
    };
    let lost = args(json!({ "label": "x", "path": "/a", "follow": false }));
    assert!(bookmark_of(&lost, None, &no_base, &[]).is_err());
    // The page of the active view: the switch wins over the setting, both ways.
    let current = page("http://localhost:5173/shop", "Shop");
    let on = bookmark_of(
        &args(json!({ "label": "x", "follow": true })),
        Some(&current),
        &ABSOLUTE,
        &[],
    )
    .expect("a bookmark");
    assert_eq!(on.path.as_deref(), Some("/shop"));
    let off = bookmark_of(
        &args(json!({ "label": "x", "follow": false })),
        Some(&current),
        &FOLLOWS,
        &[],
    )
    .expect("a bookmark");
    assert_eq!(off.url.as_deref(), Some("http://localhost:5173/shop"));
}

#[test]
fn given_fields_win_and_empty_ones_are_left_out() {
    let bookmark = bookmark_of(
        &args(json!({ "label": " Checkout ", "path": "/checkout", "group": "", "shortcut": " " })),
        None,
        &FOLLOWS,
        &["checkout".to_owned()],
    )
    .expect("a bookmark");
    assert_eq!(bookmark.id, "checkout-2");
    assert_eq!(bookmark.label, "Checkout");
    assert_eq!(bookmark.path.as_deref(), Some("/checkout"));
    assert_eq!((bookmark.group, bookmark.shortcut), (None, None));
}

#[test]
fn what_cannot_be_a_bookmark_is_refused() {
    let both = args(json!({ "label": "x", "path": "/a", "url": "https://x.example" }));
    assert!(bookmark_of(&both, None, &FOLLOWS, &[]).is_err());
    let no_page = args(json!({ "label": "x" }));
    assert!(bookmark_of(&no_page, None, &FOLLOWS, &[]).is_err());
    let blank = page("about:blank", "");
    assert!(bookmark_of(&no_page, Some(&blank), &FOLLOWS, &[]).is_err());
    let taken = args(json!({ "id": "home", "path": "/" }));
    assert!(bookmark_of(&taken, None, &FOLLOWS, &["home".to_owned()]).is_err());
}

#[test]
fn append_adds_to_the_end_and_keeps_every_other_key() {
    let mut document =
        json!({ "version": 1, "bookmarks": [{ "id": "a", "label": "A", "path": "/" }] });
    let bookmark = bookmark_of(
        &args(json!({ "label": "B", "url": "https://b.example" })),
        None,
        &FOLLOWS,
        &[],
    )
    .expect("a bookmark");
    append(&mut document, &bookmark).expect("appended");
    assert_eq!(
        document,
        json!({ "version": 1, "bookmarks": [
            { "id": "a", "label": "A", "path": "/" },
            { "id": "b", "label": "B", "url": "https://b.example" }
        ] })
    );
    let mut empty = json!({});
    append(&mut empty, &bookmark).expect("appended");
    assert_eq!(empty["bookmarks"][0]["id"], "b");
    assert!(append(&mut json!({ "bookmarks": {} }), &bookmark).is_err());
    assert!(append(&mut json!([]), &bookmark).is_err());
}

#[test]
fn the_ids_in_a_document_are_listed() {
    let document = json!({ "bookmarks": [{ "id": "a" }, { "label": "no id" }, { "id": "b" }] });
    assert_eq!(ids_in(&document), ["a", "b"]);
    assert!(ids_in(&json!({})).is_empty());
}
