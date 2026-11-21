use super::*;
use serde_json::json;

#[test]
fn canonical_text_is_two_spaces_in_key_order_with_a_newline() {
    let value = json!({ "version": 1, "b": [1, 2], "a": { "z": true, "y": null } });
    let text = to_json_string(&value);
    assert_eq!(
        text,
        "{\n  \"version\": 1,\n  \"b\": [\n    1,\n    2\n  ],\n  \"a\": {\n    \"z\": true,\n    \"y\": null\n  }\n}\n"
    );
}

#[test]
fn a_canonical_file_survives_read_and_write_byte_for_byte() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.json");
    let original = "{\n  \"version\": 1,\n  \"visual\": {\n    \"threshold\": 0.1,\n    \"fullPage\": true\n  },\n  \"name\": \"caf\u{e9}\"\n}\n";
    fs::write(&path, original).expect("write");
    let value = read_json(&path).expect("reads");
    write_json(&path, &value).expect("writes");
    assert_eq!(fs::read_to_string(&path).expect("read back"), original);
}

#[test]
fn a_syntax_error_names_line_and_column() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.json");
    fs::write(&path, "{\n  \"version\": 1,\n  oops\n}\n").expect("write");
    match read_json(&path) {
        Err(ConfigError::Parse { line, column, .. }) => assert_eq!((line, column), (3, 3)),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

#[test]
fn a_missing_file_is_an_io_error_with_its_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("absent.json");
    match read_json(&path) {
        Err(ConfigError::Io { path: reported, .. }) => assert_eq!(reported, path),
        other => panic!("expected an io error, got {other:?}"),
    }
}

#[test]
fn the_first_write_creates_the_gitignore_and_keeps_an_existing_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_config_file(dir.path(), ConfigFile::Local, &json!({ "version": 1 })).expect("writes");
    let gitignore = dir.path().join(".rbrowser/.gitignore");
    assert_eq!(
        fs::read_to_string(&gitignore).expect("gitignore"),
        GITIGNORE
    );
    assert!(dir.path().join(".rbrowser/config.local.json").is_file());

    fs::write(&gitignore, "custom\n").expect("edit");
    write_config_file(dir.path(), ConfigFile::Project, &json!({ "version": 1 })).expect("writes");
    assert_eq!(
        fs::read_to_string(&gitignore).expect("gitignore"),
        "custom\n"
    );
    let leftovers: Vec<_> = fs::read_dir(dir.path().join(".rbrowser"))
        .expect("dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no temporary file is left behind");
}

#[test]
fn saving_over_an_older_version_backs_the_file_up_and_git_ignores_the_backup() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path().join(".rbrowser");
    fs::create_dir_all(&config_dir).expect("mkdir");
    let path = config_dir.join("config.json");
    let original = "{\n  \"version\": 0\n}\n";
    fs::write(&path, original).expect("write");
    write_config_file(dir.path(), ConfigFile::Project, &json!({ "version": 1 })).expect("saves");
    assert_eq!(
        fs::read_to_string(config_dir.join("config.json.bak-v0")).expect("backup"),
        original
    );
    assert_eq!(read_json(&path).expect("reads")["version"], 1);
    assert!(
        GITIGNORE.lines().any(|line| line == "*.bak-v*"),
        "the backups stay out of the repository"
    );

    write_config_file(dir.path(), ConfigFile::Project, &json!({ "version": 1 })).expect("saves");
    assert!(
        !config_dir.join("config.json.bak-v1").exists(),
        "a current file needs no backup"
    );
}
