use super::*;
use crate::gzip::gunzip;
use crate::robots::Robots;
use crate::sitemap::{Parsed, parse_xml};

const PAGES: &[u8] = include_bytes!("../../../tests/fixtures/sitemap/sitemap-pages.xml");
const BLOG_GZ: &[u8] = include_bytes!("../../../tests/fixtures/sitemap/sitemap-blog.xml.gz");

fn entries(bytes: &[u8]) -> Vec<Entry> {
    match parse_xml(bytes, None).expect("parses") {
        Parsed::Pages { entries, .. } => entries,
        Parsed::Index { .. } => panic!("pages expected"),
    }
}

/// The 19 pages of the fixture site.
fn fixture() -> Discovery {
    let mut all = entries(PAGES);
    all.extend(entries(&gunzip(BLOG_GZ, 1 << 20).expect("unpacks")));
    Discovery {
        site: Url::parse("https://www.example.test/"),
        entries: all,
        ..Discovery::default()
    }
}

fn paths(pages: &[Page]) -> Vec<String> {
    pages
        .iter()
        .map(|page| Url::parse(&page.url).expect("url").path().to_owned())
        .collect()
}

fn run(selection: &Selection) -> (Vec<String>, Counts) {
    let (pages, counts) = select(&fixture(), selection, None).expect("selects");
    (paths(&pages), counts)
}

#[test]
fn nothing_chosen_keeps_every_page() {
    let (pages, counts) = run(&Selection::default());
    assert_eq!(pages.len(), 19);
    assert_eq!(counts.found, 19);
    assert_eq!(counts.selected, 19);
}

#[test]
fn include_and_exclude_globs_match_the_path() {
    let selection = Selection {
        include: vec!["/shop/**".to_owned(), "/blog/2024/*".to_owned()],
        exclude: vec!["/shop/shoe-?".to_owned()],
        ..Selection::default()
    };
    let (pages, counts) = run(&selection);
    assert_eq!(pages, ["/shop/", "/blog/2024/hello", "/blog/2024/second"]);
    assert_eq!(counts.pattern, 16);
}

#[test]
fn a_regex_after_re_is_searched_in_the_whole_url() {
    let selection = Selection {
        include: vec![r"re:/blog/20(25|26)/".to_owned()],
        ..Selection::default()
    };
    let (pages, _) = run(&selection);
    assert_eq!(pages.len(), 4);
    let selection = Selection {
        exclude: vec!["https://www.example.test/de/**".to_owned()],
        ..Selection::default()
    };
    assert_eq!(run(&selection).0.len(), 17);
}

#[test]
fn bad_patterns_dates_and_rewrites_are_refused() {
    let found = fixture();
    let bad = |selection: Selection| {
        select(&found, &selection, None)
            .map(|_| ())
            .unwrap_err()
            .code
    };
    assert_eq!(
        bad(Selection {
            include: vec!["re:(".to_owned()],
            ..Selection::default()
        }),
        "bad_pattern"
    );
    assert_eq!(
        bad(Selection {
            include: vec![String::new()],
            ..Selection::default()
        }),
        "bad_pattern"
    );
    assert_eq!(
        bad(Selection {
            exclude: vec!["/x".to_owned(); MAX_PATTERNS + 1],
            ..Selection::default()
        }),
        "too_many_patterns"
    );
    assert_eq!(
        bad(Selection {
            lastmod_since: Some("last week".to_owned()),
            ..Selection::default()
        }),
        "bad_date"
    );
    let rewrite = Rewrite {
        from: None,
        to: "file:///c:/".to_owned(),
    };
    let error = select(&found, &Selection::default(), Some(&rewrite))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(error.code, "bad_rewrite");
}

#[test]
fn hreflang_keeps_the_pages_of_a_language_and_its_regions() {
    let selection = Selection {
        languages: vec!["DE".to_owned()],
        ..Selection::default()
    };
    let (pages, counts) = run(&selection);
    assert_eq!(pages, ["/de/", "/de/ueber-uns"]);
    assert_eq!(counts.language, 17);
    assert!(language_matches(&["de".to_owned()], Some("de-at")));
    assert!(!language_matches(&["de".to_owned()], Some("dex")));
    assert!(!language_matches(&["de".to_owned()], None));
}

#[test]
fn lastmod_since_keeps_newer_pages_and_drops_undated_ones() {
    let selection = Selection {
        lastmod_since: Some("2026-10-01".to_owned()),
        ..Selection::default()
    };
    let (pages, counts) = run(&selection);
    assert_eq!(
        pages,
        [
            "/",
            "/pricing",
            "/shop/",
            "/shop/shoe-1",
            "/shop/shoe-2",
            "/shop/shoe-3",
            "/de/",
            "/blog/",
            "/blog/2026/sixth"
        ]
    );
    assert_eq!(counts.lastmod, 10, "/missing has no lastmod");
}

#[test]
fn one_page_per_template_path() {
    let selection = Selection {
        per_template: true,
        ..Selection::default()
    };
    let (pages, counts) = run(&selection);
    assert!(pages.contains(&"/shop/shoe-1".to_owned()));
    assert!(!pages.contains(&"/shop/shoe-2".to_owned()));
    assert!(pages.contains(&"/blog/2024/hello".to_owned()));
    assert!(!pages.contains(&"/blog/2025/third".to_owned()));
    assert_eq!(counts.template, 7);
    let url = |text: &str| Url::parse(text).expect("url");
    assert_eq!(template(&url("https://a.test/")), "/");
    assert_eq!(template(&url("https://a.test/about")), "/about");
    assert_eq!(
        template(&url("https://a.test/blog/2024/hello")),
        "/blog/*/*"
    );
    assert_eq!(template(&url("https://a.test/shop/")), "/shop");
}

#[test]
fn every_nth_and_max_sample_the_rest() {
    let selection = Selection {
        every_nth: 5,
        ..Selection::default()
    };
    let (pages, counts) = run(&selection);
    assert_eq!(pages, ["/", "/shop/", "/de/ueber-uns", "/blog/2025/third"]);
    assert_eq!(counts.sample, 15);
    let selection = Selection {
        max: 3,
        ..Selection::default()
    };
    let (pages, counts) = run(&selection);
    assert_eq!(pages, ["/", "/about", "/contact"]);
    assert_eq!(counts.max, 16);
}

#[test]
fn robots_leaves_out_what_it_disallows_when_respected() {
    let mut found = fixture();
    let base = Url::parse("https://www.example.test/robots.txt").expect("base");
    found.robots = Some(Robots::parse("User-agent: *\nDisallow: /shop/\n", &base));
    let respect = Selection {
        respect_robots: true,
        ..Selection::default()
    };
    let (pages, counts) = select(&found, &respect, None).expect("selects");
    assert_eq!(pages.len(), 15);
    assert_eq!(counts.robots, 4);
    let (pages, _) = select(&found, &Selection::default(), None).expect("selects");
    assert_eq!(pages.len(), 19);
}

#[test]
fn the_rewrite_moves_the_sites_pages_and_keeps_the_path() {
    let mut found = fixture();
    found.entries.push(Entry {
        loc: Url::parse("https://cdn.example.test/asset-page").expect("url"),
        lastmod: None,
        language: None,
    });
    let rewrite = Rewrite {
        from: None,
        to: "http://127.0.0.1:8080".to_owned(),
    };
    let (pages, _) = select(&found, &Selection::default(), Some(&rewrite)).expect("selects");
    assert_eq!(pages[1].url, "http://127.0.0.1:8080/about");
    assert_eq!(
        pages[1].sitemap_url.as_deref(),
        Some("https://www.example.test/about")
    );
    assert!(
        pages
            .iter()
            .all(|page| page.url.starts_with("http://127.0.0.1:8080/")),
        "no page is left on production"
    );
    assert_eq!(pages.len(), 19);
    let only_cdn = Rewrite {
        from: Some("https://cdn.example.test".to_owned()),
        to: "http://localhost:3000".to_owned(),
    };
    let (pages, counts) = select(&found, &Selection::default(), Some(&only_cdn)).expect("selects");
    assert_eq!(paths(&pages), ["/asset-page"]);
    assert_eq!(pages[0].url, "http://localhost:3000/asset-page");
    assert_eq!((counts.origin, counts.selected), (19, 1));
}

#[test]
fn pages_the_rewrite_does_not_cover_are_left_out_and_counted() {
    let mut found = fixture();
    found.entries.push(Entry {
        loc: Url::parse("https://cdn.example.test/asset-page").expect("url"),
        lastmod: None,
        language: None,
    });
    let rewrite = Rewrite {
        from: None,
        to: "http://127.0.0.1:8080".to_owned(),
    };
    let (pages, counts) = select(&found, &Selection::default(), Some(&rewrite)).expect("selects");
    assert_eq!(counts.origin, 1);
    assert_eq!(counts.selected, 19);
    assert!(
        !pages
            .iter()
            .any(|page| page.url.contains("cdn.example.test"))
    );
    let (pages, counts) = select(&found, &Selection::default(), None).expect("selects");
    assert_eq!((pages.len(), counts.origin), (20, 0), "no rewrite, no cut");
}

#[test]
fn a_rewrite_from_that_is_no_origin_is_refused() {
    let rewrite = Rewrite {
        from: Some("not a url".to_owned()),
        to: "http://127.0.0.1:8080".to_owned(),
    };
    let error = select(&fixture(), &Selection::default(), Some(&rewrite))
        .map(|_| ())
        .expect_err("refused");
    assert_eq!(
        (error.code, error.pattern.as_str()),
        ("bad_rewrite", "not a url")
    );
}
