use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn without_settings_every_value_is_its_default() {
    let mut host = MemoryHost::new();
    let settings = page_settings(&mut host);
    assert_eq!(settings.mutation_throttle_ms, DEFAULT_THROTTLE_MS);
    assert!(settings.show100vw);
    assert!(!settings.containers_only);
    assert_eq!(settings.colors.cause, DEFAULT_CAUSE);
    assert!(restore_on_start(&mut host));
}

#[test]
fn the_settings_are_read_by_their_own_key_or_with_the_plugin_id() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("overflow.mutationThrottleMs".to_owned(), "1000".to_owned());
    host.settings.insert(
        "overlays.outlines.containersOnly".to_owned(),
        "true".to_owned(),
    );
    host.settings
        .insert("colors.highlight".to_owned(), "\"#00aa00\"".to_owned());
    host.settings
        .insert("restoreOnStart".to_owned(), "false".to_owned());
    let settings = page_settings(&mut host);
    assert_eq!(settings.mutation_throttle_ms, 1000);
    assert!(settings.containers_only);
    assert_eq!(settings.colors.highlight, "#00aa00");
    assert!(!restore_on_start(&mut host));
}

#[test]
fn a_value_outside_its_schema_is_the_default() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("overflow.mutationThrottleMs".to_owned(), "5".to_owned());
    host.settings.insert(
        "colors.cause".to_owned(),
        "\"red; background:url(x)\"".to_owned(),
    );
    host.settings
        .insert("overflow.show100vw".to_owned(), "\"no\"".to_owned());
    let settings = page_settings(&mut host);
    assert_eq!(settings.mutation_throttle_ms, DEFAULT_THROTTLE_MS);
    assert_eq!(settings.colors.cause, DEFAULT_CAUSE);
    assert!(settings.show100vw);
}

#[test]
fn a_colour_is_hash_and_six_hex_digits() {
    assert!(is_color("#a0B1c2"));
    assert!(!is_color("#abc"));
    assert!(!is_color("a0b1c2f"));
    assert!(!is_color("#ggg000"));
    assert!(!is_color("#\u{e9}0000"));
}

#[test]
fn the_further_overlays_take_their_settings_within_bounds() {
    let mut host = MemoryHost::new();
    let defaults = page_settings(&mut host);
    assert_eq!(defaults.target_size_px, 24);
    assert_eq!(defaults.small_text_px, 12);
    assert_eq!(defaults.scrollbar_tolerance_px, 4);
    assert!((defaults.chaos_factor - 2.0).abs() < f64::EPSILON);
    assert!(!defaults.css_off_include_inline);
    assert!(report_findings(&mut host));

    host.settings
        .insert("targetSizePx".to_owned(), "44".to_owned());
    host.settings
        .insert("overlays.smallTextPx".to_owned(), "14".to_owned());
    host.settings
        .insert("scrollbarTolerancePx".to_owned(), "500".to_owned());
    host.settings
        .insert("chaos.factor".to_owned(), "0.5".to_owned());
    host.settings
        .insert("cssOff.includeInline".to_owned(), "true".to_owned());
    host.settings
        .insert("reportFindings".to_owned(), "false".to_owned());
    let settings = page_settings(&mut host);
    assert_eq!(settings.target_size_px, 44);
    assert_eq!(settings.small_text_px, 14);
    assert_eq!(settings.scrollbar_tolerance_px, 4);
    assert!((settings.chaos_factor - 0.5).abs() < f64::EPSILON);
    assert!(settings.css_off_include_inline);
    assert!(!report_findings(&mut host));
    let json = serde_json::to_value(&settings).expect("JSON");
    assert_eq!(json["targetSizePx"], serde_json::json!(44));
    assert_eq!(json["cssOffIncludeInline"], serde_json::json!(true));
}

#[test]
fn the_tab_order_settings_are_on_unless_set_and_reach_the_page_in_camel_case() {
    let mut host = MemoryHost::new();
    let defaults = page_settings(&mut host);
    assert!(
        defaults.tab_order_show_lines
            && defaults.tab_order_live
            && defaults.tab_order_include_iframes
    );
    host.settings
        .insert("overlays.tabOrder.showLines".to_owned(), "false".to_owned());
    host.settings
        .insert("tabOrder.includeIframes".to_owned(), "false".to_owned());
    host.settings
        .insert("tabOrder.live".to_owned(), "\"off\"".to_owned());
    let page = serde_json::to_value(page_settings(&mut host)).expect("JSON");
    assert_eq!(page["tabOrderShowLines"], serde_json::json!(false));
    assert_eq!(page["tabOrderIncludeIframes"], serde_json::json!(false));
    assert_eq!(page["tabOrderLive"], serde_json::json!(true));
}

#[test]
fn every_colour_setting_is_a_colour_field_with_a_hex_default() {
    // RB-090-58: `format: "color"` gives the key the settings window's colour picker.
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("the schema");
    let colors = schema["properties"]["colors"]["properties"]
        .as_object()
        .expect("the colours");
    assert_eq!(colors.len(), 4);
    for (key, field) in colors {
        assert_eq!(field["format"], "color", "colors.{key}");
        assert_eq!(field["pattern"], "^#[0-9a-fA-F]{6}$", "colors.{key}");
        let default = field["default"].as_str().expect("a default");
        assert!(
            default.len() == 7
                && default.starts_with('#')
                && default[1..].chars().all(|c| c.is_ascii_hexdigit()),
            "colors.{key}: {default}"
        );
    }
}
