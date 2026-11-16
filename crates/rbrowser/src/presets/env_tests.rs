use super::*;

fn env(id: &str, url: &str, color: Option<&str>) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: color.map(str::to_owned),
        reload_interval_sec: None,
    }
}

#[test]
fn switching_keeps_path_query_and_hash_and_changes_only_the_origin() {
    assert_eq!(
        rewrite_url(
            "http://localhost:3000/products/42?x=1#top",
            Some("http://localhost:3000"),
            "https://staging.example.com"
        ),
        "https://staging.example.com/products/42?x=1#top"
    );
}

#[test]
fn an_origin_with_a_trailing_slash_does_not_double_it() {
    assert_eq!(
        rewrite_url(
            "http://localhost:3000/a?b#c",
            None,
            "https://staging.example.com/"
        ),
        "https://staging.example.com/a?b#c"
    );
}

#[test]
fn a_base_path_of_the_old_environment_is_swapped_for_the_new_one() {
    assert_eq!(
        rewrite_url(
            "https://example.com/preview/shop/cart?step=2",
            Some("https://example.com/preview"),
            "https://example.com/live"
        ),
        "https://example.com/live/shop/cart?step=2"
    );
    // A path that only starts with the same letters is not under the base path.
    assert_eq!(
        rewrite_url(
            "https://example.com/previewer/x",
            Some("https://example.com/preview"),
            "https://other.test"
        ),
        "https://other.test/previewer/x"
    );
}

#[test]
fn a_page_outside_the_web_opens_the_environment_itself() {
    for current in ["about:blank", "", "data:text/html,hi", "rbrowser://ui/x"] {
        assert_eq!(
            rewrite_url(current, None, "https://staging.example.com"),
            "https://staging.example.com",
            "{current}"
        );
    }
}

#[test]
fn the_origin_compares_without_case_and_keeps_a_bare_host() {
    assert_eq!(
        rewrite_url(
            "HTTP://LocalHost:3000",
            Some("http://localhost:3000"),
            "https://s.test"
        ),
        "https://s.test"
    );
    assert_eq!(
        rewrite_url("http://localhost:3000?q=1", None, "https://s.test"),
        "https://s.test?q=1"
    );
}

#[test]
fn the_start_environment_is_the_one_the_start_url_belongs_to() {
    let envs = [
        env("local", "http://localhost:3000", None),
        env("staging", "https://staging.example.com", Some("#f9a825")),
    ];
    assert_eq!(
        initial_environment(&envs, Some("https://staging.example.com/x")).map(|e| e.id.as_str()),
        Some("staging")
    );
    assert_eq!(
        initial_environment(&envs, Some("https://elsewhere.test/")).map(|e| e.id.as_str()),
        Some("local")
    );
    assert_eq!(
        initial_environment(&envs, None).map(|e| e.id.as_str()),
        Some("local")
    );
    assert!(initial_environment(&[], Some("https://x.test")).is_none());
}

#[test]
fn a_frame_colour_is_hash_and_six_hex_digits() {
    assert_eq!(frame_argb("#f9a825"), Some(0xFFF9_A825));
    assert_eq!(frame_argb("#000000"), Some(0xFF00_0000));
    for bad in ["f9a825", "#f9a82", "#f9a8255", "#g9a825", "", "#+12345"] {
        assert_eq!(frame_argb(bad), None, "{bad}");
    }
}

#[test]
fn environments_cycle_in_config_order() {
    let envs = [
        env("local", "http://localhost", None),
        env("staging", "https://s.test", None),
        env("prod", "https://p.test", None),
    ];
    assert_eq!(next_environment(&envs, Some("local")), Some("staging"));
    assert_eq!(next_environment(&envs, Some("prod")), Some("local"));
    assert_eq!(next_environment(&envs, None), Some("local"));
    assert_eq!(next_environment(&envs, Some("gone")), Some("local"));
    assert_eq!(next_environment(&[], None), None);
}

#[test]
fn only_http_pages_are_web_pages() {
    assert!(is_web_url("https://example.com"));
    assert!(is_web_url("http://localhost:3000/x"));
    assert!(!is_web_url("about:blank"));
    assert!(!is_web_url(""));
    assert!(!is_web_url("file:///C:/x.html"));
}
