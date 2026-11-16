use serde_json::{Value, json};

use super::*;
use crate::test_support::annotated;

/// `extra` merged into the annotations of `category` > `section` at `order`.
fn key(category: &str, section: &str, order: u32, scope: &[&str], extra: Value) -> Value {
    let mut value = annotated(category, section, order, scope);
    if let (Some(target), Value::Object(more)) = (value.as_object_mut(), extra) {
        target.extend(more);
    }
    value
}

/// A schema in the shape `schemars` writes: groups behind `$ref`, an optional group behind
/// `anyOf`, root fields that are file structure.
fn project_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "$schema": { "type": "string" },
            "version": { "type": "integer" },
            "startUrl": key("project", "start-url", 1, &["project", "local"],
                json!({ "type": "string", "default": "" })),
            "sync": { "$ref": "#/$defs/Sync" },
            "network": {
                "anyOf": [{ "$ref": "#/$defs/Network" }, { "type": "null" }]
            }
        },
        "$defs": {
            "Sync": {
                "type": "object",
                "properties": {
                    "scroll": key("sync", "scroll", 1, &["project", "local"],
                        json!({ "type": "boolean", "default": true })),
                    "navigation": key("sync", "navigation", 1, &["project", "local"],
                        json!({ "type": "boolean", "default": true }))
                }
            },
            "Network": {
                "type": "object",
                "properties": {
                    "ignoreCertificateErrors": key("network", "certificates", 1, &["local"],
                        json!({ "type": "boolean", "default": false, "x-local-only": true })),
                    "proxy": {
                        "$ref": "#/$defs/Proxy",
                        "x-category": "network",
                        "x-section": "certificates",
                        "x-order": 2,
                        "x-scope": ["local"],
                        "x-local-only": true
                    }
                }
            },
            "Proxy": {
                "type": "object",
                "properties": { "server": { "type": "string" } },
                "default": {}
            }
        }
    })
}

#[test]
fn reads_every_annotated_key_through_refs() {
    let defs = defs_from_schema(&project_schema()).expect("valid schema");
    let keys: Vec<&str> = defs.iter().map(|def| def.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "startUrl",
            "sync.scroll",
            "sync.navigation",
            "network.ignoreCertificateErrors",
            "network.proxy",
        ]
    );
    let certs = &defs[3];
    assert_eq!(certs.category, Category::Network);
    assert_eq!(certs.section, "certificates");
    assert!(certs.local_only);
    assert_eq!(certs.scope_allowed, [Scope::Local]);
    assert_eq!(
        certs.label_key,
        "settings.key.network.ignoreCertificateErrors.label"
    );
    assert_eq!(
        certs.description_key,
        "settings.key.network.ignoreCertificateErrors.description"
    );
    assert_eq!(
        certs.help_key,
        "settings.key.network.ignoreCertificateErrors.help"
    );
}

#[test]
fn a_ref_key_keeps_its_target_schema_and_its_own_annotations() {
    let defs = defs_from_schema(&project_schema()).expect("valid schema");
    let proxy = defs
        .iter()
        .find(|def| def.key == "network.proxy")
        .expect("proxy");
    assert_eq!(proxy.schema["type"], "object");
    assert_eq!(proxy.schema["x-order"], 2);
    assert!(proxy.schema.get("$ref").is_none());
    assert_eq!(proxy.default_value(), json!({}));
}

#[test]
fn a_key_without_x_category_is_named() {
    let mut schema = project_schema();
    schema["$defs"]["Sync"]["properties"]["hover"] = json!({ "type": "boolean" });
    let issues = defs_from_schema(&schema).expect_err("hover lacks annotations");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key, "sync.hover");
    assert!(
        matches!(&issues[0].error, SettingsError::Invalid { reason, .. }
            if reason.contains(X_CATEGORY)),
        "{issues:?}"
    );
}

#[test]
fn each_missing_annotation_is_reported() {
    for missing in [X_SECTION, X_ORDER, X_SCOPE] {
        let mut schema = project_schema();
        let scroll = &mut schema["$defs"]["Sync"]["properties"]["scroll"];
        scroll.as_object_mut().expect("object").remove(missing);
        let issues = defs_from_schema(&schema).expect_err(missing);
        assert_eq!(issues[0].key, "sync.scroll");
        assert!(
            matches!(&issues[0].error, SettingsError::Invalid { reason, .. }
                if reason.contains(missing)),
            "{issues:?}"
        );
    }
}

#[test]
fn unknown_category_section_and_scope_are_refused() {
    let mut schema = project_schema();
    schema["properties"]["startUrl"]["x-category"] = json!("my-category");
    let issues = defs_from_schema(&schema).expect_err("unknown category");
    assert_eq!(
        issues[0].error,
        SettingsError::UnknownCategory("my-category".into())
    );

    let mut schema = project_schema();
    schema["properties"]["startUrl"]["x-section"] = json!("scroll");
    let issues = defs_from_schema(&schema).expect_err("section of sync");
    assert!(matches!(
        issues[0].error,
        SettingsError::UnknownSection { .. }
    ));

    let mut schema = project_schema();
    schema["properties"]["startUrl"]["x-scope"] = json!(["global"]);
    assert!(defs_from_schema(&schema).is_err());

    let mut schema = project_schema();
    schema["properties"]["startUrl"]["x-order"] = json!(-1);
    assert!(defs_from_schema(&schema).is_err());
}

#[test]
fn plugin_keys_are_prefixed_and_labelled_in_the_plugin_namespace() {
    let fragment = json!({
        "type": "object",
        "properties": {
            "titleLength": key("checks", "seo", 1, &["project", "local"],
                json!({ "type": "integer", "default": 60 }))
        }
    });
    let defs = plugin_defs_from_schema("seo", &fragment).expect("valid fragment");
    assert_eq!(defs[0].key, "seo.titleLength");
    assert_eq!(defs[0].label_key, "plugin.seo.settings.titleLength.label");
    assert_eq!(
        defs[0].description_key,
        "plugin.seo.settings.titleLength.description"
    );
    assert_eq!(defs[0].help_key, "plugin.seo.settings.titleLength.help");
}

#[test]
fn a_plugin_cannot_add_a_top_level_category() {
    let fragment = json!({
        "type": "object",
        "properties": {
            "mode": key("seo-tools", "general", 1, &["project"], json!({ "type": "string" }))
        }
    });
    let issues = plugin_defs_from_schema("seo", &fragment).expect_err("new category");
    assert_eq!(issues[0].key, "seo.mode");
    assert_eq!(
        issues[0].error,
        SettingsError::UnknownCategory("seo-tools".into())
    );
}

#[test]
fn register_schema_fills_the_registry_and_reports_registry_rules() {
    let mut registry = SettingsRegistry::new();
    assert_eq!(registry.register_schema(&project_schema()), Ok(5));
    assert!(registry.get("sync.scroll").is_some());

    let mut schema = project_schema();
    schema["$defs"]["Sync"]["properties"]["navigation"]["x-section"] = json!("scroll");
    let mut registry = SettingsRegistry::new();
    let issues = registry
        .register_schema(&schema)
        .expect_err("two keys at order 1 in sync > scroll");
    assert_eq!(issues[0].key, "sync.navigation");

    let mut registry = SettingsRegistry::new();
    let fragment = json!({
        "type": "object",
        "properties": {
            "titleLength": key("checks", "seo", 1, &["project"], json!({ "type": "integer" }))
        }
    });
    assert_eq!(registry.register_plugin_schema("seo", &fragment), Ok(1));
    assert_eq!(registry.plugin_of("seo.titleLength"), Some("seo"));
}

#[test]
fn a_recursive_schema_ends() {
    let schema = json!({
        "type": "object",
        "properties": { "node": { "$ref": "#/$defs/Node" } },
        "$defs": {
            "Node": { "type": "object", "properties": { "child": { "$ref": "#/$defs/Node" } } }
        }
    });
    let issues = defs_from_schema(&schema).expect_err("never annotated");
    assert!(!issues.is_empty());
}
