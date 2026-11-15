use super::*;

fn url(text: &str) -> Url {
    Url::parse(text).expect("a URL")
}

#[test]
fn parse_takes_http_and_https_only() {
    assert_eq!(
        url("https://Example.COM/a?b=1#top").to_string(),
        "https://example.com/a?b=1"
    );
    assert_eq!(url("http://example.com").to_string(), "http://example.com/");
    assert!(Url::parse("ftp://example.com/").is_none());
    assert!(Url::parse("file:///etc/passwd").is_none());
    assert!(Url::parse("data:image/png;base64,AAAA").is_none());
    assert!(Url::parse("/relative").is_none());
}

#[test]
fn parse_refuses_credentials_white_space_and_long_urls() {
    assert!(Url::parse("https://user:secret@example.com/").is_none());
    assert!(Url::parse("https://exa mple.com/").is_none());
    assert!(Url::parse("https:///path").is_none());
    let long = format!("https://example.com/{}", "a".repeat(MAX_URL_CHARS));
    assert!(Url::parse(&long).is_none());
}

#[test]
fn parse_drops_the_default_port() {
    assert_eq!(url("https://a.test:443/x").origin(), "https://a.test");
    assert_eq!(url("http://a.test:80/x").origin(), "http://a.test");
    assert_eq!(url("http://a.test:8080/x").origin(), "http://a.test:8080");
}

#[test]
fn join_resolves_every_kind_of_reference() {
    let base = url("https://a.test/app/index.html");
    let joined = |reference: &str| base.join(reference).expect("joined").to_string();
    assert_eq!(joined("https://b.test/x"), "https://b.test/x");
    assert_eq!(joined("//c.test/y"), "https://c.test/y");
    assert_eq!(joined("/root"), "https://a.test/root");
    assert_eq!(joined("icons/a.png"), "https://a.test/app/icons/a.png");
    assert_eq!(joined("./"), "https://a.test/app/");
    assert_eq!(joined("../up"), "https://a.test/up");
    assert_eq!(joined("?pwa=1"), "https://a.test/app/index.html?pwa=1");
    assert_eq!(joined(""), "https://a.test/app/index.html");
    assert!(base.join("javascript:alert(1)").is_none());
}

#[test]
fn same_origin_compares_scheme_host_and_port() {
    let page = url("http://localhost:8080/pwa-ok/");
    assert!(page.same_origin(&url("http://localhost:8080/x.png")));
    assert!(!page.same_origin(&url("http://localhost:8081/x.png")));
    assert!(!page.same_origin(&url("https://localhost:8080/x.png")));
    assert!(!page.same_origin(&url("http://127.0.0.1:8080/x.png")));
}

#[test]
fn within_takes_the_scope_as_a_path_prefix_of_the_same_origin() {
    let scope = url("https://a.test/app/");
    assert!(url("https://a.test/app/").within(&scope));
    assert!(url("https://a.test/app/start?pwa=1").within(&scope));
    assert!(!url("https://a.test/other/").within(&scope));
    assert!(!url("https://a.test/ap").within(&scope));
    assert!(!url("https://b.test/app/").within(&scope));
}
