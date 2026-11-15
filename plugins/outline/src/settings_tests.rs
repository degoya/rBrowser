use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

#[test]
fn the_defaults_hold_without_settings() {
    let mut host = MemoryHost::new();
    assert_eq!(Settings::read(&mut host), Settings::default());
    assert_eq!(Settings::default().rps, 2);
    assert!(Settings::default().link_check);
}

#[test]
fn values_are_read_and_held_to_their_bounds() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("linkCheck.rps".to_owned(), "40".to_owned());
    host.settings
        .insert("linkCheck.timeoutMs".to_owned(), "10".to_owned());
    host.settings
        .insert("linkCheck.enabled".to_owned(), "false".to_owned());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.rps, MAX_RPS);
    assert_eq!(settings.timeout_ms, 1_000);
    assert!(!settings.link_check);
}

#[test]
fn custom_selectors_keep_only_well_formed_entries() {
    let value = json!([
        { "label": "CMS blocks", "selector": "[data-cms-block]" },
        { "label": "", "selector": "main" },
        { "label": "Bad", "selector": "a\u{0}b" },
        { "selector": "nav" },
        "nonsense",
        { "label": "Long", "selector": "x".repeat(MAX_SELECTOR_CHARS + 1) }
    ]);
    assert_eq!(
        custom_selectors(&value),
        vec![CustomSelector {
            label: "CMS blocks".to_owned(),
            selector: "[data-cms-block]".to_owned(),
        }]
    );
    let many = json!(vec![
        json!({ "label": "x", "selector": "p" });
        MAX_CUSTOM + 5
    ]);
    assert_eq!(custom_selectors(&many).len(), MAX_CUSTOM);
}
