use super::*;
use std::fs;

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn windows_uses_appdata_rbrowser() {
    let path = settings_path_for(true, env(&[("APPDATA", r"C:\Users\a\AppData\Roaming")]));
    let expected = PathBuf::from(r"C:\Users\a\AppData\Roaming")
        .join("rBrowser")
        .join("settings.json");
    assert_eq!(path, Some(expected));
    assert_eq!(settings_path_for(true, env(&[("APPDATA", "")])), None);
    assert_eq!(settings_path_for(true, env(&[])), None);
}

#[test]
fn elsewhere_xdg_comes_before_home() {
    assert_eq!(
        settings_path_for(
            false,
            env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/home/a")])
        ),
        Some(PathBuf::from("/x/rbrowser/settings.json"))
    );
    assert_eq!(
        settings_path_for(false, env(&[("HOME", "/home/a")])),
        Some(PathBuf::from("/home/a/.config/rbrowser/settings.json"))
    );
    assert_eq!(settings_path_for(false, env(&[])), None);
}

#[test]
fn a_missing_file_gives_the_defaults_at_the_current_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings = load_user_settings(&dir.path().join("settings.json")).expect("defaults");
    assert_eq!(settings.version, CURRENT_VERSION);
    assert_eq!(settings.ui.language, "en");
    assert!(settings.shortcuts.disable_in_devtools);
}

#[test]
fn a_file_loads_and_a_bad_value_names_its_pointer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    fs::write(
        &path,
        r#"{ "version": 1, "cdp": { "enabled": true, "port": 9333 } }"#,
    )
    .expect("write");
    let settings = load_user_settings(&path).expect("loads");
    assert_eq!(settings.cdp.port, Some(9333));

    fs::write(&path, r#"{ "version": 1, "cdp": { "port": 70000 } }"#).expect("write");
    let (settings, warnings) = load_user_settings_with_warnings(&path).expect("loads");
    assert_eq!(settings.cdp.port, None, "the default");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].path, "/cdp/port");
    assert_eq!(warnings[0].key, "config.error.invalid_value");

    // RB-080-72: a string of a few fixed values is held to them (`enum`), the default applies.
    fs::write(
        &path,
        r#"{ "version": 1, "engines": { "cookies": { "copy": "sometimes" } } }"#,
    )
    .expect("write");
    let (settings, warnings) = load_user_settings_with_warnings(&path).expect("loads");
    assert_eq!(settings.engines.cookies.copy, "manual", "the default");
    assert_eq!(warnings[0].path, "/engines/cookies/copy");

    fs::write(&path, "[1]").expect("write");
    assert!(matches!(
        load_user_settings(&path),
        Err(ConfigError::Invalid(_))
    ));
}

#[test]
fn a_value_outside_the_schema_falls_back_to_its_default_and_the_rest_applies() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    fs::write(
        &path,
        r#"{
            "version": 1,
            "shortcuts": { "leaderTimeoutMs": 20000, "disableInDevtools": false },
            "logs": { "crashDumpsInBundle": 99, "level": "loud", "maxFiles": 7 },
            "ui": { "theme": "purple" },
            "cdp": { "enabled": true }
        }"#,
    )
    .expect("write");
    let (settings, warnings) = load_user_settings_with_warnings(&path).expect("loads");
    assert_eq!(settings.shortcuts.leader_timeout_ms, 0);
    assert!(!settings.shortcuts.disable_in_devtools);
    assert_eq!(settings.logs.crash_dumps_in_bundle, 3);
    assert_eq!(settings.logs.level, "info");
    assert_eq!(settings.logs.max_files, 7);
    assert_eq!(settings.ui.theme, crate::Theme::System);
    assert!(settings.cdp.enabled);
    let mut paths: Vec<&str> = warnings.iter().map(|w| w.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "/logs/crashDumpsInBundle",
            "/logs/level",
            "/shortcuts/leaderTimeoutMs",
            "/ui/theme"
        ]
    );
    assert!(warnings[0].message.contains("default"), "{warnings:?}");
    assert_eq!(
        load_user_settings(&path)
            .expect("loads")
            .shortcuts
            .leader_timeout_ms,
        0
    );
}

#[test]
fn a_newer_version_and_broken_json_are_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    fs::write(&path, r#"{ "version": 99 }"#).expect("write");
    assert!(matches!(
        load_user_settings(&path),
        Err(ConfigError::UnsupportedVersion { found: 99, .. })
    ));
    fs::write(&path, "{").expect("write");
    assert!(matches!(
        load_user_settings(&path),
        Err(ConfigError::Parse { .. })
    ));
}
