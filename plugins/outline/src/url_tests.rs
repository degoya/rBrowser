use super::*;

#[test]
fn a_fragment_is_dropped() {
    assert_eq!(without_fragment("https://a.test/x#top"), "https://a.test/x");
    assert_eq!(without_fragment("https://a.test/x"), "https://a.test/x");
}

#[test]
fn only_plain_http_urls_are_checked() {
    assert!(is_checkable("https://example.test/a?b=1"));
    assert!(is_checkable("http://localhost:3000/"));
    assert!(!is_checkable("mailto:someone@example.test"));
    assert!(!is_checkable("javascript:void(0)"));
    assert!(!is_checkable("https://user:secret@example.test/"));
    assert!(!is_checkable("https:///nohost"));
    assert!(!is_checkable("https://example.test/a b"));
}

#[test]
fn local_hosts_are_recognised() {
    assert!(is_local("http://localhost:8080/"));
    assert!(is_local("http://127.0.0.1/"));
    assert!(is_local("http://192.168.1.4/"));
    assert!(is_local("http://[::1]:3000/"));
    assert!(is_local("http://printer.local/"));
    assert!(!is_local("https://example.test/"));
    assert!(!is_local("https://8.8.8.8/"));
}

#[test]
fn a_location_is_joined_to_its_base() {
    let base = "https://a.test/docs/page.html?x=1";
    assert_eq!(join(base, "/new").as_deref(), Some("https://a.test/new"));
    assert_eq!(
        join(base, "other.html").as_deref(),
        Some("https://a.test/docs/other.html")
    );
    assert_eq!(
        join(base, "//b.test/x").as_deref(),
        Some("https://b.test/x")
    );
    assert_eq!(
        join(base, "https://c.test/y#frag").as_deref(),
        Some("https://c.test/y")
    );
    assert_eq!(join(base, "ftp://c.test/"), None);
    assert_eq!(join(base, "  "), None);
}
