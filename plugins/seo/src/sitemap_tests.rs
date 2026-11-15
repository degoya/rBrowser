use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

fn xml(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), "application/xml".to_owned())],
        body: body.as_bytes().to_vec(),
    }
}

const URLSET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
  <url>
    <loc>https://a.test/en/</loc>
    <xhtml:link rel="alternate" hreflang="en" href="https://a.test/en/"/>
    <xhtml:link rel="alternate" hreflang="de" href="https://a.test/de/"/>
  </url>
  <url><loc>https://a.test/plain/</loc></url>
</urlset>"#;

#[test]
fn a_urlset_gives_the_alternates_by_page() {
    let base = Url::parse("https://a.test/sitemap.xml").expect("url");
    let map = parse(URLSET.as_bytes(), &base).expect("a sitemap");
    let Parsed::Pages(pages) = map else {
        panic!("pages");
    };
    assert_eq!(pages.len(), 1, "a page without alternates is not kept");
    let en = Url::parse("https://a.test/en/").expect("url");
    let codes: Vec<&str> = pages[&en].iter().map(|a| a.code.as_str()).collect();
    assert_eq!(codes, ["en", "de"]);
}

#[test]
fn a_dtd_and_broken_xml_are_refused() {
    let base = Url::parse("https://a.test/sitemap.xml").expect("url");
    let dtd = r#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY a "aaaa">]><urlset>&a;</urlset>"#;
    assert!(parse(dtd.as_bytes(), &base).is_none());
    assert!(parse(b"<urlset><url>", &base).is_none());
    assert!(parse(b"<html></html>", &base).is_none());
}

#[test]
fn the_index_is_followed_within_the_origin_and_its_limits() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        "https://a.test/sitemap_index.xml".to_owned(),
        xml(
            r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <sitemap><loc>https://a.test/pages.xml</loc></sitemap>
            <sitemap><loc>https://other.test/pages.xml</loc></sitemap>
            </sitemapindex>"#,
        ),
    );
    host.responses
        .insert("https://a.test/pages.xml".to_owned(), xml(URLSET));
    let start = [Url::parse("https://a.test/sitemap_index.xml").expect("url")];
    let found = Sitemaps::load(&mut host, &start, false);
    assert_eq!(found.pages.len(), 1);
    let fetched: Vec<&str> = host.fetched.iter().map(|r| r.url.as_str()).collect();
    assert_eq!(
        fetched,
        [
            "https://a.test/sitemap_index.xml",
            "https://a.test/pages.xml"
        ]
    );
}

#[test]
fn a_local_sitemap_of_a_public_site_is_never_fetched() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    let start = [Url::parse("http://127.0.0.1/sitemap.xml").expect("url")];
    let found = Sitemaps::load(&mut host, &start, false);
    assert!(found.pages.is_empty());
    assert!(host.fetched.is_empty());
    let _ = Sitemaps::load(&mut host, &start, true);
    assert_eq!(host.fetched.len(), 1, "a local site reads its own");
}

#[test]
fn the_alternates_kept_are_bounded() {
    let mut host = MemoryHost::new().grant(Capability::Http);
    let links: String = (0..400)
        .map(|index| format!(r#"<xhtml:link rel="alternate" hreflang="x{index}" href="https://a.test/{index}/"/>"#))
        .collect();
    let urls: String = (0..200)
        .map(|index| format!("<url><loc>https://a.test/p{index}/</loc>{links}</url>"))
        .collect();
    host.responses.insert(
        "https://a.test/sitemap.xml".to_owned(),
        xml(&format!(
            r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">{urls}</urlset>"#
        )),
    );
    let start = [Url::parse("https://a.test/sitemap.xml").expect("url")];
    let found = Sitemaps::load(&mut host, &start, false);
    let kept: usize = found.pages.values().map(Vec::len).sum();
    assert!(kept <= MAX_KEPT);
    assert_eq!(found.pages.len(), MAX_KEPT / 400);
}
