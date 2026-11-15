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
    assert!(Url::parse("javascript:alert(1)").is_none());
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
fn parse_resolves_dot_segments() {
    assert_eq!(url("https://a.test/x/../y/./z").path(), "/y/z");
    assert_eq!(url("https://a.test/../../etc").path(), "/etc");
    assert_eq!(url("https://a.test/x/y/..").path(), "/x/");
}

#[test]
fn join_resolves_every_kind_of_reference() {
    let base = url("https://a.test/dir/sitemap.xml");
    assert_eq!(
        base.join("https://b.test/x").expect("abs").to_string(),
        "https://b.test/x"
    );
    assert_eq!(
        base.join("//c.test/y").expect("scheme").to_string(),
        "https://c.test/y"
    );
    assert_eq!(
        base.join("/root.xml").expect("path").to_string(),
        "https://a.test/root.xml"
    );
    assert_eq!(
        base.join("child.xml").expect("rel").to_string(),
        "https://a.test/dir/child.xml"
    );
    assert_eq!(
        base.join("?page=2").expect("query").to_string(),
        "https://a.test/dir/sitemap.xml?page=2"
    );
    assert!(base.join("ftp://a.test/").is_none());
    assert!(base.join("data:text/html,x").is_none());
}

#[test]
fn origin_keeps_the_path_apart() {
    let page = url("https://www.example.com/shop/cart?x=1");
    assert_eq!(page.origin(), "https://www.example.com");
    assert_eq!(page.path_and_query(), "/shop/cart?x=1");
}

#[test]
fn loopback_private_and_single_label_hosts_are_local() {
    for local in [
        "http://localhost:5173/",
        "http://app.localhost/",
        "http://printer.local/",
        "http://intranet/",
        "http://127.0.0.1/",
        "http://127.8.9.10:8080/",
        "http://10.0.0.5/",
        "http://172.16.0.1/",
        "http://172.31.255.255/",
        "http://192.168.1.1/",
        "http://169.254.169.254/latest/meta-data",
        "http://100.64.0.1/",
        "http://0.0.0.0/",
        "http://[::1]:8080/",
        "http://[fd00::1]/",
        "http://[fe80::1]/",
        "http://[::ffff:127.0.0.1]/",
        "http://2130706433/",
        "http://0x7f.0.0.1/",
        "http://127.1/",
    ] {
        assert!(url(local).is_local(), "{local}");
    }
    for public in [
        "https://www.example.com/",
        "https://example.com:8443/",
        "http://172.32.0.1/",
        "http://8.8.8.8/",
        "http://[2001:db8::1]/",
        "https://1password.com/",
    ] {
        assert!(!url(public).is_local(), "{public}");
    }
}
