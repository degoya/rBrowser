use rb_plugin_types::codes;

use super::{check_locale, check_settings_schema};
use crate::refusal::refusal_code;

#[test]
fn a_locale_holds_keys_below_its_plugin_only() {
    let good =
        br#"{"plugin": {"seo": {"name": "SEO", "commands": {"check": "Run the SEO check"}}}}"#;
    assert_eq!(check_locale("seo", good).expect("valid"), 2);
    for bad in [
        &br#"{"toolbar": {"reload": "Override"}}"#[..],
        br#"{"plugin": {"other": {"name": "Another plugin's"}}}"#,
        br#"{"plugin": {"seo": {"name": 3}}}"#,
        br#"{"plugin": {"seo": {"name": ""}}}"#,
        br#"["not", "an", "object"]"#,
        b"not json",
    ] {
        assert!(
            check_locale("seo", bad).is_err(),
            "{}",
            String::from_utf8_lossy(bad)
        );
    }
}

fn schema(category: &str, section: &str) -> String {
    format!(
        r#"{{"type": "object", "properties": {{
            "titleLength": {{"type": "integer", "default": 60,
                "x-category": "{category}", "x-section": "{section}", "x-order": 1,
                "x-scope": ["project"]}}
        }}}}"#
    )
}

#[test]
fn a_schema_in_an_existing_category_passes() {
    let parsed = check_settings_schema(schema("checks", "seo").as_bytes()).expect("valid");
    assert!(parsed["properties"]["titleLength"].is_object());
}

#[test]
fn a_new_top_level_category_is_refused() {
    let error =
        check_settings_schema(schema("seo-tools", "seo").as_bytes()).expect_err("new category");
    assert_eq!(refusal_code(&error), codes::SETTINGS_CATEGORY);
    assert!(format!("{error:#}").contains("seo-tools"));
}

#[test]
fn an_unknown_section_or_a_missing_annotation_is_refused() {
    let error =
        check_settings_schema(schema("checks", "nowhere").as_bytes()).expect_err("unknown section");
    assert_eq!(refusal_code(&error), codes::SETTINGS_SECTION);
    let bare = br#"{"properties": {"a": {"type": "boolean", "x-category": "checks", "x-section": "seo"}}}"#;
    let error = check_settings_schema(bare).expect_err("no x-order");
    assert_eq!(refusal_code(&error), codes::SETTINGS_ANNOTATION);
}

#[test]
fn nested_groups_are_checked_at_their_leaves() {
    let nested = br#"{"properties": {"group": {"type": "object", "properties": {
        "leaf": {"type": "boolean", "x-category": "plugins", "x-section": "installed", "x-order": 2,
            "x-scope": ["user"]}
    }}}}"#;
    check_settings_schema(nested).expect("valid nested schema");
}
