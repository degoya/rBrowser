use super::*;

#[test]
fn a_missing_scheme_is_completed() {
    assert_eq!(
        normalize_url("example.com").expect("ok"),
        "https://example.com"
    );
    assert_eq!(
        normalize_url(" localhost:3000/a?b#c ").expect("ok"),
        "http://localhost:3000/a?b#c"
    );
    assert_eq!(
        normalize_url("127.0.0.1:18765/grid.html").expect("ok"),
        "http://127.0.0.1:18765/grid.html"
    );
    assert_eq!(normalize_url("shop.test").expect("ok"), "http://shop.test");
}

#[test]
fn allowed_schemes_pass_unchanged() {
    for url in [
        "http://a.b",
        "HTTPS://a.b/c",
        "about:blank",
        "chrome://sandbox",
        "file:///C:/site/index.html",
        "data:text/html,hi",
    ] {
        assert_eq!(normalize_url(url).expect("ok"), url);
    }
}

#[test]
fn script_and_shell_schemes_and_junk_are_refused() {
    for url in [
        "javascript:alert(1)",
        "rbrowser://ui/index.html",
        "",
        "   ",
        "a b",
    ] {
        assert!(
            matches!(normalize_url(url), Err(EngineError::Invalid(_))),
            "{url} was accepted"
        );
    }
}

#[test]
fn marker_urls_name_their_view() {
    let url = marker_url(ViewId(12));
    assert_eq!(url, "about:blank#rb-view-12");
    assert_eq!(marker_view(&url), Some(ViewId(12)));
    assert_eq!(marker_view("about:blank"), None);
    assert_eq!(marker_view("about:blank#rb-view-x"), None);
}
