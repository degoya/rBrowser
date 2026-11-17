use scraper::Html;

use super::*;
use crate::page;

fn facts(head: &str) -> Facts {
    page::facts(&Html::parse_document(&format!(
        "<html><head>{head}</head></html>"
    )))
}

fn page_url() -> Url {
    Url::parse("https://shop.test/a/").expect("url")
}

#[test]
fn the_table_names_every_platform_with_its_rule_and_source() {
    let data = data();
    assert_eq!(data.version, 2);
    assert_eq!(
        platform_ids(),
        [
            "facebook", "x", "linkedin", "slack", "whatsapp", "discord", "imessage", "telegram",
            "teams", "mastodon", "bluesky", "threads", "signal"
        ]
    );
    assert_eq!(
        default_ids(),
        [
            "facebook", "x", "linkedin", "slack", "whatsapp", "discord", "imessage"
        ]
    );
    for platform in &data.platforms {
        assert!(platform.title.is_none_or(|max| max > 10), "{}", platform.id);
        // A sure rule names its source; an unverified one is marked so.
        let unverified = platform.rule == Rule::Unverified;
        assert_eq!(
            unverified,
            platform.confidence == Confidence::Unverified,
            "{}",
            platform.id
        );
        if !unverified {
            let source = platform.source.as_deref().unwrap_or_default();
            assert!(source.starts_with("https://"), "{}", platform.id);
        }
    }
}

#[test]
fn a_long_text_is_cut_with_one_ellipsis_character() {
    assert_eq!(cut("short", Some(10)), ("short".to_owned(), false));
    assert_eq!(
        cut("exactly10!", Some(10)),
        ("exactly10!".to_owned(), false)
    );
    assert_eq!(
        cut("one two three", Some(9)),
        ("one two\u{2026}".to_owned(), true)
    );
    let (text, truncated) = cut(&"\u{e4}".repeat(20), Some(5));
    assert!(truncated);
    assert_eq!(text.chars().count(), 5);
}

#[test]
fn open_graph_comes_first_and_x_prefers_its_own_tags() {
    let facts = facts(
        r#"<title>Page title</title>
<meta name="description" content="Page description">
<meta property="og:title" content="OG title">
<meta property="og:image" content="/img/share.png">
<meta name="twitter:title" content="X title">
<meta name="twitter:card" content="summary_large_image">"#,
    );
    let cards = cards(&facts, &page_url(), &[]);
    assert_eq!(cards.len(), 13);
    let facebook = &cards[0];
    let title = facebook.title.as_ref().expect("title");
    assert_eq!(
        (title.text.as_str(), title.source.as_str()),
        ("OG title", "og:title")
    );
    let description = facebook.description.as_ref().expect("description");
    assert_eq!(description.source, "description");
    assert_eq!(
        facebook.image.as_deref(),
        Some("https://shop.test/img/share.png")
    );
    let x = &cards[1];
    assert_eq!(x.title.as_ref().expect("title").source, "twitter:title");
    assert_eq!(x.layout, Layout::Large);
    assert_eq!(x.reason.code, "card_large");
    // Before the image is loaded a size rule cannot decide.
    assert!(facebook.unsure);
    assert_eq!(facebook.reason.code, "size_unknown");
    assert_eq!(x.image_source.as_deref(), Some("og:image"));
    let linkedin = &cards[2];
    assert_eq!(linkedin.description, None);
}

#[test]
fn without_tags_the_title_is_the_last_fallback_and_the_card_is_text() {
    let facts = facts("<title>Only a title</title>");
    let cards = cards(&facts, &page_url(), &["x".to_owned()]);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title.as_ref().expect("title").source, "title");
    assert_eq!(cards[0].layout, Layout::Text);
    assert_eq!(cards[0].image, None);
    assert_eq!(cards[0].description, None);
}

#[test]
fn the_domain_is_og_url_s_host() {
    let with = facts(r#"<meta property="og:url" content="https://www.shop.test/x">"#);
    assert_eq!(domain(&with, &page_url()), "www.shop.test");
    assert_eq!(domain(&facts(""), &page_url()), "shop.test");
}

#[test]
fn a_text_without_a_published_cut_stays_whole() {
    assert_eq!(cut(&"a".repeat(500), None), ("a".repeat(500), false));
}

#[test]
fn a_card_settles_its_layout_once_the_image_size_is_known() {
    let facts = facts(r#"<meta property="og:image" content="/s.png">"#);
    let mut cards = cards(&facts, &page_url(), &["facebook".to_owned()]);
    let card = &mut cards[0];
    card.settle(Some(Shape {
        width: 400,
        height: 400,
        bytes: 1,
    }));
    assert_eq!(
        (card.layout, card.alternate, card.unsure),
        (Layout::Small, Some(Layout::Large), false)
    );
    assert_eq!(card.reason.code, "below");
    let json = serde_json::to_value(&*card).expect("json");
    assert_eq!(json["layout"], "small");
    assert_eq!(json["alternate"], "large");
    assert_eq!(json["reason"]["params"]["min_width"], "600");
    assert_eq!(json["confidence"], "official");
    assert!(json.get("rule").is_none() && json.get("twitterCard").is_none());
}
