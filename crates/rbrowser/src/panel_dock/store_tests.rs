use rb_config::{PanelDock, PanelDockState};
use serde_json::Map;

use super::*;

#[test]
fn only_the_keys_that_changed_are_named() {
    let before = PanelSettings::default();
    assert!(changed(&before, &before).is_empty());
    let mut after = before;
    after.dock = PanelDock::Left;
    after.dock_state = PanelDockState::Open;
    PanelSide::Bottom.set_in(&mut after.dock_size, 250);
    assert_eq!(
        changed(&before, &after),
        [
            ("panels.dock".to_owned(), json!("left")),
            ("panels.dockState".to_owned(), json!("open")),
            ("panels.dockSize.bottom".to_owned(), json!(250)),
        ]
    );
}

#[test]
fn a_dotted_key_is_set_and_the_other_keys_stay() {
    let mut root: Map<String, Value> = serde_json::from_value(json!({
        "ui": { "theme": "dark" },
        "panels": { "dockSize": { "left": 300 } },
        "odd": 1
    }))
    .expect("an object");
    set_path(&mut root, "panels.dockSize.bottom", json!(250));
    set_path(&mut root, "panels.dock", json!("bottom"));
    set_path(&mut root, "odd.inner", json!(true));
    assert_eq!(
        Value::Object(root),
        json!({
            "ui": { "theme": "dark" },
            "panels": { "dock": "bottom", "dockSize": { "left": 300, "bottom": 250 } },
            "odd": { "inner": true }
        })
    );
}

/// RB-080-93: once `dockState` is written the file no longer holds RB-080-61's key.
#[test]
fn the_old_collapsed_key_goes_with_the_new_state() {
    let mut root: Map<String, Value> = serde_json::from_value(json!({
        "panels": { "dockCollapsed": true, "dock": "left" }
    }))
    .expect("an object");
    drop_legacy(&mut root);
    set_path(&mut root, STATE_KEY, json!("rail"));
    assert_eq!(
        Value::Object(root),
        json!({ "panels": { "dock": "left", "dockState": "rail" } })
    );
}

/// RB-090-30 (seen on Windows): the write of an earlier change that runs after the write of a
/// later one still leaves the later value in the file.
#[test]
fn a_late_write_of_an_earlier_change_keeps_the_newest_value() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let key = STATE_KEY.to_owned();
    // The fold, then the undock, in the order of the changes on the calling thread.
    lock(&LATEST).insert((path.clone(), key.clone()), json!("rail"));
    lock(&LATEST).insert((path.clone(), key.clone()), json!("open"));
    // The undock's write runs first, the fold's last.
    write_latest(&path, std::slice::from_ref(&key));
    write_latest(&path, std::slice::from_ref(&key));
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    assert_eq!(file["panels"]["dockState"], "open");
}
