use super::*;

const HEAD: &str = r#"<!doctype html><html lang=" de "><head>
<meta charset="utf-8"><title>  Shop   start </title>
<meta name="description" content="Fine things">
<meta name="ROBOTS" content="NoIndex, follow">
<meta name="viewport" content="width=device-width">
<meta property="og:title" content="One"><meta property="og:title" content="Two">
<meta name="twitter:image:alt" content="A cat">
<link rel="alternate canonical" href=" /a ">
<script type="application/ld+json">{"@type":"Thing"}</script>
</head><body><h1>Hi <b>there</b></h1><h3>Deep</h3>
<a href="/x#top">x</a><a href="mailto:a@b.test">m</a><a rel="nofollow" href="/n">n</a>
</body></html>"#;

#[test]
fn the_head_facts_are_read() {
    let html = Html::parse_document(HEAD);
    let facts = facts(&html);
    assert_eq!(facts.titles, ["Shop start"]);
    assert_eq!(facts.descriptions, ["Fine things"]);
    assert_eq!(facts.robots, ["noindex, follow"]);
    assert_eq!(facts.canonicals, ["/a"]);
    assert_eq!(facts.lang.as_deref(), Some("de"));
    assert_eq!(facts.viewport.as_deref(), Some("width=device-width"));
    assert!(facts.charset);
    assert_eq!(facts.social["og:title"], "One");
    assert_eq!(facts.social["twitter:image:alt"], "A cat");
    assert_eq!(facts.json_ld.len(), 1);
    assert_eq!(facts.json_ld[0].path, "html > head > script");
    let levels: Vec<u8> = facts.headings.iter().map(|h| h.level).collect();
    assert_eq!(levels, [1, 3]);
    assert_eq!(facts.headings[0].text, "Hi there");
}

#[test]
fn an_empty_document_has_no_facts() {
    let facts = facts(&Html::parse_document(""));
    assert!(facts.titles.is_empty());
    assert!(facts.lang.is_none());
    assert!(!facts.charset);
}

#[test]
fn noindex_is_read_from_meta_and_header_values() {
    assert!(says_noindex("noindex, follow"));
    assert!(says_noindex("NONE"));
    assert!(says_noindex("googlebot: noindex"));
    assert!(!says_noindex("otherbot: noindex"));
    assert!(!says_noindex("index, follow"));
    assert!(!says_noindex("unavailable_after: 2030-01-01"));
    assert!(says_noindex("max-image-preview:large, noindex"));
}

#[test]
fn links_resolve_and_leave_out_other_schemes_and_nofollow() {
    let html = Html::parse_document(HEAD);
    let base = Url::parse("https://shop.test/de/").expect("base");
    let found: Vec<String> = links(&html, &base)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(found, ["https://shop.test/x"]);
}
