use serde_json::json;

use super::*;

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn an_opened_project_moves_to_the_front_once() {
    let recent = list(&["C:\\a", "C:\\b", "C:\\c"]);
    assert_eq!(
        push_recent(&recent, "c:/b"),
        list(&["c:/b", "C:\\a", "C:\\c"])
    );
    assert_eq!(push_recent(&[], "C:\\a"), list(&["C:\\a"]));
}

#[test]
fn the_list_keeps_the_newest_ten() {
    let many: Vec<String> = (0..12).map(|i| format!("C:\\p{i}")).collect();
    let pushed = push_recent(&many, "C:\\new");
    assert_eq!(pushed.len(), MAX_RECENT);
    assert_eq!(pushed[0], "C:\\new");
    assert_eq!(pushed[MAX_RECENT - 1], "C:\\p8");
}

#[test]
fn without_arguments_the_newest_other_project_is_picked() {
    let recent = list(&["C:\\a", "C:\\b"]);
    assert_eq!(
        pick(&recent, Some(Path::new("C:\\A")), &Value::Null).expect("picks"),
        "C:\\b"
    );
    assert_eq!(pick(&recent, None, &json!({})).expect("picks"), "C:\\a");
    assert!(matches!(
        pick(&list(&["C:\\a"]), Some(Path::new("C:\\a")), &Value::Null),
        Err(CommandError::Failed(key)) if key == NO_RECENT_KEY
    ));
}

#[test]
fn a_path_or_index_must_name_a_recent_project() {
    let recent = list(&["C:\\a", "C:\\b"]);
    assert_eq!(
        pick(&recent, None, &json!({ "index": 1 })).expect("picks"),
        "C:\\b"
    );
    assert_eq!(
        pick(&recent, None, &json!({ "path": "c:/b" })).expect("picks"),
        "C:\\b"
    );
    for args in [
        json!({ "index": 2 }),
        json!({ "path": "C:\\Windows" }),
        json!({ "path": 1 }),
        json!({ "command": "calc" }),
        json!("C:\\a"),
    ] {
        assert!(
            matches!(
                pick(&recent, None, &args),
                Err(CommandError::InvalidArgs(_))
            ),
            "{args}"
        );
    }
}

#[test]
fn a_key_is_set_in_the_settings_file_keeping_the_rest() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{\"ui\":{\"theme\":\"dark\"}}").expect("writes");
    set_recent(&path, &list(&["C:\\a"])).expect("sets");
    let written = rb_config::read_json(&path).expect("reads");
    assert_eq!(
        written,
        json!({ "ui": { "theme": "dark" }, "recentProjects": ["C:\\a"] })
    );
    let newer = format!("{{ \"version\": {} }}", rb_config::CURRENT_VERSION + 1);
    std::fs::write(&path, &newer).expect("writes");
    assert!(set_recent(&path, &list(&["C:\\b"])).is_err());
    assert_eq!(std::fs::read_to_string(&path).expect("reads"), newer);
}
