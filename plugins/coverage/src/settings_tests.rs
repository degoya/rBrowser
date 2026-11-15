use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::{Settings, glob, origin, patterns};
use crate::model::Mode;

#[test]
fn defaults_hold_without_settings() {
    let mut host = MemoryHost::new();
    assert_eq!(Settings::read(&mut host), Settings::default());
    assert_eq!(Settings::default().min_bytes, 10_240);
    assert_eq!(Settings::default().unused_percent, 50);
}

#[test]
fn values_are_held_to_the_schemas_bounds() {
    let mut host = MemoryHost::new();
    for (key, value) in [
        ("minBytes", "500"),
        ("unusedPercent", "250"),
        ("defaultMode", "\"interaction\""),
        ("thirdParty", "false"),
        ("ignore", "[\"*/vendor/*\", \"\", 3]"),
        ("idleMs", "5"),
        ("loadTimeoutSeconds", "\"x\""),
    ] {
        host.settings.insert(key.to_owned(), value.to_owned());
    }
    let settings = Settings::read(&mut host);
    assert_eq!(settings.min_bytes, 500);
    assert_eq!(settings.unused_percent, 100);
    assert_eq!(settings.default_mode, Mode::Interaction);
    assert!(!settings.third_party);
    assert_eq!(settings.ignore, vec!["*/vendor/*".to_owned()]);
    assert_eq!(settings.idle_ms, 100);
    assert_eq!(settings.load_timeout_seconds, 30);
}

#[test]
fn globs_match_any_characters_for_a_star() {
    assert!(glob("*/vendor/*", "http://a/vendor/x.js"));
    assert!(glob("*.min.css", "http://a/b.min.css"));
    assert!(glob("http://a/*.css", "http://a/x/y.css"));
    assert!(!glob("*.min.css", "http://a/b.css"));
    assert!(glob("exact", "exact"));
    assert!(!glob("ab*ba", "aba"));
    assert_eq!(patterns(&json!("x")), Vec::<String>::new());
}

#[test]
fn third_party_files_are_those_of_another_origin() {
    let mut settings = Settings::default();
    let page = "http://localhost:8080/coverage/index.html";
    assert!(!settings.skips("https://cdn.example/a.css", page));
    settings.third_party = false;
    assert!(settings.skips("https://cdn.example/a.css", page));
    assert!(!settings.skips("http://localhost:8080/coverage/a.css", page));
    assert_eq!(
        origin("HTTP://User@Host:1/x?y").as_deref(),
        Some("http://host:1")
    );
    assert_eq!(origin("data:text/css,a"), None);
}
