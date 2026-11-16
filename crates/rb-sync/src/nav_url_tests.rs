use super::*;

#[test]
fn normalize_lowercases_the_origin_and_drops_default_ports() {
    assert_eq!(
        normalize("HTTP://Example.COM:80/Produkt?a=1", false),
        "http://example.com/Produkt?a=1"
    );
    assert_eq!(
        normalize("https://example.com:443/", false),
        "https://example.com/"
    );
    assert_eq!(
        normalize("http://localhost:5173/a", false),
        "http://localhost:5173/a"
    );
}

#[test]
fn normalize_gives_an_empty_path_its_slash() {
    assert_eq!(
        normalize("http://example.com", false),
        "http://example.com/"
    );
    assert_eq!(
        normalize("http://example.com?q=1", false),
        "http://example.com/?q=1"
    );
}

#[test]
fn the_hash_counts_only_when_asked() {
    assert_eq!(
        normalize("http://example.com/a#top", false),
        "http://example.com/a"
    );
    assert_eq!(
        normalize("http://example.com/a#top", true),
        "http://example.com/a#top"
    );
    // An empty fragment is no fragment.
    assert_eq!(
        normalize("http://example.com/a#", true),
        "http://example.com/a"
    );
}

#[test]
fn same_page_compares_the_normalized_urls() {
    assert!(same_page(
        "http://Example.com/a#x",
        "http://example.com:80/a#y",
        false
    ));
    assert!(!same_page(
        "http://example.com/a#x",
        "http://example.com/a#y",
        true
    ));
    assert!(!same_page(
        "http://example.com/m/produkt",
        "http://example.com/produkt",
        false
    ));
    assert!(!same_page(
        "http://example.com/a?x=1",
        "http://example.com/a?x=2",
        false
    ));
}

#[test]
fn urls_outside_http_compare_as_written() {
    assert!(same_page("about:blank", "about:blank", false));
    assert!(same_page(
        "file:///C:/site/index.html#a",
        "file:///C:/site/index.html",
        false
    ));
}

#[test]
fn only_web_and_file_pages_are_followed() {
    for url in [
        "http://localhost:5173/",
        "HTTPS://example.com/a",
        "file:///C:/site/index.html",
    ] {
        assert!(followable(url), "{url}");
    }
    for url in [
        "",
        "about:blank",
        "chrome-error://chromewebdata/",
        "javascript:alert(1)",
        "data:text/html,x",
        "chrome://settings",
        "view-source:http://example.com/",
    ] {
        assert!(!followable(url), "{url}");
    }
}

#[test]
fn same_origin_ignores_case_and_default_ports() {
    assert!(same_origin(
        "http://Example.com/a",
        "http://example.com:80/b"
    ));
    assert!(!same_origin(
        "http://example.com/a",
        "https://example.com/a"
    ));
    assert!(!same_origin(
        "http://example.com/a",
        "http://m.example.com/a"
    ));
    assert!(!same_origin("about:blank", "about:blank"));
}

#[test]
fn a_follower_in_another_environment_keeps_its_origin() {
    let environments = vec![
        "http://localhost:5173".to_owned(),
        "https://staging.example.com/shop/".to_owned(),
    ];
    assert_eq!(
        follow_url(
            "http://localhost:5173/produkt?id=7#preis",
            "https://staging.example.com/shop/start",
            &environments
        ),
        "https://staging.example.com/shop/produkt?id=7#preis"
    );
    assert_eq!(
        follow_url(
            "https://staging.example.com/shop/a",
            "http://localhost:5173/b",
            &environments
        ),
        "http://localhost:5173/a"
    );
}

#[test]
fn a_follower_in_the_same_or_no_environment_loads_the_leaders_url() {
    let environments = vec!["http://localhost:5173".to_owned()];
    assert_eq!(
        follow_url(
            "http://localhost:5173/produkt",
            "http://localhost:5173/m/produkt",
            &environments
        ),
        "http://localhost:5173/produkt"
    );
    assert_eq!(
        follow_url(
            "http://localhost:5173/produkt",
            "about:blank",
            &environments
        ),
        "http://localhost:5173/produkt"
    );
    assert_eq!(
        follow_url(
            "https://example.org/x",
            "http://localhost:5173/y",
            &environments
        ),
        "https://example.org/x"
    );
    assert_eq!(
        follow_url("http://localhost:5173/a", "http://other.test/b", &[]),
        "http://localhost:5173/a"
    );
}

#[test]
fn an_environment_matches_only_at_a_segment_boundary() {
    let environments = vec![
        "http://localhost:5173/shop".to_owned(),
        "http://localhost:5174".to_owned(),
    ];
    // `/shopping` is not under `/shop`.
    assert_eq!(
        follow_url(
            "http://localhost:5173/shopping",
            "http://localhost:5174/x",
            &environments
        ),
        "http://localhost:5173/shopping"
    );
}

#[test]
fn the_origin_of_a_web_page() {
    assert_eq!(
        origin_of("HTTP://Example.com:80/a?b#c").as_deref(),
        Some("http://example.com")
    );
    assert_eq!(
        origin_of("http://localhost:3000").as_deref(),
        Some("http://localhost:3000")
    );
    assert_eq!(origin_of("file:///C:/a.html"), None);
    assert_eq!(origin_of("about:blank"), None);
}
