use rb_plugin_api::ViewInfo;
use serde_json::json;

use super::*;
use crate::config::Settings;
use crate::feed::{self, FEED_KEY};
use crate::report::build;

#[test]
fn texts_come_from_the_catalogue_and_fall_back_to_the_key() {
    let texts = Texts::load();
    assert_eq!(texts.t("html.title", &[]), "Error report");
    assert_eq!(
        texts.t("html.count", &[("count", "3".to_owned())]),
        "3 times"
    );
    assert_eq!(texts.t("html.nope", &[]), "html.nope");
}

#[test]
fn everything_is_escaped() {
    assert_eq!(
        escape("<script>alert('x') & \"y\"</script>"),
        "&lt;script&gt;alert(&#39;x&#39;) &amp; &quot;y&quot;&lt;/script&gt;"
    );
}

#[test]
fn utc_dates() {
    assert_eq!(utc(0), "1970-01-01 00:00");
    assert_eq!(utc(1_731_300_000), "2024-11-11 04:40");
    assert_eq!(utc(951_782_400), "2000-02-29 00:00");
}

#[test]
fn the_page_lists_the_categories_with_their_entries_and_no_markup_from_the_page() {
    let views = vec![
        ViewInfo {
            id: 1,
            label: "Mobile".to_owned(),
            breakpoint: "mobile".to_owned(),
            engine: "chromium".to_owned(),
            ..ViewInfo::default()
        },
        ViewInfo {
            id: 2,
            label: "Desktop".to_owned(),
            breakpoint: "desktop".to_owned(),
            engine: "chromium".to_owned(),
            ..ViewInfo::default()
        },
    ];
    let feed = feed::read(&json!({ FEED_KEY: { "views": [
        { "view": 1, "errors": [{ "text": "Uncaught Error: <img src=x onerror=alert(1)>\n    at f (https://a.test/a.js:1:2)", "time": 1.0 }],
          "network": [{ "url": "https://a.test/m.png", "status": 404, "resourceType": "Image", "time": 1.0 }] },
        { "view": 2, "errors": [{ "text": "Uncaught Error: <img src=x onerror=alert(1)>\n    at f (https://a.test/a.js:1:2)", "time": 1.0 }] }
    ] } }))
    .expect("a capture");
    let report = build(&feed, &views, &Settings::default(), 0.0);
    let page = render(&report, 1_731_300_000);
    assert!(page.starts_with("<!doctype html>"));
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains("<h2>Uncaught JavaScript exception (1)</h2>"));
    assert!(page.contains("<h2>Resource not found (404, 410) (1)</h2>"));
    assert!(page.contains("Only in views: mobile"));
    assert!(page.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(!page.contains("<img"));
    assert!(page.contains("Generated 2024-11-11 04:40 UTC over 2 views."));
    let empty = render(
        &build(&feed::Feed::default(), &[], &Settings::default(), 0.0),
        0,
    );
    assert!(empty.contains("No errors."));
}
