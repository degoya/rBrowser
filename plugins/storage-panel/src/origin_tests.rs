use super::*;

#[test]
fn the_origin_of_a_url_drops_path_userinfo_and_default_port() {
    assert_eq!(
        origin_of("http://127.0.0.1:18767/storage/index.html?x#y").as_deref(),
        Some("http://127.0.0.1:18767")
    );
    assert_eq!(
        origin_of("HTTPS://User:pw@Example.COM:443/a").as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        origin_of("http://example.com:80").as_deref(),
        Some("http://example.com")
    );
    assert_eq!(
        origin_of("http://[::1]:8080/").as_deref(),
        Some("http://[::1]:8080")
    );
}

#[test]
fn no_origin_for_other_schemes_or_broken_urls() {
    for url in [
        "about:blank",
        "data:text/html,x",
        "file:///c:/x",
        "http://",
        "ftp://a.b",
        "",
    ] {
        assert_eq!(origin_of(url), None, "{url}");
    }
}

#[test]
fn an_origin_is_valid_only_without_a_path() {
    assert!(is_origin("https://example.com"));
    assert!(is_origin("http://localhost:3000"));
    assert!(!is_origin("https://example.com/"));
    assert!(!is_origin("https://example.com/a"));
    assert!(!is_origin("example.com"));
    assert!(!is_origin("https://Example.com"));
}

#[test]
fn host_and_path_of_a_url() {
    assert_eq!(
        host_of("https://a.example.com:8443/x").as_deref(),
        Some("a.example.com")
    );
    assert_eq!(path_of("https://a.example.com/x/y?q=1#f"), "/x/y");
    assert_eq!(path_of("https://a.example.com"), "/");
}

#[test]
fn cookie_domains_match_hosts_as_browsers_do() {
    // A domain cookie (leading dot) covers its subdomains; a host-only cookie its host alone.
    assert!(domain_matches("example.com", ".example.com"));
    assert!(domain_matches("a.example.com", ".example.com"));
    assert!(!domain_matches("badexample.com", ".example.com"));
    assert!(domain_matches("example.com", "example.com"));
    assert!(!domain_matches("a.example.com", "example.com"));
    assert!(domain_matches("EXAMPLE.com", "example.COM"));
}

#[test]
fn cookie_paths_match_on_segment_boundaries() {
    assert!(path_matches("/", "/"));
    assert!(path_matches("/shop/cart", "/shop"));
    assert!(path_matches("/shop/", "/shop/"));
    assert!(!path_matches("/shopping", "/shop"));
    assert!(!path_matches("/", "/shop"));
}

#[test]
fn a_cookie_domain_is_a_host_name_with_an_optional_dot() {
    assert!(is_cookie_domain(".example.com"));
    assert!(is_cookie_domain("127.0.0.1"));
    assert!(is_cookie_domain("localhost"));
    assert!(!is_cookie_domain(""));
    assert!(!is_cookie_domain("."));
    assert!(!is_cookie_domain("exa mple.com"));
    assert!(!is_cookie_domain("example.com/x"));
    assert!(!is_cookie_domain(&"a".repeat(254)));
}
