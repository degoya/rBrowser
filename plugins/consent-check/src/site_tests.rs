use super::*;

#[test]
fn the_site_is_the_last_two_labels_or_three_below_a_country_second_level() {
    assert_eq!(site_of("www.shop.example.test"), "example.test");
    assert_eq!(site_of("example.test"), "example.test");
    assert_eq!(site_of("cdn.example.co.uk"), "example.co.uk");
    assert_eq!(site_of("127.0.0.1"), "127.0.0.1");
    assert_eq!(site_of("localhost"), "localhost");
}

#[test]
fn a_host_rule_is_a_host_or_a_domain_with_its_subdomains() {
    let rules = host_rules(&["cdn.example.test".to_owned(), "*.assets.test".to_owned()]);
    assert!(allowed("cdn.example.test", &rules));
    assert!(
        allowed("CDN.example.test", &rules),
        "hosts compare without case"
    );
    assert!(
        !allowed("x.cdn.example.test", &rules),
        "a plain host names only itself"
    );
    assert!(allowed("assets.test", &rules));
    assert!(allowed("img.eu.assets.test", &rules));
    assert!(!allowed("badassets.test", &rules));
}

#[test]
fn malformed_and_too_many_host_rules_are_dropped() {
    let mut list: Vec<String> = vec![
        String::new(),
        "*.".to_owned(),
        "has space.test".to_owned(),
        "https://x.test/".to_owned(),
    ];
    list.extend((0..100).map(|n| format!("h{n}.test")));
    let rules = host_rules(&list);
    assert_eq!(rules.len(), MAX_HOST_RULES);
    assert!(rules.iter().all(|rule| !rule.host.is_empty()));
}

#[test]
fn a_request_to_another_site_is_third_party_unless_allowed() {
    let none = host_rules(&[]);
    assert_eq!(
        party("https://static.example.test/a.js", "example.test", &none),
        Party::First
    );
    assert_eq!(
        party("https://tracker.test/t.js?id=1", "example.test", &none),
        Party::Third("tracker.test".to_owned())
    );
    let allow = host_rules(&["tracker.test".to_owned()]);
    assert_eq!(
        party("https://tracker.test/t.js", "example.test", &allow),
        Party::Allowed("tracker.test".to_owned())
    );
    assert_eq!(
        party("data:image/png;base64,AA", "example.test", &none),
        Party::None
    );
    assert_eq!(
        party("blob:https://example.test/x", "example.test", &none),
        Party::None
    );
}

#[test]
fn resource_types_map_to_the_kinds_of_the_findings() {
    assert_eq!(kind_of("Script"), Kind::Script);
    assert_eq!(kind_of("Image"), Kind::Pixel);
    assert_eq!(kind_of("Ping"), Kind::Pixel);
    assert_eq!(kind_of("Font"), Kind::Font);
    assert_eq!(
        kind_of("Document"),
        Kind::Iframe,
        "a third-party document is a frame"
    );
    assert_eq!(kind_of("Stylesheet"), Kind::Style);
    assert_eq!(kind_of("XHR"), Kind::Fetch);
    assert_eq!(kind_of("Fetch"), Kind::Fetch);
    assert_eq!(kind_of("Media"), Kind::Media);
    assert_eq!(kind_of("Manifest"), Kind::Other);
    assert_eq!(Kind::Iframe.as_str(), "iframe");
}

#[test]
fn a_url_shown_in_a_finding_keeps_neither_query_nor_fragment() {
    assert_eq!(
        shown_url("https://tracker.test/p.gif?uid=42&ref=x#y"),
        "https://tracker.test/p.gif"
    );
    let long = format!("https://tracker.test/{}", "a".repeat(400));
    assert_eq!(shown_url(&long).chars().count(), MAX_URL_CHARS);
}
