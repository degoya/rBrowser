use super::*;
use crate::gzip::gunzip;

const PAGES: &[u8] = include_bytes!("../../../tests/fixtures/sitemap/sitemap-pages.xml");
const INDEX: &[u8] = include_bytes!("../../../tests/fixtures/sitemap/sitemap_index.xml");
const BLOG_GZ: &[u8] = include_bytes!("../../../tests/fixtures/sitemap/sitemap-blog.xml.gz");
const LIST: &[u8] = include_bytes!("../../../tests/fixtures/sitemap/urls.txt");

fn base() -> Url {
    Url::parse("https://www.example.test/sitemap.xml").expect("base")
}

fn pages(parsed: Parsed) -> (Vec<Entry>, usize) {
    match parsed {
        Parsed::Pages { entries, skipped } => (entries, skipped),
        Parsed::Index { .. } => panic!("an index, not pages"),
    }
}

#[test]
fn a_urlset_gives_its_pages_with_lastmod_and_language() {
    let (entries, skipped) = pages(parse_xml(PAGES, Some(&base())).expect("parses"));
    assert_eq!(entries.len(), 12);
    assert_eq!(skipped, 0);
    let home = &entries[0];
    assert_eq!(home.loc.to_string(), "https://www.example.test/");
    assert_eq!(home.lastmod.as_deref(), Some("2026-10-01"));
    assert_eq!(home.language.as_deref(), Some("en"));
    let german = entries
        .iter()
        .find(|entry| entry.loc.path() == "/de/")
        .expect("/de/");
    assert_eq!(german.language.as_deref(), Some("de"));
    let missing = entries.last().expect("last");
    assert_eq!(missing.loc.path(), "/missing");
    assert_eq!(missing.lastmod, None);
    assert_eq!(missing.language, None);
}

#[test]
fn an_index_gives_its_sitemaps() {
    match parse_xml(INDEX, Some(&base())).expect("parses") {
        Parsed::Index { sitemaps, skipped } => {
            let names: Vec<String> = sitemaps.iter().map(ToString::to_string).collect();
            assert_eq!(
                names,
                [
                    "https://www.example.test/sitemap-pages.xml",
                    "https://www.example.test/sitemap-nested-index.xml"
                ]
            );
            assert_eq!(skipped, 0);
        }
        Parsed::Pages { .. } => panic!("pages, not an index"),
    }
}

#[test]
fn a_gzip_sitemap_unpacks_and_parses() {
    let xml = gunzip(BLOG_GZ, 1 << 20).expect("unpacks");
    let (entries, _) = pages(parse_xml(&xml, Some(&base())).expect("parses"));
    assert_eq!(entries.len(), 7);
    assert_eq!(
        entries[4].lastmod.as_deref(),
        Some("2025-08-01T10:00:00+02:00")
    );
}

#[test]
fn locs_that_are_no_http_urls_are_skipped_and_relative_ones_resolved() {
    let xml = br#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url><loc>javascript:alert(1)</loc></url>
        <url><loc>file:///etc/passwd</loc></url>
        <url><loc>  /relative/page  </loc></url>
        <url><lastmod>2026-01-01</lastmod></url>
        <url><loc>https://a.test/x?y=1&amp;z=2</loc></url>
    </urlset>"#;
    let (entries, skipped) = pages(parse_xml(xml, Some(&base())).expect("parses"));
    let locs: Vec<String> = entries.iter().map(|entry| entry.loc.to_string()).collect();
    assert_eq!(
        locs,
        [
            "https://www.example.test/relative/page",
            "https://a.test/x?y=1&z=2"
        ]
    );
    assert_eq!(skipped, 3);
}

#[test]
fn a_sitemap_without_its_namespace_still_reads() {
    let xml = b"<urlset><url><loc>https://a.test/</loc></url></urlset>";
    let (entries, _) = pages(parse_xml(xml, None).expect("parses"));
    assert_eq!(entries.len(), 1);
}

#[test]
fn a_dtd_and_other_documents_are_refused() {
    let laughs = br#"<?xml version="1.0"?><!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;">]><urlset><url><loc>&lol2;</loc></url></urlset>"#;
    assert!(matches!(parse_xml(laughs, None), Err(ParseError::Xml(_))));
    assert!(matches!(
        parse_xml(b"<html><body/></html>", None),
        Err(ParseError::NotASitemap(name)) if name == "html"
    ));
    assert!(matches!(
        parse_xml(b"<urlset>", None),
        Err(ParseError::Xml(_))
    ));
    assert_eq!(
        parse_xml(&[0xff, 0xfe, 0x00], None),
        Err(ParseError::Encoding)
    );
}

#[test]
fn a_url_list_skips_comments_blank_lines_and_other_schemes() {
    let (entries, skipped) = pages(parse_list(LIST, None).expect("parses"));
    let paths: Vec<&str> = entries.iter().map(|entry| entry.loc.path()).collect();
    assert_eq!(paths, ["/", "/about", "/contact"]);
    assert_eq!(skipped, 2);
}

#[test]
fn parse_any_tells_xml_from_a_list() {
    assert!(matches!(
        parse_any(INDEX, None).expect("xml"),
        Parsed::Index { .. }
    ));
    let (entries, _) = pages(parse_any(LIST, None).expect("list"));
    assert_eq!(entries.len(), 3);
    let (entries, _) = pages(parse_any("\u{feff}<urlset/>".as_bytes(), None).expect("bom"));
    assert!(entries.is_empty());
}
