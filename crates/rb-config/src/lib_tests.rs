use super::*;

#[test]
fn the_entry_points_are_implemented() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(
        load_project(dir.path()),
        Err(ConfigError::Io { .. })
    ));
    let settings = load_user_settings(&dir.path().join("settings.json")).expect("defaults");
    assert_eq!(settings.version, CURRENT_VERSION);
    let (value, notes) = migrate(serde_json::json!({ "version": 0 })).expect("migrates");
    assert_eq!(value["version"], CURRENT_VERSION);
    assert_eq!(notes.len(), 1);
    assert_eq!(discover(&dir.path().join("nowhere")), None);
}

#[test]
fn constants_name_the_files() {
    assert_eq!(CURRENT_VERSION, 1);
    assert_eq!(CONFIG_DIR, ".rbrowser");
    assert_eq!(CONFIG_FILE, "config.json");
    assert_eq!(LOCAL_CONFIG_FILE, "config.local.json");
    assert!(LOCAL_ONLY_KEYS.contains(&"network.ignoreCertificateErrors"));
}

#[test]
fn schemas_generate() {
    let project = serde_json::to_value(project_schema()).expect("serializes");
    assert!(project["properties"]["startUrl"].is_object());
    assert!(project["properties"]["breakpoints"].is_object());
    let user = serde_json::to_value(user_settings_schema()).expect("serializes");
    assert!(user["properties"]["shortcuts"].is_object());
}

#[test]
fn errors_carry_an_i18n_key() {
    let issue = ConfigIssue {
        path: "/breakpoints/1/id".into(),
        key: "config.error.duplicate_id".into(),
        message: "duplicate breakpoint id `mobile`".into(),
    };
    let error = ConfigError::Invalid(vec![issue]);
    assert!(error.to_string().contains("1 problem"));
    assert_eq!(error.key(), "config.error.invalid");
    let newer = ConfigError::UnsupportedVersion {
        found: 2,
        supported: 1,
    };
    assert_eq!(newer.key(), "config.error.unsupported_version");
}
