use super::follows;

#[test]
fn a_window_follows_page_urls_it_is_not_at() {
    assert!(follows("http://localhost/", "http://localhost/about"));
    assert!(follows("", "https://example.test/"));
    assert!(follows("http://a/", "file:///C:/site/index.html"));
    assert!(!follows("http://localhost/about", "http://localhost/about"));
}

#[test]
fn internal_pages_are_never_followed() {
    assert!(!follows("http://a/", "about:blank#rb-view-3"));
    assert!(!follows("http://a/", "about:blank"));
    assert!(!follows("http://a/", "chrome://version"));
    assert!(!follows(
        "http://a/",
        "devtools://devtools/bundled/inspector.html"
    ));
}
