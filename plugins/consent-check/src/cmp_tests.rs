use super::*;

#[test]
fn the_bundled_data_parses_and_names_its_source() {
    let data = data();
    assert_eq!(data.version, 1);
    assert!(data.cmps.len() >= 10);
    for cmp in &data.cmps {
        assert!(
            data.sources.iter().any(|source| source.id == cmp.source),
            "{} names a listed source",
            cmp.id
        );
        assert!(!cmp.banner.is_empty(), "{} has a banner selector", cmp.id);
    }
    assert!(data.reject_texts.iter().any(|text| text == "alle ablehnen"));
}

#[test]
fn a_known_consent_manager_has_a_name() {
    assert_eq!(name_of("onetrust"), Some("OneTrust"));
    assert_eq!(name_of("nobody"), None);
}

#[test]
fn session_and_consent_cookies_are_essential_others_not() {
    let none: Vec<String> = Vec::new();
    assert!(essential("PHPSESSID", &none));
    assert!(
        essential("OptanonConsent", &none),
        "a consent manager's own state"
    );
    assert!(essential("cmplz_marketing", &none), "a prefix pattern");
    assert!(!essential("_ga", &none));
    assert!(!essential("_track", &none));
    let own = vec!["shop_cart".to_owned(), "pref_*".to_owned()];
    assert!(essential("shop_cart", &own));
    assert!(essential("pref_theme", &own));
    assert!(!essential("shop_cart2", &own));
}

#[test]
fn the_collector_config_carries_selectors_texts_and_the_mode() {
    let config = collector_config(Mode::Read);
    assert_eq!(config["mode"], "read");
    assert!(
        config["cmps"]
            .as_array()
            .is_some_and(|list| !list.is_empty())
    );
    assert_eq!(config["cmps"][0]["id"], "onetrust");
    assert!(config["cmps"][0]["reject"].is_array());
    assert!(
        config["cmps"][0].get("cookies").is_none(),
        "the page needs no cookie names"
    );
    assert!(config["rejectTexts"].is_array());
    assert_eq!(collector_config(Mode::Reject)["mode"], "reject");
}

#[test]
fn the_collector_script_is_one_expression_over_the_config() {
    let script = collector(Mode::Read);
    assert!(!script.contains(CONFIG_SLOT));
    assert!(script.contains("\"mode\":\"read\""));
    assert!(script.trim_end().ends_with(')'));
}
