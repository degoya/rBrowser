use super::*;
use crate::Scope;
use crate::test_support::def;

#[test]
fn registers_and_finds_a_definition() {
    let mut registry = SettingsRegistry::new();
    registry
        .register(def("sync.scroll", Category::Sync, "scroll", 1))
        .expect("registers");
    let found = registry.get("sync.scroll").expect("registered");
    assert_eq!(found.section, "scroll");
    assert_eq!(registry.iter().count(), 1);
    assert_eq!(registry.plugin_of("sync.scroll"), None);
}

#[test]
fn refuses_a_section_the_category_does_not_have() {
    let mut registry = SettingsRegistry::new();
    let error = registry
        .register(def("sync.scroll", Category::Sync, "media-features", 1))
        .expect_err("emulation's section under sync");
    assert_eq!(
        error,
        SettingsError::UnknownSection {
            category: "sync".into(),
            section: "media-features".into(),
        }
    );
}

#[test]
fn refuses_a_key_twice() {
    let mut registry = SettingsRegistry::new();
    registry
        .register(def("sync.scroll", Category::Sync, "scroll", 1))
        .expect("registers");
    let error = registry
        .register(def("sync.scroll", Category::Sync, "scroll", 2))
        .expect_err("same key");
    assert_eq!(error, SettingsError::Duplicate("sync.scroll".into()));
}

#[test]
fn refuses_a_repeated_order_in_one_section() {
    let mut registry = SettingsRegistry::new();
    registry
        .register(def("sync.scroll", Category::Sync, "scroll", 1))
        .expect("registers");
    let error = registry
        .register(def("sync.scrollSmooth", Category::Sync, "scroll", 1))
        .expect_err("same order");
    assert!(
        matches!(&error, SettingsError::Invalid { key, reason }
            if key == "sync.scrollSmooth" && reason.contains("x-order 1")),
        "{error:?}"
    );
    registry
        .register(def("sync.navigation", Category::Sync, "navigation", 1))
        .expect("the same order in another section is fine");
}

#[test]
fn refuses_malformed_keys_and_scopes() {
    let mut registry = SettingsRegistry::new();
    for key in [
        "",
        "sync.",
        ".scroll",
        "sync..scroll",
        "sync scroll",
        "1sync",
    ] {
        assert!(
            registry
                .register(def(key, Category::Sync, "scroll", 1))
                .is_err(),
            "accepted `{key}`"
        );
    }
    let mut no_scope = def("sync.scroll", Category::Sync, "scroll", 1);
    no_scope.scope_allowed.clear();
    assert!(registry.register(no_scope).is_err());
    let mut twice = def("sync.scroll", Category::Sync, "scroll", 1);
    twice.scope_allowed = vec![Scope::Local, Scope::Local];
    assert!(registry.register(twice).is_err());
    let mut bad_label = def("sync.scroll", Category::Sync, "scroll", 1);
    bad_label.label_key = "Settings Scroll".into();
    assert!(registry.register(bad_label).is_err());
}

#[test]
fn a_local_only_key_allows_only_the_local_scope() {
    let mut registry = SettingsRegistry::new();
    let mut certs = def(
        "network.ignoreCertificateErrors",
        Category::Network,
        "certificates",
        1,
    );
    certs.local_only = true;
    let error = registry
        .register(certs.clone())
        .expect_err("project allowed");
    assert!(matches!(error, SettingsError::Invalid { .. }), "{error:?}");
    certs.scope_allowed = vec![Scope::Local];
    registry.register(certs).expect("local only");
}

#[test]
fn iter_category_orders_by_section_then_order() {
    let mut registry = SettingsRegistry::new();
    for def in [
        def("sync.scroll", Category::Sync, "scroll", 2),
        def("sync.navigation", Category::Sync, "navigation", 1),
        def("sync.scrollSmooth", Category::Sync, "scroll", 1),
        def("layout.gap", Category::Layout, "grid", 1),
    ] {
        registry.register(def).expect("registers");
    }
    let keys: Vec<&str> = registry
        .iter_category(Category::Sync)
        .map(|def| def.key.as_str())
        .collect();
    assert_eq!(
        keys,
        ["sync.navigation", "sync.scrollSmooth", "sync.scroll"]
    );
}

#[test]
fn plugin_keys_carry_the_plugin_prefix() {
    let mut registry = SettingsRegistry::new();
    let error = registry
        .register_plugin("seo", def("lighthouse.runs", Category::Checks, "seo", 1))
        .expect_err("foreign prefix");
    assert!(matches!(error, SettingsError::Invalid { .. }), "{error:?}");
    registry
        .register_plugin("seo", def("seo.titleLength", Category::Checks, "seo", 1))
        .expect("own prefix");
    assert_eq!(registry.plugin_of("seo.titleLength"), Some("seo"));
    let keys: Vec<&str> = registry
        .iter_plugin("seo")
        .map(|def| def.key.as_str())
        .collect();
    assert_eq!(keys, ["seo.titleLength"]);
    assert!(
        registry
            .register_plugin("Bad Id", def("Bad Id.x", Category::Checks, "seo", 1))
            .is_err()
    );
}

#[test]
fn orders_repeat_freely_between_core_and_a_plugin() {
    let mut registry = SettingsRegistry::new();
    registry
        .register(def("checks.seoEnabled", Category::Checks, "seo", 1))
        .expect("core");
    registry
        .register_plugin("seo", def("seo.titleLength", Category::Checks, "seo", 1))
        .expect("a plugin numbers its own keys");
    let error = registry
        .register_plugin("seo", def("seo.metaLength", Category::Checks, "seo", 1))
        .expect_err("inside the plugin the order is unique");
    assert!(matches!(error, SettingsError::Invalid { .. }), "{error:?}");
}

#[test]
fn mcp_access_refuses_protected_and_unknown_keys() {
    let mut registry = SettingsRegistry::new();
    let mut mcp = def("mcp.enabled", Category::Automation, "mcp", 1);
    mcp.protected_from_mcp = true;
    mcp.scope_allowed = vec![Scope::User];
    registry.register(mcp).expect("registers");
    registry
        .register(def("sync.scroll", Category::Sync, "scroll", 1))
        .expect("registers");
    assert_eq!(
        registry.check_mcp_access("mcp.enabled"),
        Err(SettingsError::Protected("mcp.enabled".into()))
    );
    assert_eq!(
        registry.check_mcp_access("nope.key"),
        Err(SettingsError::UnknownKey("nope.key".into()))
    );
    assert!(registry.check_mcp_access("sync.scroll").is_ok());
}

#[test]
fn the_default_comes_from_the_schema() {
    let with = def("sync.scroll", Category::Sync, "scroll", 1);
    assert_eq!(with.default_value(), serde_json::Value::Bool(true));
    let mut without = with;
    without.schema = serde_json::json!({ "type": "string" });
    assert_eq!(without.default_value(), serde_json::Value::Null);
}
