//! The fake's settings document, moved out of `testing.rs` (RB-098-01) and re-exported there.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::PROTECTED;

/// The keys of the fake's settings: key, category, section, default, label.
const FAKE_KEYS: &[(&str, &str, &str, bool, &str)] = &[
    (
        "startup.openLastProject",
        "general",
        "startup",
        false,
        "Open the last project",
    ),
    ("grid.fitToWindow", "layout", "zoom", true, "Fit to window"),
    (
        "mcp.allowEvaluate",
        "security",
        "evaluate",
        false,
        "Allow evaluate",
    ),
];

/// The settings document as the app hands it to `settings_get` (`crate::settings_view`): three
/// categories with a section each, the [`FAKE_KEYS`] and every other key `written`, with the
/// written values (origin `user`).
#[must_use]
pub fn settings_document(written: &BTreeMap<String, Value>) -> Value {
    let mut keys: Vec<Value> = FAKE_KEYS
        .iter()
        .map(|(key, category, section, default, label)| {
            let (value, origin) = written
                .get(*key)
                .map_or((Value::Bool(*default), "default"), |value| {
                    (value.clone(), "user")
                });
            json!({
                "key": key, "category": category, "section": section, "type": "boolean",
                "value": value, "origin": origin, "default": default, "label": label,
                "description": "", "protected": PROTECTED.contains(key),
            })
        })
        .collect();
    keys.extend(
        written
            .iter()
            .filter(|(key, _)| !FAKE_KEYS.iter().any(|known| known.0 == key.as_str()))
            .map(|(key, value)| {
                json!({ "key": key, "category": "general", "section": "startup",
                        "value": value, "origin": "user" })
            }),
    );
    json!({
        "categories": [
            { "id": "general", "title": "General", "sections": [{ "id": "startup", "title": "Startup" }] },
            { "id": "layout", "title": "Layout", "sections": [{ "id": "zoom", "title": "Zoom" }] },
            { "id": "security", "title": "Security", "sections": [{ "id": "evaluate", "title": "Evaluate" }] },
        ],
        "keys": keys,
    })
}
