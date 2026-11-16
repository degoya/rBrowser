use serde_json::json;

use super::*;

/// RB-090-52: `capture.video.format` is gone; a file that still sets it loads, the key is
/// dropped with one warning that says why, and nothing else is touched.
#[test]
fn a_retired_key_is_dropped_with_a_warning() {
    let mut value = json!({
        "version": 1,
        "capture": { "video": { "format": "mp4", "fps": 24 } }
    });
    let warnings = strip_retired(&mut value);
    assert_eq!(
        value,
        json!({ "version": 1, "capture": { "video": { "fps": 24 } } })
    );
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].key, RETIRED_KEY);
    assert_eq!(warnings[0].path, "/capture/video/format");
    assert!(
        warnings[0].message.contains("capture.video.format"),
        "{}",
        warnings[0].message
    );
    assert!(
        warnings[0].message.contains("WebM"),
        "{}",
        warnings[0].message
    );
}

#[test]
fn a_file_without_retired_keys_is_left_alone() {
    let mut value = json!({ "capture": { "video": { "fps": 24 } }, "format": "mp4" });
    let before = value.clone();
    assert!(strip_retired(&mut value).is_empty());
    assert_eq!(value, before, "only the retired key's own place counts");
    let mut odd = json!({ "capture": "not an object" });
    assert!(strip_retired(&mut odd).is_empty());
}

/// The project config: an old `config.json` with the key loads and names it.
#[test]
fn an_old_project_config_with_the_format_still_loads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let rbrowser = dir.path().join(".rbrowser");
    std::fs::create_dir_all(&rbrowser).expect("folder");
    std::fs::write(
        rbrowser.join("config.json"),
        r#"{ "version": 1, "capture": { "video": { "format": "mp4", "quality": 70 } } }"#,
    )
    .expect("config");
    let loaded = crate::load_project(dir.path()).expect("loads");
    assert_eq!(loaded.config.capture.video.quality, Some(70));
    assert!(
        loaded
            .warnings
            .iter()
            .any(|warning| warning.key == RETIRED_KEY),
        "{:?}",
        loaded.warnings
    );
}

/// The user settings: an old `settings.json` with the key loads and names it.
#[test]
fn old_user_settings_with_the_format_still_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{ "capture": { "video": { "format": "mp4", "fps": 15 } } }"#,
    )
    .expect("settings");
    let (settings, warnings) = crate::load_user_settings_with_warnings(&path).expect("loads");
    assert_eq!(settings.capture.video.fps, Some(15));
    assert!(
        warnings.iter().any(|warning| warning.key == RETIRED_KEY),
        "{warnings:?}"
    );
}
