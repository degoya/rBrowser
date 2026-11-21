use scraper::Html;

use super::*;
use crate::page;

fn facts(head: &str) -> Facts {
    page::facts(&Html::parse_document(&format!(
        "<html><head>{head}</head><body></body></html>"
    )))
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("a test URL")
}

#[test]
fn the_title_and_description_come_from_the_head_before_open_graph() {
    let serp = serp(
        &facts(
            r#"<title>Shoes for every day</title>
<meta name="description" content="Light shoes, made to last.">
<meta property="og:title" content="OG title"><meta property="og:description" content="OG text">"#,
        ),
        &url("https://www.shop.test/shoes/"),
        &[],
    );
    assert_eq!(
        serp.title,
        Some(Text {
            text: "Shoes for every day".to_owned(),
            source: "title".to_owned()
        })
    );
    assert_eq!(
        serp.description,
        Some(Text {
            text: "Light shoes, made to last.".to_owned(),
            source: "description".to_owned()
        })
    );
    assert!(!serp.noindex);
}

#[test]
fn open_graph_stands_in_for_a_missing_or_empty_title_and_description() {
    let serp = serp(
        &facts(
            r#"<title>  </title><meta name="description" content="">
<meta property="og:title" content="OG title"><meta property="og:description" content="OG text">"#,
        ),
        &url("https://shop.test/"),
        &[],
    );
    assert_eq!(
        serp.title.map(|t| (t.text, t.source)),
        Some(("OG title".to_owned(), "og:title".to_owned()))
    );
    assert_eq!(
        serp.description.map(|t| (t.text, t.source)),
        Some(("OG text".to_owned(), "og:description".to_owned()))
    );
}

#[test]
fn without_any_description_the_preview_has_none() {
    let serp = serp(&facts("<title>T</title>"), &url("https://shop.test/"), &[]);
    assert_eq!(serp.description, None);
    let serp = super::serp(&facts(""), &url("https://shop.test/"), &[]);
    assert_eq!(serp.title, None);
}

#[test]
fn the_site_name_is_og_site_name_else_the_domain_without_www() {
    let named = serp(
        &facts(r#"<meta property="og:site_name" content="Shoe Shop">"#),
        &url("https://www.shop.test/"),
        &[],
    );
    assert_eq!(
        (
            named.site_name.text.as_str(),
            named.site_name.source.as_str()
        ),
        ("Shoe Shop", "og:site_name")
    );
    let bare = serp(&facts(""), &url("https://www.shop.test:8443/"), &[]);
    assert_eq!(
        (bare.site_name.text.as_str(), bare.site_name.source.as_str()),
        ("shop.test", "domain")
    );
}

#[test]
fn the_breadcrumb_is_the_host_and_the_decoded_path_without_the_query() {
    let crumbs = breadcrumb(&url(
        "https://www.shop.test/schuhe/gr%C3%BC%C3%9Fe-42/?color=red&size=42",
    ));
    assert_eq!(crumbs.host, "www.shop.test");
    assert_eq!(crumbs.segments, ["schuhe", "gr\u{fc}\u{df}e-42"]);
    assert!(breadcrumb(&url("https://shop.test/")).segments.is_empty());
    assert_eq!(
        breadcrumb(&url("https://shop.test//a//b")).segments,
        ["a", "b"]
    );
}

#[test]
fn a_broken_escape_is_kept_as_written() {
    assert_eq!(decode("100%"), "100%");
    assert_eq!(decode("a%zzb"), "a%zzb");
    assert_eq!(decode("a%20b"), "a b");
    // Bytes that are no UTF-8 become the replacement character, never a panic.
    assert_eq!(decode("%FF"), "\u{fffd}");
}

#[test]
fn noindex_comes_from_the_robots_meta_or_the_header() {
    let meta = serp(
        &facts(r#"<meta name="robots" content="noindex, follow">"#),
        &url("https://shop.test/"),
        &[],
    );
    assert!(meta.noindex);
    let header = serp(&facts(""), &url("https://shop.test/"), &["noindex"]);
    assert!(header.noindex);
    let other_bot = serp(
        &facts(""),
        &url("https://shop.test/"),
        &["otherbot: noindex"],
    );
    assert!(!other_bot.noindex);
}

#[test]
fn the_limits_are_the_documented_pixel_widths_and_lines() {
    let limits = &serp(&facts(""), &url("https://shop.test/"), &[]).limits;
    assert_eq!(limits.confidence, Confidence::Unverified);
    assert_eq!(
        (
            limits.desktop.width,
            limits.desktop.title_lines,
            limits.desktop.description_lines
        ),
        (600, 1, 2)
    );
    assert_eq!(
        (
            limits.desktop.title_font_px,
            limits.desktop.description_font_px
        ),
        (20, 14)
    );
    assert_eq!(
        (limits.mobile.title_lines, limits.mobile.description_lines),
        (2, 3)
    );
    assert!(limits.mobile.width < 360, "a 360 px phone less its margins");
}

#[test]
fn the_serp_serializes_in_camel_case() {
    let value = serde_json::to_value(serp(
        &facts("<title>T</title>"),
        &url("https://shop.test/a"),
        &[],
    ))
    .expect("json");
    assert_eq!(value["siteName"]["text"], "shop.test");
    assert_eq!(value["breadcrumb"]["segments"][0], "a");
    assert_eq!(value["limits"]["desktop"]["titleFontPx"], 20);
    assert_eq!(value["limits"]["confidence"], "unverified");
    assert_eq!(value["url"], "https://shop.test/a");
}
