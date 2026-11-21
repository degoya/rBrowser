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

#[test]
fn the_longest_heading_is_read_and_held_to_its_bounds() {
    assert_eq!(Settings::default().heading_max_chars, 120);
    let mut host = MemoryHost::new();
    host.settings
        .insert("headingMaxLength".to_owned(), "5".to_owned());
    assert_eq!(
        Settings::read(&mut host).heading_max_chars,
        MIN_HEADING_CHARS
    );
    host.settings
        .insert("headingMaxLength".to_owned(), "90".to_owned());
    assert_eq!(Settings::read(&mut host).heading_max_chars, 90);
    host.settings
        .insert("headingMaxLength".to_owned(), "\"long\"".to_owned());
    assert_eq!(Settings::read(&mut host).heading_max_chars, 120);
}

#[test]
fn the_breakpoint_diff_compares_the_chosen_elements_in_their_sections() {
    let defaults = Settings::default();
    assert_eq!(defaults.diff_elements, DIFF_ELEMENTS.map(str::to_owned));
    assert_eq!(
        defaults.diff_sections(&[]),
        ["focus", "headings", "landmarks"]
    );
    let only_headings = ["headings".to_owned(), "links".to_owned()];
    assert_eq!(defaults.diff_sections(&only_headings), ["headings"]);

    let mut host = MemoryHost::new();
    host.settings.insert(
        "diff.elements".to_owned(),
        r#"["landmarks", "nonsense", "interactive", "landmarks"]"#.to_owned(),
    );
    let settings = Settings::read(&mut host);
    assert_eq!(settings.diff_elements, ["interactive", "landmarks"]);
    assert_eq!(settings.diff_sections(&[]), ["focus", "landmarks"]);

    host.settings
        .insert("diff.elements".to_owned(), "[]".to_owned());
    let none = Settings::read(&mut host);
    assert!(none.diff_elements.is_empty());
    assert!(none.diff_sections(&[]).is_empty(), "nothing compared");

    host.settings
        .insert("diff.elements".to_owned(), "\"headings\"".to_owned());
    assert_eq!(
        Settings::read(&mut host).diff_elements,
        defaults.diff_elements
    );
}

#[test]
fn the_reading_order_tolerance_is_read_and_held_to_its_bounds() {
    assert_eq!(Settings::default().tolerance_px, 4);
    let mut host = MemoryHost::new();
    host.settings
        .insert("readingOrder.tolerancePx".to_owned(), "500".to_owned());
    assert_eq!(Settings::read(&mut host).tolerance_px, MAX_TOLERANCE_PX);
    host.settings
        .insert("readingOrder.tolerancePx".to_owned(), "0".to_owned());
    assert_eq!(Settings::read(&mut host).tolerance_px, 0);
    host.settings
        .insert("readingOrder.tolerancePx".to_owned(), "-3".to_owned());
    assert_eq!(Settings::read(&mut host).tolerance_px, 4);
}
