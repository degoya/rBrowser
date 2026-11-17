use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

#[test]
fn without_settings_the_defaults_apply() {
    let mut host = MemoryHost::new();
    assert_eq!(Config::read(&mut host, &json!({})), Config::default());
}

#[test]
fn settings_are_read_by_their_property_path_and_held_to_their_range() {
    let mut host = MemoryHost::new();
    for (key, value) in [
        ("maxPages", "25"),
        ("title.maxLength", "70"),
        ("description.minLength", "-3"),
        ("images.maxAltLength", "\"long\""),
        ("images.placeholders", "[\" Pic \", \"\", 3]"),
        (
            "rules",
            "{\"images\": \"off\", \"meta.title_length\": \"error\", \"x\": \"loud\"}",
        ),
        ("ignore", "[{\"url\": \"*/legacy/*\"}]"),
    ] {
        host.settings.insert(key.to_owned(), value.to_owned());
    }
    let config = Config::read(&mut host, &json!({}));
    assert_eq!(config.max_pages, 25);
    assert_eq!(config.title_max, 70);
    assert_eq!(config.description_min, 70, "out of range: the default");
    assert_eq!(config.max_alt_length, 125, "no number: the default");
    assert_eq!(config.placeholders, ["pic"]);
    assert_eq!(config.level("images.alt_missing"), Some(Level::Off));
    assert_eq!(
        config.level("meta.title_length"),
        Some(Level::Severity(Severity::Error))
    );
    assert_eq!(config.level("x"), None, "no level: no override");
    assert!(config.ignores("meta.title_missing", Some("https://a.test/legacy/x"), None));
    assert!(!config.ignores("meta.title_missing", Some("https://a.test/new/x"), None));
}

#[test]
fn the_arguments_win_and_the_crawl_stays_bounded() {
    let mut host = MemoryHost::new();
    host.settings.insert("maxPages".to_owned(), "5".to_owned());
    assert_eq!(
        Config::read(&mut host, &json!({"maxPages": 3})).max_pages,
        3
    );
    assert_eq!(
        Config::read(&mut host, &json!({"maxPages": 100000})).max_pages,
        MAX_PAGES
    );
}

#[test]
fn an_empty_ignore_pattern_ignores_nothing() {
    let config = Config {
        ignore: vec![IgnorePattern::default()],
        ..Config::default()
    };
    assert!(!config.ignores("meta.title_missing", None, None));
}

#[test]
fn glob_matches_the_whole_text() {
    assert!(glob("images.*", "images.alt_missing"));
    assert!(!glob("images.*", "meta.images"));
    assert!(glob("*hero*", "html > body > img#hero"));
    assert!(glob("a?c", "abc"));
    assert!(!glob("a?c", "abbc"));
    assert!(glob("*", ""));
    assert!(glob("a*b*c", "aXbYbZc"));
}

#[test]
fn the_language_set_and_parity_settings_are_read_and_bounded() {
    let mut host = MemoryHost::new();
    let config = Config::read(&mut host, &json!({}));
    assert!(config.locales.is_empty());
    assert!(config.require_x_default);
    assert!(!config.check_content_language);
    assert_eq!(config.parity_reference, SMALLEST);
    assert!((config.text_similarity - 0.9).abs() < f64::EPSILON);
    for (key, value) in [
        ("locales", r#"["de", " en-GB ", "", 3]"#),
        ("requireXDefault", "false"),
        ("hreflang.checkContentLanguage", "true"),
        ("parity.reference", r#""desktop""#),
        ("parity.textSimilarity", "0.5"),
    ] {
        host.settings.insert(key.to_owned(), value.to_owned());
    }
    let config = Config::read(&mut host, &json!({}));
    assert_eq!(config.locales, ["de", "en-GB"]);
    assert!(!config.require_x_default);
    assert!(config.check_content_language);
    assert_eq!(config.parity_reference, "desktop");
    assert!((config.text_similarity - 0.5).abs() < f64::EPSILON);
    host.settings
        .insert("parity.textSimilarity".to_owned(), "1.5".to_owned());
    let config = Config::read(&mut host, &json!({}));
    assert!((config.text_similarity - 0.9).abs() < f64::EPSILON);
}

#[test]
fn the_groups_of_rb_080_11_are_switched_and_sized_by_their_settings() {
    let mut host = MemoryHost::new();
    for (key, value) in [
        ("wellknown.enabled", "false"),
        ("head.enabled", "true"),
        ("social.platforms", "[\"x\", \"myspace\", \"discord\"]"),
        ("social.imageMinWidth", "600"),
        ("social.imageMinHeight", "0"),
        ("wellknown.maxLinks", "500"),
    ] {
        host.settings.insert(key.to_owned(), value.to_owned());
    }
    let config = Config::read(&mut host, &json!({}));
    assert_eq!(config.groups_off, ["wellknown"]);
    assert_eq!(config.level("wellknown.llms_h1"), Some(Level::Off));
    assert_eq!(config.level("head.title_late"), None);
    assert_eq!(config.social_platforms, ["x", "discord"]);
    assert_eq!(
        (config.image_min_width, config.image_min_height),
        (600, 630)
    );
    assert_eq!(config.llms_max_links, 20, "out of range: the default");
    let default = Config::default();
    assert_eq!(
        default.social_platforms,
        [
            "facebook", "x", "linkedin", "slack", "whatsapp", "discord", "imessage"
        ]
    );
    assert!(default.groups_off.is_empty());
}
