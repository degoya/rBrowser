use super::*;
use serde_json::json;

#[test]
fn every_version_below_the_current_one_has_a_step() {
    assert_eq!(STEPS.len(), CURRENT_VERSION as usize);
}

#[test]
fn version_0_moves_to_the_current_version_with_a_note() {
    let (value, notes) =
        migrate(json!({ "version": 0, "startUrl": "http://localhost" })).expect("migrates");
    assert_eq!(
        value,
        json!({ "version": CURRENT_VERSION, "startUrl": "http://localhost" })
    );
    assert_eq!(notes.len(), 1);
    assert_eq!((notes[0].from, notes[0].to), (0, 1));
    assert_eq!(notes[0].key, "config.migration.v0_to_v1");
}

#[test]
fn a_current_config_is_unchanged_without_notes() {
    let input = json!({ "version": CURRENT_VERSION, "sync": { "scroll": false } });
    let (value, notes) = migrate(input.clone()).expect("migrates");
    assert_eq!(value, input);
    assert!(notes.is_empty());
}

#[test]
fn a_newer_version_is_refused() {
    match migrate(json!({ "version": CURRENT_VERSION + 1 })) {
        Err(ConfigError::UnsupportedVersion { found, supported }) => {
            assert_eq!((found, supported), (CURRENT_VERSION + 1, CURRENT_VERSION));
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn a_missing_or_malformed_version_is_an_issue_at_version() {
    for (input, key) in [
        (json!({}), "config.error.missing_version"),
        (json!({ "version": "1" }), "config.error.invalid_version"),
        (json!({ "version": -1 }), "config.error.invalid_version"),
    ] {
        match migrate(input) {
            Err(ConfigError::Invalid(issues)) => {
                assert_eq!(issues[0].path, "/version");
                assert_eq!(issues[0].key, key);
            }
            other => panic!("expected an issue, got {other:?}"),
        }
    }
}

#[test]
fn a_backup_never_overwrites_an_older_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.json");
    fs::write(&path, "first").expect("write");
    let first = write_backup(&path, 0).expect("backup");
    assert_eq!(first, dir.path().join("config.json.bak-v0"));
    fs::write(&path, "second").expect("write");
    let second = write_backup(&path, 0).expect("backup");
    assert_eq!(second, dir.path().join("config.json.bak-v0-2"));
    assert_eq!(fs::read_to_string(first).expect("read"), "first");
    assert_eq!(fs::read_to_string(second).expect("read"), "second");
}
