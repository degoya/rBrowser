use serde_json::json;

use super::*;

fn doc() -> Value {
    json!({
        "user": { "name": "Ada", "id": 7, "full name": "Ada Lovelace" },
        "items": [{ "id": 1 }, { "id": 2 }, { "id": 3 }],
        "tags": ["admin", "ops"]
    })
}

fn picked(path: &str) -> Vec<Value> {
    select(&doc(), path)
        .expect("a path")
        .into_iter()
        .cloned()
        .collect()
}

#[test]
fn the_subset_selects_as_the_panel_does() {
    assert_eq!(picked("$"), vec![doc()]);
    assert_eq!(picked("$.user.name"), vec![json!("Ada")]);
    assert_eq!(
        picked("$['user']['full name']"),
        vec![json!("Ada Lovelace")]
    );
    assert_eq!(picked("$.items[1].id"), vec![json!(2)]);
    assert_eq!(picked("$.items[-1].id"), vec![json!(3)]);
    assert_eq!(picked("$.items[*].id"), vec![json!(1), json!(2), json!(3)]);
    assert_eq!(picked("$.tags.*"), vec![json!("admin"), json!("ops")]);
    assert_eq!(
        picked("$..id"),
        vec![json!(7), json!(1), json!(2), json!(3)]
    );
}

#[test]
fn a_missing_step_selects_nothing() {
    assert!(picked("$.user.age").is_empty());
    assert!(picked("$.items[9]").is_empty());
    assert!(picked("$.tags.name").is_empty());
}

#[test]
fn what_is_no_path_of_the_subset_is_refused() {
    for path in [
        "",
        "user",
        "$.",
        "$[",
        "$[?(@.id)]",
        "$.items[1:2]",
        "$ name",
    ] {
        assert!(select(&doc(), path).is_none(), "{path}");
    }
}
