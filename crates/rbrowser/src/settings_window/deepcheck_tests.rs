use serde_json::json;

use super::*;

fn key_schema(key: &str) -> Value {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    registry.get(key).expect("a known key").schema.clone()
}

fn paths(issues: &[SettingIssue]) -> Vec<(&str, &str)> {
    issues
        .iter()
        .map(|issue| (issue.path.as_str(), issue.message_key.as_str()))
        .collect()
}

#[test]
fn a_negative_breakpoint_width_is_named_at_its_field() {
    let schema = key_schema("breakpoints");
    let value = json!([
        { "id": "phone", "width": 375, "height": 800 },
        { "id": "tablet", "width": -5, "height": 1024 }
    ]);
    assert_eq!(
        paths(&check(&schema, &value)),
        [("/1/width", reason::MINIMUM)]
    );
}

#[test]
fn a_missing_required_field_and_a_wrong_type_are_named() {
    let schema = key_schema("environments");
    let value = json!([{ "id": "local" }, { "id": 3, "url": "https://example.test" }]);
    assert_eq!(
        paths(&check(&schema, &value)),
        [("/0/url", reason::REQUIRED), ("/1/id", reason::TYPE)]
    );
}

#[test]
fn enums_consts_and_nullable_choices_are_held_to_their_values() {
    let theme = key_schema("ui.theme");
    assert!(check(&theme, &json!("dark")).is_empty());
    assert_eq!(paths(&check(&theme, &json!("pink"))), [("", reason::ENUM)]);
    let scheme = key_schema("emulation.colorScheme");
    assert!(check(&scheme, &json!(null)).is_empty());
    assert!(check(&scheme, &json!("light")).is_empty());
    assert_eq!(
        paths(&check(&scheme, &json!("sepia"))),
        [("", reason::ENUM)]
    );
    let level = key_schema("logs.level");
    assert_eq!(paths(&check(&level, &json!("loud"))), [("", reason::ENUM)]);
}

#[test]
fn bounds_and_unknown_fields_are_named() {
    let timeout = key_schema("shortcuts.leaderTimeoutMs");
    assert_eq!(paths(&check(&timeout, &json!(-1))), [("", reason::MINIMUM)]);
    assert!(check(&timeout, &json!(0)).is_empty(), "0 is no timeout");
    assert!(check(&timeout, &json!(1000)).is_empty());
    let schema = json!({
        "type": "object",
        "properties": { "a/b": { "type": "integer", "maximum": 3 } },
        "additionalProperties": false
    });
    assert_eq!(
        paths(&check(&schema, &json!({ "a/b": 4, "c": 1 }))),
        [("/a~1b", reason::MAXIMUM), ("/c", reason::UNKNOWN_FIELD)]
    );
}
