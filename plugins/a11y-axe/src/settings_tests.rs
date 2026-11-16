use std::collections::BTreeMap;

use rb_plugin_api::FailureKind;

use super::*;
use crate::fake::FakeHost;

fn read(host: FakeHost) -> Result<Settings, Failure> {
    let mut host = host;
    Settings::read(&mut host)
}

#[test]
fn without_values_the_schema_defaults_hold() {
    let settings = read(FakeHost::default()).expect("defaults");
    assert_eq!(settings.tags, ["wcag22aa"]);
    assert!(settings.rules.is_empty());
    assert!(settings.include.is_empty() && settings.exclude.is_empty());
    assert!(!settings.report_incomplete);
}

#[test]
fn the_defaults_are_the_schema_defaults() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("the schema is JSON");
    let properties = &schema["properties"];
    assert_eq!(
        properties["tags"]["default"],
        serde_json::json!(DEFAULT_TAGS)
    );
    assert_eq!(properties["reportIncomplete"]["default"], false);
    for key in ["tags", "rules", "include", "exclude", "reportIncomplete"] {
        let property = &properties[key];
        assert_eq!(property["x-category"], "checks", "{key}");
        assert_eq!(property["x-section"], "accessibility", "{key}");
        assert!(property["x-order"].is_u64(), "{key}");
    }
}

#[test]
fn set_values_are_read() {
    let settings = read(
        FakeHost::default()
            .set("tags", r#"["wcag2a","best-practice"]"#)
            .set("rules", r#"{"color-contrast": false}"#)
            .set("include", r#"["main"]"#)
            .set("exclude", r##"["#ads"]"##)
            .set("reportIncomplete", "true"),
    )
    .expect("settings");
    assert_eq!(settings.tags, ["wcag2a", "best-practice"]);
    assert_eq!(
        settings.rules,
        BTreeMap::from([("color-contrast".to_owned(), false)])
    );
    assert_eq!(settings.include, ["main"]);
    assert_eq!(settings.exclude, ["#ads"]);
    assert!(settings.report_incomplete);
}

#[test]
fn a_value_that_does_not_fit_names_its_key() {
    for (key, value) in [
        ("tags", r#""wcag2a""#),
        ("tags", r#"["wcag 2a"]"#),
        ("rules", r#"{"bad id!": true}"#),
        ("include", r#"[""]"#),
        ("exclude", "[\"a\\u0000b\"]"),
        ("reportIncomplete", r#""yes""#),
    ] {
        let failure = read(FakeHost::default().set(key, value)).expect_err(key);
        assert_eq!(failure.kind, FailureKind::BadData);
        assert_eq!(
            failure.code(),
            Some("plugin.a11y-axe.errors.bad_setting"),
            "{key}"
        );
        assert_eq!(failure.param("key"), Some(key));
    }
}

#[test]
fn too_many_selectors_are_refused() {
    let many = serde_json::to_string(&vec!["div"; MAX_ITEMS + 1]).expect("json");
    assert!(read(FakeHost::default().set("exclude", &many)).is_err());
    let long = serde_json::to_string(&vec!["a".repeat(MAX_SELECTOR + 1)]).expect("json");
    assert!(read(FakeHost::default().set("include", &long)).is_err());
}
