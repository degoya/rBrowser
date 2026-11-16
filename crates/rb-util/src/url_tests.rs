use super::{authority_of, host_of, origin_of};

#[test]
fn the_authority_keeps_the_port_and_drops_the_user_info() {
    assert_eq!(
        authority_of("https://WWW.Example.test:8080/a?b"),
        Some("WWW.Example.test:8080")
    );
    assert_eq!(authority_of("http://user:pw@a.test#x"), Some("a.test"));
    assert_eq!(authority_of("http://[::1]:8080/"), Some("[::1]:8080"));
    assert_eq!(authority_of("ftp://x/y"), Some("x"));
    assert_eq!(authority_of("about:blank"), None);
    assert_eq!(authority_of("https://"), None);
    assert_eq!(authority_of("https:///nohost"), None);
}

#[test]
fn the_host_leaves_scheme_port_and_user_out() {
    assert_eq!(
        host_of("https://u:p@Fonts.Example.com:8443/a.woff2").as_deref(),
        Some("fonts.example.com")
    );
    assert_eq!(
        host_of("http://127.0.0.1:18766/account").as_deref(),
        Some("127.0.0.1")
    );
    assert_eq!(
        host_of("https://User@WWW.a.test:8080/x").as_deref(),
        Some("www.a.test")
    );
    assert_eq!(host_of("http://[::1]:8080/x").as_deref(), Some("[::1]"));
    assert_eq!(
        host_of("wss://Live.test/socket").as_deref(),
        Some("live.test")
    );
}

#[test]
fn no_host_outside_the_web_schemes_or_without_one() {
    for url in [
        "about:blank",
        "file:///C:/site/index.html",
        "data:font/woff2;base64,AA",
        "ftp://x/y",
        "http://",
        "http://user@/x",
        "",
    ] {
        assert_eq!(host_of(url), None, "{url}");
    }
}

#[test]
fn the_origin_drops_path_user_info_and_the_default_port() {
    assert_eq!(
        origin_of("http://127.0.0.1:18767/storage/index.html?x#y").as_deref(),
        Some("http://127.0.0.1:18767")
    );
    assert_eq!(
        origin_of("HTTPS://User:pw@Example.COM:443/a").as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        origin_of("HTTP://Example.com:80/a?b#c").as_deref(),
        Some("http://example.com")
    );
    assert_eq!(
        origin_of("https://Shop.Example.com:8443/cart?x=1#top").as_deref(),
        Some("https://shop.example.com:8443")
    );
    assert_eq!(
        origin_of("http://localhost:3000").as_deref(),
        Some("http://localhost:3000")
    );
    assert_eq!(
        origin_of("https://a.test?x").as_deref(),
        Some("https://a.test")
    );
    assert_eq!(
        origin_of("http://[::1]:8080/").as_deref(),
        Some("http://[::1]:8080")
    );
}

#[test]
fn no_origin_for_other_schemes_or_broken_urls() {
    for url in [
        "about:blank#rb-view-1",
        "rbrowser://ui/index.html",
        "devtools://devtools/x",
        "file:///C:/x.html",
        "data:text/html,hi",
        "wss://live.test",
        "ftp://a.b",
        "http://",
        "https:///nohost",
        "",
    ] {
        assert_eq!(origin_of(url), None, "{url}");
    }
}

#[test]
fn an_origin_is_its_own_origin() {
    for origin in [
        "https://example.com",
        "http://localhost:3000",
        "http://[::1]:8080",
    ] {
        assert_eq!(origin_of(origin).as_deref(), Some(origin));
    }
}
