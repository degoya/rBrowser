use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};

use super::*;

const SITE: &str = "https://www.example.test";
const FIXTURES: [(&str, &[u8]); 5] = [
    (
        "robots.txt",
        include_bytes!("../../../tests/fixtures/sitemap/robots.txt"),
    ),
    (
        "sitemap_index.xml",
        include_bytes!("../../../tests/fixtures/sitemap/sitemap_index.xml"),
    ),
    (
        "sitemap-pages.xml",
        include_bytes!("../../../tests/fixtures/sitemap/sitemap-pages.xml"),
    ),
    (
        "sitemap-nested-index.xml",
        include_bytes!("../../../tests/fixtures/sitemap/sitemap-nested-index.xml"),
    ),
    (
        "sitemap-blog.xml.gz",
        include_bytes!("../../../tests/fixtures/sitemap/sitemap-blog.xml.gz"),
    ),
];

fn ok(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: body.to_vec(),
    }
}

fn redirect(status: u16, location: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("Location".to_owned(), location.to_owned())],
        body: Vec::new(),
    }
}

/// The fixture site at `SITE`, every file answered; anything else is a 404.
fn fixture_host() -> MemoryHost {
    let mut host = MemoryHost::new().grant(Capability::Http);
    for (name, bytes) in FIXTURES {
        host.responses.insert(format!("{SITE}/{name}"), ok(bytes));
    }
    host
}

fn site() -> Url {
    Url::parse(SITE).expect("site")
}

fn paths(found: &Discovery) -> Vec<String> {
    found
        .entries
        .iter()
        .map(|entry| entry.loc.path().to_owned())
        .collect()
}

#[test]
fn auto_follows_robots_the_index_the_nested_index_and_gzip() {
    let mut host = fixture_host();
    let found = discover(&mut host, &Source::Auto(site()), Limits::default(), true);
    assert_eq!(found.entries.len(), 19, "{:?}", paths(&found));
    assert!(!found.truncated);
    assert_eq!(found.skipped, 0);
    let read: Vec<(&str, usize)> = found
        .sitemaps
        .iter()
        .map(|report| (report.url.as_str(), report.pages))
        .collect();
    assert_eq!(
        read,
        [
            ("https://www.example.test/sitemap_index.xml", 0),
            ("https://www.example.test/sitemap-pages.xml", 12),
            ("https://www.example.test/sitemap-nested-index.xml", 0),
            ("https://www.example.test/sitemap-blog.xml.gz", 7),
        ]
    );
    assert!(found.robots.is_some());
    assert_eq!(found.site, Some(site()));
}

#[test]
fn without_robots_sitemap_xml_then_sitemap_index_xml_are_tried() {
    let mut host = fixture_host();
    host.responses.remove(&format!("{SITE}/robots.txt"));
    let found = discover(&mut host, &Source::Auto(site()), Limits::default(), true);
    assert_eq!(found.sitemaps[0].url, format!("{SITE}/sitemap.xml"));
    assert_eq!(found.sitemaps[0].error.as_deref(), Some("http_status"));
    assert_eq!(found.sitemaps[0].status, Some(404));
    assert_eq!(found.sitemaps[1].url, format!("{SITE}/sitemap_index.xml"));
    assert_eq!(found.entries.len(), 19);
}

#[test]
fn the_depth_limit_stops_at_the_nested_index() {
    let mut host = fixture_host();
    let limits = Limits {
        max_depth: 1,
        ..Limits::default()
    };
    let found = discover(&mut host, &Source::Auto(site()), limits, true);
    assert_eq!(found.entries.len(), 12);
    assert!(found.truncated);
    let nested = found
        .sitemaps
        .iter()
        .find(|report| report.url.ends_with("nested-index.xml"))
        .expect("nested");
    assert_eq!(nested.error.as_deref(), Some("too_deep"));
}

#[test]
fn the_url_and_sitemap_limits_cut_the_discovery() {
    let mut host = fixture_host();
    let limits = Limits {
        max_urls: 5,
        ..Limits::default()
    };
    let found = discover(&mut host, &Source::Auto(site()), limits, true);
    assert_eq!(found.entries.len(), 5);
    assert!(found.truncated);
    let mut host = fixture_host();
    let limits = Limits {
        max_sitemaps: 2,
        ..Limits::default()
    };
    let found = discover(&mut host, &Source::Auto(site()), limits, true);
    assert_eq!(found.sitemaps.len(), 2);
    assert!(found.truncated);
}

#[test]
fn the_byte_limit_holds_for_packed_and_unpacked_files() {
    let mut host = fixture_host();
    let limits = Limits {
        max_bytes: 400,
        ..Limits::default()
    };
    let found = discover(&mut host, &Source::Auto(site()), limits, true);
    let pages = found
        .sitemaps
        .iter()
        .find(|report| report.url.ends_with("sitemap-pages.xml"))
        .expect("pages");
    assert_eq!(pages.error.as_deref(), Some("too_large"));
    let blog = found
        .sitemaps
        .iter()
        .find(|report| report.url.ends_with(".gz"))
        .expect("blog");
    assert_eq!(
        blog.error.as_deref(),
        Some("too_large"),
        "unpacked it is larger"
    );
}

#[test]
fn redirects_are_followed_within_their_scheme_only() {
    let mut host = fixture_host();
    host.responses.insert(
        format!("{SITE}/moved.xml"),
        redirect(301, "/sitemap-pages.xml"),
    );
    host.responses.insert(
        format!("{SITE}/to-http.xml"),
        redirect(302, "http://www.example.test/sitemap-pages.xml"),
    );
    host.responses
        .insert(format!("{SITE}/loop.xml"), redirect(307, "/loop.xml"));
    let source = |path: &str| Source::Sitemap(site().join(path).expect("url"));
    let found = discover(&mut host, &source("/moved.xml"), Limits::default(), false);
    assert_eq!(found.entries.len(), 12);
    let found = discover(&mut host, &source("/to-http.xml"), Limits::default(), false);
    assert_eq!(found.sitemaps[0].error.as_deref(), Some("redirect_scheme"));
    assert!(found.entries.is_empty());
    let found = discover(&mut host, &source("/loop.xml"), Limits::default(), false);
    assert_eq!(found.sitemaps[0].error.as_deref(), Some("redirect_loop"));
    assert!(
        !host
            .fetched
            .iter()
            .any(|request| request.url.starts_with("http://")),
        "nothing was fetched over another scheme"
    );
}

#[test]
fn an_index_naming_another_origin_is_not_followed() {
    let mut host = fixture_host();
    host.responses.insert(
        format!("{SITE}/foreign-index.xml"),
        ok(b"<sitemapindex><sitemap><loc>https://intranet.test/s.xml</loc></sitemap></sitemapindex>"),
    );
    let source = Source::Sitemap(site().join("/foreign-index.xml").expect("url"));
    let found = discover(&mut host, &source, Limits::default(), false);
    assert_eq!(found.skipped, 1);
    assert!(
        !host
            .fetched
            .iter()
            .any(|request| request.url.contains("intranet")),
    );
}

#[test]
fn a_sitemap_url_reads_robots_only_when_asked() {
    let mut host = fixture_host();
    let source = Source::Sitemap(site().join("/sitemap-pages.xml").expect("url"));
    let found = discover(&mut host, &source, Limits::default(), false);
    assert!(found.robots.is_none());
    let found = discover(&mut host, &source, Limits::default(), true);
    assert!(found.robots.is_some());
}

#[test]
fn a_file_source_unpacks_and_follows_its_index() {
    let mut host = fixture_host();
    let source = Source::File(FIXTURES[4].1.to_vec());
    let found = discover(&mut host, &source, Limits::default(), false);
    assert_eq!(found.entries.len(), 7);
    let source = Source::File(FIXTURES[1].1.to_vec());
    let found = discover(&mut host, &source, Limits::default(), false);
    assert_eq!(found.entries.len(), 19);
    let source = Source::File(b"<html/>".to_vec());
    let found = discover(&mut host, &source, Limits::default(), false);
    assert_eq!(found.sitemaps[0].error.as_deref(), Some("not_a_sitemap"));
}

#[test]
fn a_list_source_takes_its_urls_once() {
    let mut host = fixture_host();
    let urls = vec![
        format!("{SITE}/a"),
        format!("{SITE}/a"),
        "javascript:alert(1)".to_owned(),
        format!("{SITE}/b"),
    ];
    let found = discover(&mut host, &Source::List(urls), Limits::default(), false);
    assert_eq!(paths(&found), ["/a", "/b"]);
    assert_eq!(found.skipped, 1);
    assert!(host.fetched.is_empty());
}

#[test]
fn without_the_http_capability_every_fetch_fails() {
    let mut host = MemoryHost::new();
    let found = discover(&mut host, &Source::Auto(site()), Limits::default(), true);
    assert!(found.entries.is_empty());
    assert!(found.sitemaps.iter().all(|report| report.error.is_some()));
}

#[test]
fn limits_are_held_to_their_ceilings() {
    let limits = Limits {
        max_bytes: usize::MAX,
        max_depth: 99,
        max_urls: 0,
        max_sitemaps: usize::MAX,
        max_redirects: 99,
    }
    .clamped();
    assert_eq!(limits.max_bytes, MAX_BYTES.1);
    assert_eq!(limits.max_depth, MAX_DEPTH.1);
    assert_eq!(limits.max_urls, 1);
    assert_eq!(limits.max_sitemaps, MAX_SITEMAPS.1);
    assert_eq!(limits.max_redirects, MAX_REDIRECTS.1);
}

#[test]
fn redirects_are_followed_within_their_origin_only() {
    let mut host = fixture_host();
    host.responses.insert(
        format!("{SITE}/elsewhere.xml"),
        redirect(302, "https://intranet.test/sitemap.xml"),
    );
    let source = Source::Sitemap(site().join("/elsewhere.xml").expect("url"));
    let found = discover(&mut host, &source, Limits::default(), false);
    assert_eq!(found.sitemaps[0].error.as_deref(), Some("redirect_origin"));
    assert!(
        !host
            .fetched
            .iter()
            .any(|request| request.url.contains("intranet")),
        "nothing was fetched from another origin"
    );
}

#[test]
fn pages_of_another_origin_are_dropped_and_counted() {
    let mut host = fixture_host();
    host.responses.insert(
        format!("{SITE}/mixed.xml"),
        ok(b"<urlset>\
            <url><loc>https://www.example.test/kept</loc></url>\
            <url><loc>https://evil.test/page</loc></url>\
            <url><loc>http://127.0.0.1:9000/admin</loc></url>\
            <url><loc>http://www.example.test/kept</loc></url>\
            </urlset>"),
    );
    let source = Source::Sitemap(site().join("/mixed.xml").expect("url"));
    let found = discover(&mut host, &source, Limits::default(), false);
    assert_eq!(paths(&found), ["/kept"]);
    assert_eq!(found.foreign, 3);
}

#[test]
fn a_public_site_never_fetches_a_private_or_loopback_sitemap() {
    let mut host = fixture_host();
    host.responses.insert(
        format!("{SITE}/robots.txt"),
        ok(b"Sitemap: http://169.254.169.254/latest/meta-data\n\
             Sitemap: http://10.0.0.5/sitemap.xml\n\
             Sitemap: https://www.example.test/sitemap-pages.xml\n"),
    );
    let found = discover(&mut host, &Source::Auto(site()), Limits::default(), true);
    assert_eq!(found.entries.len(), 12, "the site's own sitemap is read");
    assert!(
        !host
            .fetched
            .iter()
            .any(|request| request.url.contains("169.254") || request.url.contains("10.0.0.5")),
        "no private address was fetched"
    );
    let blocked: Vec<&str> = found
        .sitemaps
        .iter()
        .filter(|report| report.error.as_deref() == Some("private_address"))
        .map(|report| report.url.as_str())
        .collect();
    assert_eq!(blocked.len(), 2, "{:?}", found.sitemaps);

    let mut host = fixture_host();
    let file =
        b"<sitemapindex><sitemap><loc>http://192.168.1.1/s.xml</loc></sitemap></sitemapindex>";
    let found = discover(
        &mut host,
        &Source::File(file.to_vec()),
        Limits::default(),
        false,
    );
    assert!(host.fetched.is_empty(), "a file's index has no local site");
    assert_eq!(found.sitemaps[0].error.as_deref(), Some("private_address"));
}

#[test]
fn a_local_site_may_read_local_sitemaps() {
    let local = "http://localhost:5173";
    let mut host = MemoryHost::new().grant(Capability::Http);
    host.responses.insert(
        format!("{local}/robots.txt"),
        ok(b"Sitemap: http://127.0.0.1:5173/sitemap.xml\n"),
    );
    host.responses.insert(
        "http://127.0.0.1:5173/sitemap.xml".to_owned(),
        ok(b"<urlset><url><loc>http://localhost:5173/a</loc></url></urlset>"),
    );
    let site = Url::parse(local).expect("site");
    let found = discover(&mut host, &Source::Auto(site), Limits::default(), true);
    assert_eq!(paths(&found), ["/a"]);
}
