use serde_json::json;

use super::*;

/// Two keys of the two shapes: a list whose items are a choice, and a string that is one.
#[test]
fn every_string_outside_its_enum_is_named_where_it_is() {
    let schema = json!({
        "type": "object",
        "properties": {
            "checks": { "type": "object", "properties": {
                "lighthouse": { "type": "object", "properties": {
                    "categories": { "type": "array", "items": { "type": "string", "enum": ["seo", "pwa"] } }
                } }
            } },
            "cookies": { "anyOf": [
                { "type": "object", "properties": {
                    "copy": { "type": "string", "enum": ["manual", "onStart"] }
                } },
                { "type": "null" }
            ] }
        }
    });
    let value = json!({
        "checks": { "lighthouse": { "categories": ["seo", "", "speed"] } },
        "cookies": { "copy": "sometimes" },
        "other": "free"
    });
    let found: Vec<(String, String)> = not_allowed(&schema, &value)
        .into_iter()
        .map(|issue| (issue.path, issue.key))
        .collect();
    assert_eq!(
        found,
        [
            (
                "/checks/lighthouse/categories/1".to_owned(),
                NOT_ALLOWED_KEY.to_owned()
            ),
            (
                "/checks/lighthouse/categories/2".to_owned(),
                NOT_ALLOWED_KEY.to_owned()
            ),
            ("/cookies/copy".to_owned(), NOT_ALLOWED_KEY.to_owned()),
        ]
    );
    let allowed = json!({ "checks": { "lighthouse": { "categories": ["pwa"] } }, "cookies": { "copy": "manual" } });
    assert!(not_allowed(&schema, &allowed).is_empty());
}
