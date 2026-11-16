use rb_plugin_types::{ViewInfo, codes};

use super::{check, origin};

fn view(url: &str) -> ViewInfo {
    ViewInfo {
        url: url.to_owned(),
        ..ViewInfo::default()
    }
}

#[test]
fn the_origin_is_read_only_from_a_plain_url() {
    assert_eq!(
        origin("https://Shop.example:443/a?b#c").as_deref(),
        Some("https://shop.example")
    );
    assert_eq!(
        origin("http://localhost:3000/").as_deref(),
        Some("http://localhost:3000")
    );
    for odd in [
        "https://127.0.0.1\\@shop.example/",
        "https://user:pass@shop.example/",
        "https://shop.example\n.evil/",
        "https://shop .example/",
        "file:///C:/x.html",
        "about:blank",
        "",
    ] {
        assert_eq!(origin(odd), None, "{odd:?}");
    }
}

#[test]
fn a_run_on_a_views_site_passes_and_any_other_is_refused() {
    let views = [view("http://localhost:3000/shop"), view("about:blank")];
    check(r#"{"url":"http://localhost:3000/other"}"#, &views).expect("the view's site");
    let refused = check(r#"{"url":"http://localhost:3001/"}"#, &views).expect_err("other port");
    assert_eq!(refused.code(), Some(codes::SIDECAR_URL_DENIED));
    assert_eq!(refused.param("url"), Some("http://localhost:3001/"));
    // A view on `about:blank` opens no site.
    assert!(check(r#"{"url":"about:blank"}"#, &views).is_err());
    assert!(check(r#"{"url":"https://example.com/"}"#, &[]).is_err());
}
