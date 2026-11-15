use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::test_support::{def, def_with_schema};
use crate::{Category, SettingsRegistry};

fn registry() -> Arc<SettingsRegistry> {
    let mut registry = SettingsRegistry::new();
    let mut certs = def(
        "network.ignoreCertificateErrors",
        Category::Network,
        "certificates",
        1,
    );
    certs.local_only = true;
    certs.scope_allowed = vec![Scope::Local];
    certs.schema = json!({ "type": "boolean", "default": false });
    let mut devtools = def_with_schema(
        "devtools.mode",
        Category::Devtools,
        "devtools",
        1,
        json!({
            "type": "string",
            "enum": ["docked-bottom", "docked-right", "window"],
            "default": "docked-bottom"
        }),
    );
    devtools.scope_allowed = vec![Scope::User];
    for def in [
        def("sync.scroll", Category::Sync, "scroll", 1),
        def_with_schema(
            "layout.gap",
            Category::Layout,
            "grid",
            1,
            json!({ "type": "integer", "minimum": 0, "maximum": 64, "default": 8 }),
        ),
        certs,
        devtools,
    ] {
        registry.register(def).expect("registers");
    }
    Arc::new(registry)
}

fn resolved(store: &LayeredStore, key: &str) -> (serde_json::Value, Origin) {
    let value = store.resolve(key).expect("known key");
    (value.value, value.origin)
}

#[test]
fn local_beats_project_beats_default() {
    let store = LayeredStore::new(registry());
    assert_eq!(
        resolved(&store, "sync.scroll"),
        (json!(true), Origin::Default)
    );

    store.replace_layer(Scope::Project, json!({ "sync": { "scroll": false } }));
    assert_eq!(
        resolved(&store, "sync.scroll"),
        (json!(false), Origin::Project)
    );

    store.replace_layer(Scope::Local, json!({ "sync": { "scroll": true } }));
    assert_eq!(
        resolved(&store, "sync.scroll"),
        (json!(true), Origin::Local)
    );
}

#[test]
fn a_value_in_a_scope_the_key_does_not_allow_is_ignored() {
    let store = LayeredStore::new(registry())
        .with_layer(Scope::User, json!({ "sync": { "scroll": false } }))
        .with_layer(
            Scope::Project,
            json!({ "network": { "ignoreCertificateErrors": true } }),
        );
    assert_eq!(
        resolved(&store, "sync.scroll"),
        (json!(true), Origin::Default)
    );
    assert_eq!(
        resolved(&store, "network.ignoreCertificateErrors"),
        (json!(false), Origin::Default)
    );
}

#[test]
fn unknown_keys_are_refused() {
    let store = LayeredStore::new(registry());
    assert_eq!(
        store.resolve("sync.nope"),
        Err(SettingsError::UnknownKey("sync.nope".into()))
    );
    assert_eq!(
        store.set("sync.nope", json!(true), Scope::Project),
        Err(SettingsError::UnknownKey("sync.nope".into()))
    );
}

#[test]
fn a_local_only_key_cannot_be_written_to_project() {
    let store = LayeredStore::new(registry());
    assert_eq!(
        store.set(
            "network.ignoreCertificateErrors",
            json!(true),
            Scope::Project
        ),
        Err(SettingsError::ScopeNotAllowed {
            key: "network.ignoreCertificateErrors".into(),
            scope: Scope::Project,
        })
    );
    store
        .set("network.ignoreCertificateErrors", json!(true), Scope::Local)
        .expect("local is allowed");
    assert_eq!(
        store.layer(Scope::Local),
        json!({ "network": { "ignoreCertificateErrors": true } })
    );
    assert_eq!(store.layer(Scope::Project), json!({}));
}

#[test]
fn set_writes_the_layer_and_null_removes_the_key() {
    let store = LayeredStore::new(registry())
        .with_layer(Scope::Project, json!({ "sync": { "navigation": true } }));
    store
        .set("sync.scroll", json!(false), Scope::Project)
        .expect("writes");
    assert_eq!(
        store.layer(Scope::Project),
        json!({ "sync": { "navigation": true, "scroll": false } })
    );
    assert_eq!(
        resolved(&store, "sync.scroll"),
        (json!(false), Origin::Project)
    );
    store
        .set("sync.scroll", serde_json::Value::Null, Scope::Project)
        .expect("removes");
    assert_eq!(
        store.layer(Scope::Project),
        json!({ "sync": { "navigation": true } })
    );
    assert_eq!(
        resolved(&store, "sync.scroll"),
        (json!(true), Origin::Default)
    );
}

#[test]
fn set_refuses_a_value_against_the_schema() {
    let store = LayeredStore::new(registry());
    for (key, value, scope) in [
        ("sync.scroll", json!("yes"), Scope::Project),
        ("layout.gap", json!(1.5), Scope::Project),
        ("layout.gap", json!(65), Scope::Project),
        ("layout.gap", json!(-1), Scope::Project),
        ("devtools.mode", json!("floating"), Scope::User),
    ] {
        let error = store.set(key, value.clone(), scope).expect_err("invalid");
        assert!(
            matches!(&error, SettingsError::Invalid { key: k, .. } if k == key),
            "{key} = {value}: {error:?}"
        );
    }
    store
        .set("layout.gap", json!(12), Scope::Project)
        .expect("in range");
    store
        .set("devtools.mode", json!("window"), Scope::User)
        .expect("in the enum");
}

#[test]
fn set_refuses_to_replace_a_non_object_on_the_path() {
    let store = LayeredStore::new(registry()).with_layer(Scope::Project, json!({ "sync": 3 }));
    assert!(matches!(
        store.set("sync.scroll", json!(true), Scope::Project),
        Err(SettingsError::Invalid { .. })
    ));
}

#[test]
fn the_store_is_shareable() {
    fn shareable<T: Send + Sync>() {}
    shareable::<LayeredStore>();
    let store: Arc<dyn SettingsStore> = Arc::new(LayeredStore::new(registry()));
    assert!(store.resolve("sync.scroll").is_ok());
}

#[test]
fn a_definition_checks_a_value_as_a_write_would() {
    let gap = def_with_schema(
        "layout.gap",
        Category::Layout,
        "grid",
        1,
        json!({ "type": "integer", "minimum": 0, "maximum": 64, "default": 8 }),
    );
    assert!(gap.check_value(&json!(8)).is_ok());
    assert!(gap.check_value(&json!(65)).is_err());
    assert!(gap.check_value(&json!("8")).is_err());
}
