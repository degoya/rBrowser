use super::*;

const FIXTURE: &str = include_str!("../../../tests/fixtures/sitemap/robots.txt");

fn base() -> Url {
    Url::parse("https://www.example.test/robots.txt").expect("base")
}

fn url(path: &str) -> Url {
    base().join(path).expect("url")
}

#[test]
fn the_fixture_names_its_index_and_keeps_private_pages_out() {
    let robots = Robots::parse(FIXTURE, &base(), CRAWLER);
    let sitemaps: Vec<String> = robots.sitemaps.iter().map(ToString::to_string).collect();
    assert_eq!(sitemaps, ["https://www.example.test/sitemap_index.xml"]);
    assert!(robots.allows(&url("/about")));
    assert!(!robots.allows(&url("/private/notes")));
    assert!(robots.allows(&url("/private/press")));
    assert!(!robots.allows(&url("/private/press/2026")));
}

#[test]
fn rbrowsers_own_group_wins_over_the_star_group() {
    let text = "User-agent: *\nDisallow: /\n\nUser-agent: Googlebot\nUser-agent: rBrowser\nDisallow: /admin\n";
    let robots = Robots::parse(text, &base(), CRAWLER);
    assert!(robots.allows(&url("/about")));
    assert!(!robots.allows(&url("/admin/users")));
}

#[test]
fn the_longest_pattern_wins_and_allow_wins_a_tie() {
    let text = "User-agent: *\nDisallow: /shop\nAllow: /shop/*.html$\nDisallow: /a\nAllow: /a\n";
    let robots = Robots::parse(text, &base(), CRAWLER);
    assert!(!robots.allows(&url("/shop/cart")));
    assert!(robots.allows(&url("/shop/shoe.html")));
    assert!(!robots.allows(&url("/shop/shoe.html?x=1")));
    assert!(robots.allows(&url("/a")));
}

#[test]
fn an_empty_disallow_and_no_robots_allow_everything() {
    let robots = Robots::parse("User-agent: *\nDisallow:\n", &base(), CRAWLER);
    assert!(robots.allows(&url("/anything")));
    assert!(Robots::default().allows(&url("/anything")));
}

#[test]
fn sitemap_lines_resolve_and_only_http_urls_count() {
    let text = "Sitemap: /relative.xml\nsitemap: ftp://x.test/s.xml\nSITEMAP: https://cdn.test/s.xml # comment\n";
    let robots = Robots::parse(text, &base(), CRAWLER);
    let sitemaps: Vec<String> = robots.sitemaps.iter().map(ToString::to_string).collect();
    assert_eq!(
        sitemaps,
        [
            "https://www.example.test/relative.xml",
            "https://cdn.test/s.xml"
        ]
    );
}

#[test]
fn wildcards_match_inside_the_path() {
    assert!(matches("/*/print", "/blog/print"));
    assert!(matches("/*.pdf$", "/a/b.pdf"));
    assert!(!matches("/*.pdf$", "/a/b.pdf.html"));
    assert!(matches("/a$", "/a"));
    assert!(!matches("/a$", "/ab"));
    assert!(matches("*", "/x"));
}

#[test]
fn a_search_engine_token_reads_its_own_group_or_the_star_group() {
    let text = "User-agent: googlebot\nDisallow: /g\n\nUser-agent: *\nDisallow: /all\n";
    let google = Robots::parse(text, &base(), SEARCH_ENGINE);
    assert!(!google.allows(&url("/g")));
    assert!(google.allows(&url("/all")));
    let ours = Robots::parse(text, &base(), CRAWLER);
    assert!(ours.allows(&url("/g")));
    assert!(!ours.allows(&url("/all")));
}
