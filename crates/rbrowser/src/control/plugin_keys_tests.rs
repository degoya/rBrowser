use rb_settings::Category;

use super::*;

/// The API client's settings schema as the plugin ships it.
fn api_client() -> (String, Value) {
    let schema = serde_json::from_str(include_str!(
        "../../../../plugins/api-client/settings.schema.json"
    ))
    .expect("the API client's settings schema is JSON");
    ("api-client".to_owned(), schema)
}

/// RB-094-26: the plugins' keys join the core's - `api-client.benchRepeats` is a key of
/// Network > API client, marked as the plugin's.
#[test]
fn a_loaded_plugins_keys_join_the_core_keys() {
    let registry = with_plugins(&[api_client()]);
    let def = registry
        .get("api-client.benchRepeats")
        .expect("the plugin's key is registered");
    assert_eq!(def.category, Category::Network);
    assert_eq!(def.section, "api-client");
    assert_eq!(
        registry.plugin_of("api-client.benchRepeats"),
        Some("api-client")
    );
    assert!(registry.get("sync.scroll").is_some(), "the core keys stay");
}

#[test]
fn the_registry_is_built_once_per_load_and_again_for_the_next() {
    let live = LiveRegistry::new(crate::control::settings::settings_registry().0);
    let none = live.current(Arc::default());
    assert!(none.get("api-client.benchRepeats").is_none());
    let load: PluginSchemas = Arc::new(vec![api_client()]);
    let first = live.current(Arc::clone(&load));
    assert!(first.get("api-client.benchRepeats").is_some());
    let again = live.current(Arc::clone(&load));
    assert!(
        Arc::ptr_eq(&first, &again),
        "the same load shares one registry"
    );
    // The plugin removed (a live change): its keys leave.
    let removed = live.current(Arc::new(Vec::new()));
    assert!(removed.get("api-client.benchRepeats").is_none());
}
