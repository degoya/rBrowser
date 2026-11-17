use serde_json::json;

use super::*;

#[test]
fn the_defaults_parse_from_an_empty_object() {
    let settings: LogSettings = serde_json::from_value(json!({})).expect("defaults");
    assert_eq!(settings, LogSettings::default());
    assert_eq!(settings.level, LogLevel::Info);
    assert_eq!(settings.max_file_size_mb, DEFAULT_MAX_FILE_SIZE_MB);
    assert_eq!(settings.max_files, DEFAULT_MAX_FILES);
    assert_eq!(settings.crash_dumps_in_bundle, DEFAULT_CRASH_DUMPS);
}

#[test]
fn the_settings_read_their_camel_case_keys() {
    let settings: LogSettings = serde_json::from_value(json!({
        "level": "debug",
        "maxFileSizeMb": 2,
        "maxFiles": 3,
        "crashDumpsInBundle": 0
    }))
    .expect("parse");
    assert_eq!(settings.level, LogLevel::Debug);
    assert_eq!(settings.rotation(), RotationPolicy::from_settings(2, 3));
    assert_eq!(settings.crash_dumps_in_bundle, 0);
}
