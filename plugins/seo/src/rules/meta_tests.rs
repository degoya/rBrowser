use super::*;
use crate::testing::{check, serve, short as brief};

const URL: &str = "https://shop.test/";

#[test]
fn a_complete_head_is_quiet() {
    let html = r#"<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width">
<title>A title of a fitting length for the search results</title>
<meta name="description" content="A description that is long enough to say what the page is about, and not longer.">
</head><body></body></html>"#;
    let mut host = serve(URL, html, &[]);
    assert!(brief(&check(&mut host, URL, None), "meta").is_empty());
}

#[test]
fn a_bare_document_misses_everything() {
    let mut host = serve(URL, "<p>x</p>", &[("content-type", "text/html")]);
    host.responses
        .get_mut(URL)
        .expect("page")
        .headers
        .retain(|(_, value)| !value.contains("charset"));
    assert_eq!(
        brief(&check(&mut host, URL, None), "meta"),
        [
            "meta.title_missing meta.title_missing",
            "meta.description_missing meta.description_missing",
            "meta.lang_missing meta.lang_missing",
            "meta.viewport_missing meta.viewport_missing",
            "meta.charset_missing meta.charset_missing",
        ]
    );
}

#[test]
fn lengths_and_repeats_are_reported() {
    let html = r#"<html lang="en"><head><meta name="viewport" content="x">
<title>Short</title><title>Second</title>
<meta name="description" content="Tiny"><meta name="description" content="Other">
</head></html>"#;
    let mut host = serve(URL, html, &[]);
    let found = check(&mut host, URL, None);
    assert_eq!(
        brief(&found, "meta"),
        [
            "meta.title_length meta.title_short",
            "meta.title_multiple meta.title_multiple",
            "meta.description_length meta.description_short",
            "meta.description_multiple meta.description_multiple",
        ]
    );
    let title = found
        .iter()
        .find(|finding| finding.rule == "meta.title_length")
        .expect("title length");
    assert_eq!(
        title.path.as_deref(),
        Some("html > head > title:nth-of-type(1)")
    );
    assert!(
        title
            .params
            .contains(&("length".to_owned(), "5".to_owned()))
    );
}

#[test]
fn duplicates_name_the_other_page() {
    let page = |url: &str, title: &str| PageSummary {
        final_url: url.to_owned(),
        status: Some(200),
        title: Some(title.to_owned()),
        ..PageSummary::default()
    };
    let pages = [
        page("https://a.test/1", "Same"),
        page("https://a.test/2", "same"),
        page("https://a.test/3", "Other"),
    ];
    let config = Config::default();
    let enabled = Vec::new();
    let mut sink = Sink::new(&config, &enabled);
    duplicates(&pages, &mut sink);
    let urls: Vec<_> = sink
        .findings
        .iter()
        .map(|finding| {
            (
                finding.rule.as_str(),
                finding.url.clone().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        urls,
        [
            ("meta.title_duplicate", "https://a.test/1".to_owned()),
            ("meta.title_duplicate", "https://a.test/2".to_owned())
        ]
    );
    assert!(
        sink.findings[0]
            .params
            .contains(&("other".to_owned(), "https://a.test/2".to_owned()))
    );
}
