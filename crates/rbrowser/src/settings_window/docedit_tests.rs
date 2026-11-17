use serde_json::json;

use super::*;

fn text(value: &Value) -> String {
    rb_config::to_json_string(value)
}

#[test]
fn a_changed_field_changes_one_line_of_the_file() {
    let mut file = json!({
        "version": 1,
        "sync": { "scroll": true, "click": true },
        "layout": { "columns": 2, "gap": 8 }
    });
    let before = text(&file);
    let old = json!({ "columns": 2, "rows": 1, "gap": 8, "cells": [] });
    let new = json!({ "columns": 2, "rows": 1, "gap": 12, "cells": [] });
    set(&mut file, "layout", &old, &new).expect("set");
    let after = text(&file);
    let changed: Vec<(&str, &str)> = before
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(before.lines().count(), after.lines().count());
    assert_eq!(changed, vec![("    \"gap\": 8", "    \"gap\": 12")]);
}

#[test]
fn a_key_the_file_lacks_is_appended_with_only_what_changed() {
    let mut file = json!({ "version": 1 });
    let old = json!({ "columns": 1, "rows": 1, "gap": 8 });
    let new = json!({ "columns": 1, "rows": 1, "gap": 4 });
    set(&mut file, "layout", &old, &new).expect("set");
    assert_eq!(file, json!({ "version": 1, "layout": { "gap": 4 } }));
    set(&mut file, "sync.scroll", &json!(true), &json!(false)).expect("set");
    assert_eq!(file["sync"], json!({ "scroll": false }));
}

#[test]
fn list_items_are_updated_in_place_and_a_new_list_replaces_the_old() {
    let mut file = json!({ "breakpoints": [{ "id": "a", "width": 375, "height": 800 }] });
    let old = json!([{ "id": "a", "width": 375, "height": 800, "engine": "chromium" }]);
    let new = json!([{ "id": "a", "width": 390, "height": 800, "engine": "chromium" }]);
    set(&mut file, "breakpoints", &old, &new).expect("set");
    assert_eq!(
        file,
        json!({ "breakpoints": [{ "id": "a", "width": 390, "height": 800 }] })
    );
    let longer = json!([{ "id": "a" }, { "id": "b" }]);
    set(&mut file, "breakpoints", &new, &longer).expect("set");
    assert_eq!(file["breakpoints"], longer);
}

#[test]
fn a_removed_object_field_leaves_the_file() {
    let mut file = json!({ "emulation": { "headers": { "X-A": "1", "X-B": "2" } } });
    set(
        &mut file,
        "emulation.headers",
        &json!({ "X-A": "1", "X-B": "2" }),
        &json!({ "X-B": "2" }),
    )
    .expect("set");
    assert_eq!(file, json!({ "emulation": { "headers": { "X-B": "2" } } }));
}

#[test]
fn reset_removes_the_key_and_the_objects_left_empty() {
    let mut file = json!({ "version": 1, "network": { "ignoreCertificateErrors": true }, "z": 1 });
    remove(&mut file, "network.ignoreCertificateErrors");
    assert_eq!(file, json!({ "version": 1, "z": 1 }));
    let order: Vec<&String> = file.as_object().expect("object").keys().collect();
    assert_eq!(order, ["version", "z"]);
    remove(&mut file, "not.there");
    assert_eq!(file, json!({ "version": 1, "z": 1 }));
}

#[test]
fn a_parent_that_is_no_object_is_refused() {
    let mut file = json!({ "sync": true });
    assert!(set(&mut file, "sync.scroll", &json!(true), &json!(false)).is_err());
    assert_eq!(lookup(&file, "sync"), Some(&json!(true)));
    assert_eq!(lookup(&file, "sync.scroll"), None);
}
