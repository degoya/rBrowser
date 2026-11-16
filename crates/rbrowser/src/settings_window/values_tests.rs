use std::collections::BTreeSet;

use rb_config::UserSettings;
use serde_json::json;

use super::*;
use crate::host::state::default_config;

fn registry() -> SettingsRegistry {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    registry
}

fn documents(
    project_file: Value,
    local_file: Option<Value>,
    user_file: Option<Value>,
) -> Documents {
    Documents {
        user: serde_json::to_value(UserSettings::default()).expect("serializes"),
        user_file,
        project: Some(ProjectDocuments {
            root: PathBuf::from("/work/site"),
            effective: serde_json::to_value(default_config()).expect("serializes"),
            project_file,
            local_file,
        }),
    }
}

fn find<'a>(values: &'a [SettingValue], key: &str) -> &'a SettingValue {
    values
        .iter()
        .find(|value| value.key == key)
        .unwrap_or_else(|| panic!("{key} is listed"))
}

#[test]
fn every_key_of_the_schemas_is_listed_once_in_values_and_schema() {
    let registry = registry();
    let documents = documents(json!({ "version": 1 }), None, None);
    let all: BTreeSet<String> = registry.iter().map(|def| def.key.clone()).collect();
    let listed: Vec<String> = values(&registry, &documents)
        .into_iter()
        .map(|value| value.key)
        .collect();
    assert_eq!(listed.len(), all.len());
    assert_eq!(listed.iter().cloned().collect::<BTreeSet<_>>(), all);
    let payload = schema_payload(&registry, &documents);
    let defs = payload["defs"].as_array().expect("defs");
    let keys: BTreeSet<&str> = defs.iter().filter_map(|def| def["key"].as_str()).collect();
    assert_eq!(defs.len(), all.len());
    assert_eq!(keys.len(), all.len());
    assert_eq!(payload["categories"].as_array().map(Vec::len), Some(18));
    assert_eq!(payload["categories"][0]["id"], "general");
    assert_eq!(payload["project"]["root"], "/work/site");
    assert_eq!(payload["project"]["hasLocal"], false);
    let def = defs
        .iter()
        .find(|def| def["key"] == "network.ignoreCertificateErrors")
        .expect("listed");
    assert_eq!(def["localOnly"], true);
    assert_eq!(def["scopeAllowed"], json!(["local"]));
}

#[test]
fn a_value_of_config_local_json_is_badged_local() {
    let registry = registry();
    let documents = documents(
        json!({ "version": 1, "sync": { "scroll": true } }),
        Some(json!({ "sync": { "scroll": false } })),
        None,
    );
    let values = values(&registry, &documents);
    assert_eq!(find(&values, "sync.scroll").origin, Origin::Local);
    assert_eq!(find(&values, "sync.click").origin, Origin::Default);
}

#[test]
fn a_project_value_is_badged_project_and_a_user_value_user() {
    let registry = registry();
    let documents = documents(
        json!({ "version": 1, "layout": { "gap": 12 } }),
        None,
        Some(json!({ "ui": { "theme": "dark" } })),
    );
    let values = values(&registry, &documents);
    assert_eq!(find(&values, "layout").origin, Origin::Project);
    assert_eq!(find(&values, "ui.theme").origin, Origin::User);
    assert_eq!(find(&values, "logs.level").origin, Origin::Default);
}

#[test]
fn a_trust_gated_local_value_shows_what_its_file_stores() {
    let registry = registry();
    let documents = documents(
        json!({ "version": 1 }),
        Some(json!({ "network": { "ignoreCertificateErrors": true } })),
        None,
    );
    let value = resolve(
        registry
            .get("network.ignoreCertificateErrors")
            .expect("known"),
        &documents,
    );
    assert_eq!(value.origin, Origin::Local);
    assert_eq!(value.value, json!(true));
}

#[test]
fn without_a_user_file_a_changed_user_value_counts_as_the_users() {
    let registry = registry();
    let mut user = UserSettings::default();
    user.ui.theme = rb_config::Theme::Dark;
    let documents = Documents {
        user: serde_json::to_value(user).expect("serializes"),
        user_file: None,
        project: None,
    };
    let values = values(&registry, &documents);
    assert_eq!(find(&values, "ui.theme").origin, Origin::User);
    assert_eq!(find(&values, "ui.theme").value, json!("dark"));
    assert_eq!(find(&values, "sync.scroll").origin, Origin::Default);
    assert_eq!(find(&values, "sync.scroll").value, json!(true));
    assert!(schema_payload(&registry, &documents)["project"].is_null());
}

/// RB-090-02: General > Updates lists every key; without the feature the update keys carry the
/// hint that locks them, the CEF notice does not.
#[test]
fn the_update_keys_are_marked_locked_while_self_update_is_not_built() {
    let registry = registry();
    let payload = schema_payload(&registry, &documents(json!({ "version": 1 }), None, None));
    let defs = payload["defs"].as_array().expect("defs");
    let locked = |key: &str| {
        defs.iter()
            .find(|def| def["key"] == key)
            .unwrap_or_else(|| panic!("{key} is listed"))
            .get("lockedKey")
            .cloned()
    };
    for key in crate::updates::LOCKED_KEYS {
        let expected = (!crate::updates::SELF_UPDATE_BUILT)
            .then(|| Value::String(crate::updates::INACTIVE_KEY.to_owned()));
        assert_eq!(locked(key), expected, "{key}");
    }
    assert_eq!(locked("updates.cefAgeWarning"), None);
}

/// RB-080-82: the settings window has the core's catalogue only; the labels of a plugin's keys
/// and its name come with the schema, from the plugin's own catalogue, and nothing else of it.
#[test]
fn the_schema_carries_the_texts_of_the_plugins_keys() {
    let locale = br#"{"plugin":{"texts-schema":{"name":"Texts schema","settings":{"depth":{"label":"Depth","description":"How deep.","help":"How deep, in levels."}},"commands":{"run":"Run"},"panel":{"title":"Panel"}}}}"#;
    rb_i18n::register_plugin_locale("texts-schema", "en", locale).expect("registers");
    let mut registry = registry();
    let fragment = json!({
        "type": "object",
        "properties": {
            "depth": {
                "type": "integer",
                "default": 1,
                "x-category": "devtools",
                "x-section": "console",
                "x-order": 900,
                "x-scope": ["user"]
            }
        }
    });
    registry
        .register_plugin_schema("texts-schema", &fragment)
        .expect("registers");
    let payload = schema_payload(&registry, &documents(json!({ "version": 1 }), None, None));
    let texts = payload["texts"].as_object().expect("texts");
    assert_eq!(texts["plugin.texts-schema.name"], "Texts schema");
    assert_eq!(texts["plugin.texts-schema.settings.depth.label"], "Depth");
    assert_eq!(
        texts["plugin.texts-schema.settings.depth.description"],
        "How deep."
    );
    assert_eq!(
        texts["plugin.texts-schema.settings.depth.help"],
        "How deep, in levels."
    );
    // What the settings do not show stays with the plugin.
    assert!(!texts.contains_key("plugin.texts-schema.commands.run"));
    assert!(!texts.contains_key("plugin.texts-schema.panel.title"));
    assert!(texts.keys().all(|key| key.starts_with("plugin.")));
}
