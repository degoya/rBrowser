use serde_json::json;

use super::*;

fn newer() -> String {
    format!(
        "{{ \"version\": {}, \"ui\": {{ \"theme\": \"dark\" }} }}",
        CURRENT_VERSION + 1
    )
}

#[test]
fn a_change_keeps_every_other_key_and_makes_the_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{\"ui\":{\"theme\":\"dark\"}}").expect("writes");
    update(&path, |map| {
        map.insert("recentProjects".to_owned(), json!(["C:\\a"]));
    })
    .expect("updates");
    assert_eq!(
        rb_config::read_json(&path).expect("reads"),
        json!({ "ui": { "theme": "dark" }, "recentProjects": ["C:\\a"] })
    );
    let fresh = dir.path().join("new").join("settings.json");
    update(&fresh, |map| {
        map.insert("a".to_owned(), json!(1));
    })
    .expect("creates");
    assert_eq!(
        rb_config::read_json(&fresh).expect("reads"),
        json!({ "a": 1 })
    );
}

#[test]
fn a_file_that_does_not_read_is_newer_or_no_object_is_kept() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    for content in ["{ broken".to_owned(), newer(), "[1, 2]".to_owned()] {
        std::fs::write(&path, &content).expect("writes");
        let error = update(&path, |map| {
            map.insert("a".to_owned(), json!(1));
        })
        .expect_err("kept");
        assert!(matches!(error, UserFileError::Kept), "{content}: {error}");
        let error = write(&path, &json!({ "a": 1 })).expect_err("kept");
        assert!(matches!(error, UserFileError::Kept), "{content}: {error}");
        assert_eq!(std::fs::read_to_string(&path).expect("reads"), content);
    }
}

#[test]
fn a_whole_document_is_written_over_a_known_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, format!("{{ \"version\": {CURRENT_VERSION} }}")).expect("writes");
    write(&path, &json!({ "version": CURRENT_VERSION, "a": 1 })).expect("writes");
    assert_eq!(
        rb_config::read_json(&path).expect("reads"),
        json!({ "version": CURRENT_VERSION, "a": 1 })
    );
}
