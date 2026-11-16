use super::*;

#[test]
fn scope_and_origin_serialize_lowercase() {
    assert_eq!(
        serde_json::to_value(Scope::Local).expect("serializes"),
        "local"
    );
    assert_eq!(
        serde_json::to_value(Origin::Default).expect("serializes"),
        "default"
    );
    assert_eq!(Origin::from(Scope::Project), Origin::Project);
}

#[test]
fn annotation_names_are_fixed() {
    assert_eq!(
        [X_CATEGORY, X_SECTION, X_ORDER],
        ["x-category", "x-section", "x-order"]
    );
}

#[test]
fn a_new_registry_is_empty() {
    let registry = SettingsRegistry::new();
    assert!(registry.get("sync.scroll").is_none());
    assert_eq!(registry.iter().count(), 0);
    assert_eq!(registry.iter_category(Category::Sync).count(), 0);
}

#[test]
fn setting_def_serializes_camel_case() {
    let def = SettingDef {
        key: "network.ignoreCertificateErrors".into(),
        category: Category::Network,
        section: "certificates".into(),
        order: 1,
        scope_allowed: vec![Scope::Local],
        local_only: true,
        protected_from_mcp: false,
        label_key: "l".into(),
        description_key: "d".into(),
        help_key: "h".into(),
        schema: serde_json::json!({}),
    };
    let json = serde_json::to_value(&def).expect("serializes");
    assert_eq!(json["scopeAllowed"], serde_json::json!(["local"]));
    assert_eq!(json["localOnly"], true);
    assert_eq!(json["protectedFromMcp"], false);
}
