use std::cell::Cell;
use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{Layers, SettingsCache, plugin_setting, plugin_values};

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "maxPages": {
                "type": "integer", "default": 10,
                "x-category": "checks", "x-section": "seo", "x-order": 1,
                "x-scope": ["user", "project", "local"]
            },
            "strict": {
                "type": "boolean", "default": false,
                "x-category": "checks", "x-section": "seo", "x-order": 2, "x-scope": ["user"]
            },
            "depth": {
                "type": "integer", "default": 1,
                "x-category": "checks", "x-section": "seo", "x-order": 3,
                "x-scope": ["project", "user"]
            }
        }
    })
}

fn user(value: Value) -> Layers {
    Layers {
        user: value,
        ..Layers::default()
    }
}

#[test]
fn a_plugins_values_are_the_users_over_the_schema_defaults() {
    let layers = user(json!({ "seo": { "maxPages": 25 }, "other": { "maxPages": 99 } }));
    let values = plugin_values("seo", &schema(), &layers);
    assert_eq!(values.get("seo.maxPages"), Some(&json!(25)));
    assert_eq!(values.get("seo.strict"), Some(&json!(false)));
    assert_eq!(values.get("seo.depth"), Some(&json!(1)));
    assert_eq!(values.len(), 3);
}

#[test]
fn the_strongest_scope_the_schema_allows_wins() {
    let layers = Layers {
        user: json!({ "seo": { "maxPages": 25, "strict": true, "depth": 2 } }),
        project: Some(json!({ "seo": { "maxPages": 40, "strict": false, "depth": 3 } })),
        local: Some(json!({ "seo": { "maxPages": 60, "depth": 9 } })),
    };
    let values = plugin_values("seo", &schema(), &layers);
    assert_eq!(
        values["seo.maxPages"],
        json!(60),
        "local over project over user"
    );
    assert_eq!(
        values["seo.strict"],
        json!(true),
        "a user-only key ignores the project"
    );
    assert_eq!(
        values["seo.depth"],
        json!(3),
        "a key without local scope ignores the local file"
    );
    let without_local = Layers {
        local: None,
        ..layers
    };
    assert_eq!(
        plugin_values("seo", &schema(), &without_local)["seo.maxPages"],
        json!(40)
    );
}

#[test]
fn settings_get_takes_the_key_with_or_without_the_plugin_id() {
    let values = plugin_values(
        "seo",
        &schema(),
        &user(json!({ "seo": { "strict": true } })),
    );
    assert_eq!(
        plugin_setting("seo", &values, "strict").as_deref(),
        Some("true")
    );
    assert_eq!(
        plugin_setting("seo", &values, "seo.maxPages").as_deref(),
        Some("10")
    );
    assert_eq!(plugin_setting("seo", &values, "undeclared"), None);
    assert_eq!(plugin_setting("seo", &values, "other.maxPages"), None);
}

#[test]
fn the_values_are_resolved_once_per_change() {
    let cache = SettingsCache::default();
    let builds = Cell::new(0);
    let build = || {
        builds.set(builds.get() + 1);
        BTreeMap::from([("seo".to_owned(), serde_json::Map::new())])
    };
    let first = cache.get_or_build(build);
    let second = cache.get_or_build(build);
    assert_eq!(builds.get(), 1, "the second call reads the cache");
    assert_eq!(first, second);
    cache.invalidate();
    let _ = cache.get_or_build(build);
    assert_eq!(builds.get(), 2, "a change resolves again");
}

fn axe_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "tags": {
                "type": "array", "items": { "type": "string" }, "default": ["wcag22aa"],
                "x-category": "checks", "x-section": "accessibility", "x-order": 10,
                "x-scope": ["user", "project", "local"]
            }
        }
    })
}

#[test]
fn the_removed_core_key_checks_axe_tags_still_sets_the_axe_plugins_tags() {
    let legacy = |project: Value| Layers {
        project: Some(project),
        ..Layers::default()
    };
    // A project that set the old core key keeps its tags.
    let layers = legacy(json!({ "checks": { "axe": { "tags": ["wcag2a"] } } }));
    let values = plugin_values("a11y-axe", &axe_schema(), &layers);
    assert_eq!(values["a11y-axe.tags"], json!(["wcag2a"]));
    // The plugin's own key wins in the same file.
    let layers = legacy(json!({
        "checks": { "axe": { "tags": ["wcag2a"] } },
        "a11y-axe": { "tags": ["best-practice"] }
    }));
    let values = plugin_values("a11y-axe", &axe_schema(), &layers);
    assert_eq!(values["a11y-axe.tags"], json!(["best-practice"]));
    // The old default, an empty list, is no choice: the plugin's default holds.
    let layers = legacy(json!({ "checks": { "axe": { "tags": [] } } }));
    let values = plugin_values("a11y-axe", &axe_schema(), &layers);
    assert_eq!(values["a11y-axe.tags"], json!(["wcag22aa"]));
    // No other plugin reads it.
    let layers = legacy(json!({ "checks": { "axe": { "tags": ["wcag2a"] } } }));
    let values = plugin_values("seo", &axe_schema(), &layers);
    assert_eq!(values["seo.tags"], json!(["wcag22aa"]));
}
