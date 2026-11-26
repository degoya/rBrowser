use serde_json::json;

use super::*;
use crate::sitemap::tool::MAX_PAGES;

#[test]
fn the_arguments_read_and_none_is_the_start_url() {
    assert_eq!(
        DiscoverArgs::parse(&json!(null)),
        Ok(DiscoverArgs::default())
    );
    let args = DiscoverArgs::parse(&json!({
        "url": "https://a.test/",
        "include": ["/de/**"],
        "depth": 2
    }))
    .expect("reads");
    assert_eq!(args.url.as_deref(), Some("https://a.test/"));
    assert_eq!(args.depth, Some(2));
}

#[test]
fn out_of_bounds_arguments_are_refused() {
    for value in [
        json!({ "depth": 0 }),
        json!({ "depth": MAX_DEPTH + 1 }),
        json!({ "max": MAX_PAGES + 1 }),
        json!({ "url": "" }),
        json!({ "url": "https://a.test/", "bookmarks": true }),
        json!({ "target": "all" }),
    ] {
        assert!(DiscoverArgs::parse(&value).is_err(), "{value}");
    }
}

#[test]
fn a_discovery_takes_every_page_unless_a_max_is_named() {
    assert_eq!(DiscoverArgs::default().source_args().max, Some(0));
    let capped = DiscoverArgs {
        max: Some(20),
        ..DiscoverArgs::default()
    };
    assert_eq!(capped.source_args().max, Some(20));
}

#[test]
fn a_capture_with_urls_discovers_its_own_source() {
    let capture = CaptureArgs {
        url: Some("sitemap.txt".to_owned()),
        include: Some(vec!["/de/**".to_owned()]),
        env: Some("local".to_owned()),
        ..CaptureArgs::default()
    };
    let discover = DiscoverArgs::of_capture(&capture);
    assert_eq!(discover.url.as_deref(), Some("sitemap.txt"));
    assert_eq!(discover.include, capture.include);
}

#[test]
fn the_tool_has_its_name() {
    assert_eq!(DISCOVER_TOOL, "sitemap_discover");
}
