use scraper::Html;

use super::*;

fn base() -> Url {
    Url::parse("https://a.test/en/").expect("url")
}

#[test]
fn the_head_gives_its_alternates_resolved() {
    let html = Html::parse_document(
        r#"<html><head>
        <link rel="alternate" hreflang="de" href="https://a.test/de/">
        <link rel="ALTERNATE nofollow" hreflang="fr" href="/fr/">
        <link rel="alternate" type="application/rss+xml" href="/feed">
        <link rel="canonical" hreflang="it" href="/it/">
        </head></html>"#,
    );
    let found = html_alternates(&html, &base());
    let pairs: Vec<(&str, &str, Option<String>)> = found
        .iter()
        .map(|a| {
            (
                a.code.as_str(),
                a.href.as_str(),
                a.url.as_ref().map(ToString::to_string),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [
            (
                "de",
                "https://a.test/de/",
                Some("https://a.test/de/".to_owned())
            ),
            ("fr", "/fr/", Some("https://a.test/fr/".to_owned())),
        ]
    );
}

#[test]
fn link_headers_give_their_alternates_and_ignore_the_rest() {
    let values = [
        r#"<https://a.test/de/>; rel="alternate"; hreflang="de", <https://a.test/en/>; rel="canonical""#,
        r#"<https://a.test/fr/>; hreflang=fr; rel=alternate"#,
        r#"<https://a.test/x/>; rel="alternate""#,
        "garbage <without end",
    ];
    let found = header_alternates(values.into_iter(), &base());
    let pairs: Vec<(&str, &str)> = found
        .iter()
        .map(|a| (a.code.as_str(), a.href.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [("de", "https://a.test/de/"), ("fr", "https://a.test/fr/")]
    );
}

#[test]
fn absolute_is_http_or_https_only() {
    let alternate = |href: &str| Alternate::new("de", href, &base());
    assert!(alternate("https://a.test/de/").is_absolute());
    assert!(alternate("HTTP://a.test/de/").is_absolute());
    assert!(!alternate("/de/").is_absolute());
    assert!(!alternate("//a.test/de/").is_absolute());
    assert!(alternate("javascript:alert(1)").url.is_none());
}
