use crate::ProjectConfig;

use super::*;

#[test]
fn the_section_names_the_environments_and_nothing_secret() {
    let config: ProjectConfig =
        serde_json::from_str(r#"{ "version": 1, "basicAuth": { "environments": ["staging"] } }"#)
            .expect("parses");
    assert_eq!(config.basic_auth.environments, ["staging"]);
    assert!(config.basic_auth.uses("staging"));
    assert!(!config.basic_auth.uses("local"));
}

#[test]
fn a_user_or_password_in_the_section_is_refused() {
    // The credentials live in the platform's credential store, never in a config file: a
    // section that carries them does not load.
    for field in ["user", "password", "username", "credentials"] {
        let text =
            format!(r#"{{ "version": 1, "basicAuth": {{ "environments": [], "{field}": "x" }} }}"#);
        let parsed: Result<ProjectConfig, _> = serde_json::from_str(&text);
        assert!(parsed.is_err(), "{field} must be refused");
    }
}

#[test]
fn a_default_section_is_not_written() {
    let config: ProjectConfig = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(config.basic_auth, BasicAuthConfig::default());
    let written = serde_json::to_value(&config).expect("serializes");
    assert!(written.get("basicAuth").is_none(), "{written}");
}

#[test]
fn the_list_offers_the_projects_environments() {
    let schema = schemars::schema_for!(BasicAuthConfig);
    let value = serde_json::to_value(schema).expect("schema");
    assert_eq!(
        value.pointer("/properties/environments/x-options"),
        Some(&serde_json::json!("environments"))
    );
}

#[test]
fn the_key_is_protected_and_sits_beside_the_environments() {
    let (project, _) = crate::schema::schemas();
    let project = serde_json::to_value(project).expect("schema");
    let key = &project["properties"]["basicAuth"]["properties"]["environments"];
    assert_eq!(key[rb_settings::X_PROTECTED], true, "{key}");
    assert_eq!(key[rb_settings::X_CATEGORY], "project");
    assert_eq!(key[rb_settings::X_SECTION], "environments");
    assert!(key[rb_settings::X_ORDER].is_u64(), "{key}");
    assert_eq!(key[rb_settings::X_OPTIONS], "environments");
}
