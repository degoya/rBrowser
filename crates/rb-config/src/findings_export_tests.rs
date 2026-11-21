use serde_json::json;

use super::*;
use crate::{ProjectConfig, UserSettings};

#[test]
fn the_export_keys_live_in_the_checks_section() {
    let config: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "checks": {
            "export": { "defaultFormat": "junit", "failOn": "warning", "dir": "reports" }
        }
    }))
    .expect("a config");
    let export = &config.checks.findings.export;
    assert_eq!(export.default_format, Some(FindingsExportFormat::Junit));
    assert_eq!(export.fail_on, Some(FindingsFailOn::Warning));
    assert_eq!(export.dir.as_deref(), Some("reports"));
    assert!(!config.checks.extra.contains_key("export"));
}

#[test]
fn the_user_settings_read_the_same_keys() {
    let user: UserSettings = serde_json::from_value(json!({
        "checks": { "export": { "failOn": "info" } }
    }))
    .expect("user settings");
    assert_eq!(user.checks.export.fail_on, Some(FindingsFailOn::Info));
    assert_eq!(user.checks.export.default_format, None);
}

#[test]
fn an_unknown_format_is_refused() {
    let parsed = serde_json::from_value::<FindingsExportSettings>(json!({
        "defaultFormat": "pdf"
    }));
    assert!(parsed.is_err());
}

#[test]
fn the_project_wins_per_key() {
    let project = FindingsExportSettings {
        fail_on: Some(FindingsFailOn::Warning),
        ..FindingsExportSettings::default()
    };
    let user = FindingsExportSettings {
        default_format: Some(FindingsExportFormat::Html),
        fail_on: Some(FindingsFailOn::Info),
        dir: Some("C:\\reports".to_owned()),
    };
    let merged = project.over(&user);
    assert_eq!(merged.fail_on, Some(FindingsFailOn::Warning));
    assert_eq!(merged.default_format, Some(FindingsExportFormat::Html));
    assert_eq!(merged.dir.as_deref(), Some("C:\\reports"));
}

#[test]
fn nothing_set_writes_nothing() {
    let value = serde_json::to_value(FindingsExportSettings::default()).expect("serializes");
    assert_eq!(value, json!({}));
}
