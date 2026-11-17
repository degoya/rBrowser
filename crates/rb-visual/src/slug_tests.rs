use super::*;

#[test]
fn the_path_is_the_slug_by_default() {
    let cases = [
        ("http://localhost:5173/", "index"),
        ("http://localhost:5173", "index"),
        ("https://example.com/shop/cart", "shop-cart"),
        ("https://example.com/Shop/Cart/?page=2#top", "shop-cart"),
        ("https://example.com/de/%C3%BCber-uns", "de-c3-bcber-uns"),
        ("https://example.com/a//b__c", "a-b-c"),
    ];
    for (url, expected) in cases {
        assert_eq!(slug(url, SlugStrategy::Path), expected, "{url}");
    }
}

#[test]
fn host_path_puts_the_host_in_front_without_port_or_credentials() {
    assert_eq!(
        slug(
            "https://user:secret@staging.example.com:8443/shop",
            SlugStrategy::HostPath
        ),
        "staging-example-com-shop"
    );
    assert_eq!(
        slug("http://[::1]:3000/", SlugStrategy::HostPath),
        "1",
        "an IPv6 host keeps its digits"
    );
}

#[test]
fn path_query_keeps_the_query_but_never_the_fragment() {
    assert_eq!(
        slug("https://example.com/list?page=2#x", SlugStrategy::PathQuery),
        "list-page-2"
    );
    assert_eq!(
        slug("https://example.com/list", SlugStrategy::PathQuery),
        "list"
    );
}

#[test]
fn a_long_slug_is_cut_and_keeps_a_hash_of_the_whole() {
    let long_a = format!("https://example.com/{}", "segment/".repeat(30));
    let long_b = format!("https://example.com/{}x", "segment/".repeat(30));
    let (a, b) = (
        slug(&long_a, SlugStrategy::Path),
        slug(&long_b, SlugStrategy::Path),
    );
    assert!(a.len() <= MAX_SLUG && b.len() <= MAX_SLUG);
    assert_ne!(a, b, "two long pages keep two folders");
    assert!(
        a.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    );
}

#[test]
fn the_strategies_parse_by_their_setting_names() {
    assert_eq!(SlugStrategy::parse("path"), Some(SlugStrategy::Path));
    assert_eq!(
        SlugStrategy::parse("hostPath"),
        Some(SlugStrategy::HostPath)
    );
    assert_eq!(
        SlugStrategy::parse("pathQuery"),
        Some(SlugStrategy::PathQuery)
    );
    assert_eq!(SlugStrategy::parse("custom"), None);
}

#[test]
fn baseline_names_are_plain_file_names() {
    for good in [
        "mobile",
        "desktop-ff",
        "393x852",
        "tablet_landscape",
        "v1.2",
    ] {
        assert!(valid_name(good), "{good}");
    }
    let too_long = "x".repeat(MAX_NAME + 1);
    for bad in [
        "",
        ".hidden",
        "../up",
        "a/b",
        "a\\b",
        "c:",
        too_long.as_str(),
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
}

#[test]
fn every_slug_is_a_valid_name_whatever_its_length() {
    for length in 1..=120 {
        let url = format!("https://example.com/{}", "a".repeat(length));
        for strategy in [
            SlugStrategy::Path,
            SlugStrategy::HostPath,
            SlugStrategy::PathQuery,
        ] {
            let made = slug(&url, strategy);
            assert!(valid_name(&made), "{length}: {made}");
        }
    }
}

#[test]
fn windows_device_names_are_no_names_and_no_slugs() {
    for bad in ["con", "CON", "nul.png", "Lpt9", "com1.meta.json", "aux"] {
        assert!(!valid_name(bad), "{bad}");
    }
    for good in ["console", "com10", "lpt", "con-tact", "_con"] {
        assert!(valid_name(good), "{good}");
    }
    for page in ["con", "PRN", "com3", "nul/"] {
        let made = slug(&format!("https://example.com/{page}"), SlugStrategy::Path);
        assert!(valid_name(&made), "{page}: {made}");
    }
    assert_eq!(slug("https://example.com/aux", SlugStrategy::Path), "_aux");
}

#[test]
fn a_trailing_dot_is_no_name() {
    assert!(!valid_name("mobile."));
}
