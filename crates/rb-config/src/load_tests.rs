use super::*;
use std::fs;
use std::path::PathBuf;

use crate::jsonio::write_config_file;
use crate::{EngineKind, Origin};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A copy of fixture `name` in a temp directory.
fn copy_fixture(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path().join(".rbrowser");
    fs::create_dir_all(&config_dir).expect("mkdir");
    for entry in fs::read_dir(fixture(name).join(".rbrowser")).expect("fixture") {
        let entry = entry.expect("entry");
        fs::copy(entry.path(), config_dir.join(entry.file_name())).expect("copy");
    }
    dir
}

fn write_local(dir: &tempfile::TempDir, text: &str) {
    fs::write(dir.path().join(".rbrowser/config.local.json"), text).expect("write local");
}

#[test]
fn the_plan_example_loads_without_problems() {
    let loaded = load_project(&fixture("project")).expect("loads");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert!(loaded.migrations.is_empty());
    let config = &loaded.config;
    assert_eq!(config.start_url.as_deref(), Some("http://localhost:5173"));
    assert_eq!(config.environments.len(), 2);
    assert_eq!(config.breakpoints.len(), 2);
    assert_eq!(config.breakpoints[1].engine, EngineKind::Firefox);
    // RB-070-06: a path follows the environment, a URL stays.
    assert_eq!(
        crate::bookmarks::resolve(config, Some("local")),
        ["http://localhost:5173/checkout", "http://localhost:6006"]
    );
    assert_eq!(
        crate::bookmarks::resolve(config, Some("staging")),
        [
            "https://staging.example.com/checkout",
            "http://localhost:6006"
        ]
    );
    assert_eq!((config.layout.columns, config.layout.rows), (3, 2));
    assert_eq!(config.reload.debounce_ms, 200);
    assert_eq!(config.checks.lighthouse.categories, ["performance"]);
    assert_eq!(loaded.origins["sync.scroll"], Origin::Project);
}

/// RB-080-72: the owner's project held `"categories": [""]` (the free-text list editor of
/// before RB-080-20 saved an empty entry on "+ Add"). A value that is no Lighthouse category
/// keeps the config in force and is reported as a problem with a sentence, where it is.
#[test]
fn a_lighthouse_category_that_does_not_exist_is_a_warning() {
    let dir = copy_fixture("project");
    write_local(
        &dir,
        r#"{ "version": 1, "checks": { "lighthouse": { "categories": ["seo", "", "pwa"] } } }"#,
    );
    let loaded = load_project(dir.path()).expect("the config stays in force");
    let warnings: Vec<(&str, &str)> = loaded
        .warnings
        .iter()
        .map(|issue| (issue.path.as_str(), issue.key.as_str()))
        .collect();
    assert_eq!(
        warnings,
        [
            (
                "/checks/lighthouse/categories/1",
                crate::allowed::NOT_ALLOWED_KEY
            ),
            (
                "/checks/lighthouse/categories/2",
                crate::allowed::NOT_ALLOWED_KEY
            ),
        ]
    );
    assert!(
        loaded.warnings[0]
            .message
            .contains("performance, accessibility"),
        "the sentence names the allowed values: {}",
        loaded.warnings[0].message
    );
    assert_eq!(
        loaded.config.checks.lighthouse.categories,
        ["seo", "", "pwa"],
        "the value stays, so the settings window can show it and offer to remove it"
    );
}

#[test]
fn the_local_file_overrides_single_breakpoint_fields_by_id() {
    let dir = copy_fixture("project");
    write_local(
        &dir,
        r#"{ "breakpoints": [{ "id": "desktop-ff", "width": 1920 }], "sync": { "hover": false } }"#,
    );
    let loaded = load_project(dir.path()).expect("loads");
    let desktop = &loaded.config.breakpoints[1];
    assert_eq!(desktop.id, "desktop-ff");
    assert_eq!((desktop.width, desktop.height), (Some(1920), Some(900)));
    assert_eq!(desktop.engine, EngineKind::Firefox, "untouched fields stay");
    assert_eq!(loaded.config.breakpoints[0].width, Some(393));
    assert!(!loaded.config.sync.hover);
    assert_eq!(
        loaded.origins["breakpoints.desktop-ff.width"],
        Origin::Local
    );
    assert_eq!(
        loaded.origins["breakpoints.desktop-ff.height"],
        Origin::Project
    );
    assert_eq!(loaded.origins["sync.hover"], Origin::Local);
}

#[test]
fn the_network_keys_apply_from_no_project_file_until_trust_exists() {
    let dir = tempfile::tempdir().expect("temp dir");
    let shared = serde_json::json!({
        "version": 1,
        "network": { "ignoreCertificateErrors": true, "hostResolverRules": "MAP * 10.0.0.1" }
    });
    write_config_file(dir.path(), ConfigFile::Project, &shared).expect("write");
    let loaded = load_project(dir.path()).expect("loads");
    assert_eq!(loaded.config.network, crate::NetworkConfig::default());
    let keys: Vec<(&str, &str)> = loaded
        .warnings
        .iter()
        .map(|w| (w.path.as_str(), w.key.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            (
                "/network/ignoreCertificateErrors",
                "config.warning.local_only_ignored"
            ),
            (
                "/network/hostResolverRules",
                "config.warning.trust_required"
            ),
        ]
    );

    // A committed config.local.json is a project file too.
    write_local(
        &dir,
        r#"{ "network": { "ignoreCertificateErrors": true, "proxy": "evil:3128",
             "hostResolverRules": "MAP * 10.0.0.1" } }"#,
    );
    let loaded = load_project(dir.path()).expect("loads");
    assert_eq!(loaded.config.network, crate::NetworkConfig::default());
    assert_eq!(loaded.warnings.len(), 5, "{:?}", loaded.warnings);
    assert!(!loaded.origins.keys().any(|key| key.starts_with("network.")));
}

#[test]
fn problems_of_the_merged_config_point_into_it() {
    let dir = copy_fixture("project");
    write_local(&dir, r#"{ "breakpoints": [{ "id": "tablet" }] }"#);
    match load_project(dir.path()) {
        Err(ConfigError::Invalid(issues)) => {
            assert_eq!(issues[0].path, "/breakpoints/2/width");
            assert_eq!(issues[0].key, "config.error.size_required", "{issues:?}");
        }
        other => panic!("expected issues, got {other:?}"),
    }
}

#[test]
fn load_then_save_without_change_is_byte_identical() {
    let dir = copy_fixture("project");
    let path = ConfigFile::Project.path(dir.path());
    let before = fs::read(&path).expect("read");
    load_project(dir.path()).expect("loads");
    let value = read_json(&path).expect("reads");
    write_config_file(dir.path(), ConfigFile::Project, &value).expect("writes");
    assert_eq!(fs::read(&path).expect("read"), before);
}

#[test]
fn a_version_0_config_is_migrated_in_memory_and_the_file_stays() {
    let dir = copy_fixture("version-0");
    let path = ConfigFile::Project.path(dir.path());
    let original = fs::read(&path).expect("read");
    let loaded = load_project(dir.path()).expect("loads");
    assert_eq!(loaded.config.version, CURRENT_VERSION);
    assert_eq!(
        loaded.config.start_url.as_deref(),
        Some("http://localhost:8080")
    );
    assert_eq!(loaded.migrations.len(), 1);
    assert_eq!(loaded.migrations[0].key, "config.migration.v0_to_v1");
    assert_eq!(
        fs::read(&path).expect("read"),
        original,
        "loading writes nothing"
    );
    let entries = fs::read_dir(dir.path().join(".rbrowser"))
        .expect("dir")
        .count();
    assert_eq!(entries, 1, "no backup, no rewrite");

    let again = load_project(dir.path()).expect("loads again");
    assert_eq!(again.migrations.len(), 1, "the file is still version 0");
}

#[test]
fn a_newer_version_is_refused_without_touching_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let value = serde_json::json!({ "version": CURRENT_VERSION + 1 });
    write_config_file(dir.path(), ConfigFile::Project, &value).expect("write");
    assert!(matches!(
        load_project(dir.path()),
        Err(ConfigError::UnsupportedVersion { .. })
    ));
    let entries = fs::read_dir(dir.path().join(".rbrowser"))
        .expect("dir")
        .count();
    assert_eq!(entries, 2, "config.json and .gitignore only, no backup");
}

#[test]
fn a_missing_config_is_an_io_error_and_a_non_object_an_issue() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(
        load_project(dir.path()),
        Err(ConfigError::Io { .. })
    ));
    fs::create_dir_all(dir.path().join(".rbrowser")).expect("mkdir");
    fs::write(dir.path().join(".rbrowser/config.json"), "[1]").expect("write");
    match load_project(dir.path()) {
        Err(ConfigError::Invalid(issues)) => {
            assert_eq!(issues[0].key, "config.error.not_an_object");
        }
        other => panic!("expected an issue, got {other:?}"),
    }
}
