use crate::testing::{check, serve, short};

const URL: &str = "https://shop.test/";

fn found(head: &str, group: &str) -> Vec<String> {
    let mut host = serve(URL, &format!("<html><head>{head}</head></html>"), &[]);
    short(&check(&mut host, URL, None), group)
}

#[test]
fn the_open_graph_basics_are_each_named() {
    let og = found(
        r#"<meta property="og:title" content="T"><meta property="og:url" content="u">"#,
        "social.og",
    );
    assert_eq!(
        og,
        [
            "social.og_missing social.og_missing",
            "social.og_missing social.og_missing"
        ]
    );
}

#[test]
fn a_share_image_needs_its_alt_text() {
    let head = r#"<meta property="og:image" content="a.jpg"><meta name="twitter:image" content="b.jpg"><meta name="twitter:image:alt" content="A cat">"#;
    assert_eq!(
        found(head, "images"),
        ["images.social_alt_missing images.social_alt_missing"]
    );
}

#[test]
fn broken_json_ld_is_reported_with_its_place() {
    let head = r#"<script type="application/ld+json">{"@type": "Thing"}</script>
<script type="application/ld+json">{"@type": "Thing",}</script>"#;
    let mut host = serve(URL, &format!("<html><head>{head}</head></html>"), &[]);
    let findings = check(&mut host, URL, None);
    let broken: Vec<_> = findings
        .iter()
        .filter(|finding| finding.rule == "structured.jsonld_invalid")
        .collect();
    assert_eq!(broken.len(), 1);
    assert_eq!(
        broken[0].path.as_deref(),
        Some("html > head > script:nth-of-type(2)")
    );
}

#[test]
fn a_relative_share_image_and_a_missing_card_type_are_named() {
    let head = r#"<meta property="og:image" content="/img/share.png">"#;
    assert_eq!(
        found(head, "social.image"),
        ["social.image_relative social.image_relative"]
    );
    assert_eq!(
        found(head, "social.twitter"),
        ["social.twitter_card social.twitter_card_missing"]
    );
    let bad = r#"<meta name="twitter:card" content="large">"#;
    assert_eq!(
        found(bad, "social.twitter"),
        ["social.twitter_card social.twitter_card_invalid"]
    );
    let good = r#"<meta property="og:image" content="https://cdn.test/a.png">
<meta name="twitter:card" content="Summary_Large_Image">"#;
    assert!(found(good, "social.image").is_empty());
    assert!(found(good, "social.twitter").is_empty());
}

#[test]
fn og_url_has_to_name_the_canonical() {
    let same =
        r#"<meta property="og:url" content="/"><link rel="canonical" href="https://shop.test/">"#;
    assert!(found(same, "social.url").is_empty());
    let other = r#"<meta property="og:url" content="https://shop.test/?ref=og">
<link rel="canonical" href="https://shop.test/">"#;
    assert_eq!(
        found(other, "social.url"),
        ["social.url_canonical social.url_canonical"]
    );
}
