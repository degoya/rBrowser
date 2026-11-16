use serde_json::json;

use super::*;

#[test]
fn assign_takes_a_target_and_a_session() {
    let args: AssignArgs =
        serde_json::from_value(json!({ "target": 3, "session": "admin" })).expect("parses");
    assert_eq!(args.target, Some(Target::View(ViewId(3))));
    assert_eq!(args.session, "admin");
    let all: AssignArgs =
        serde_json::from_value(json!({ "target": "all", "session": "default" })).expect("parses");
    assert_eq!(all.target, Some(Target::All));
    let active: AssignArgs = serde_json::from_value(json!({ "session": "qa" })).expect("parses");
    assert_eq!(active.target, None);
}

#[test]
fn unknown_or_missing_fields_are_refused() {
    assert!(serde_json::from_value::<AssignArgs>(json!({ "target": 1 })).is_err());
    assert!(
        serde_json::from_value::<AssignArgs>(json!({ "session": "a", "path": "../x" })).is_err()
    );
    assert!(serde_json::from_value::<ClearArgs>(json!({})).is_err());
}

#[test]
fn the_answers_are_camel_case() {
    let answer = AssignAnswer {
        session: "admin".to_owned(),
        moved: vec![Moved {
            from: ViewId(2),
            view: ViewId(7),
        }],
    };
    assert_eq!(
        serde_json::to_value(answer).expect("serializes"),
        json!({ "session": "admin", "moved": [{ "from": 2, "view": 7 }] })
    );
    let cleared = ClearAnswer {
        session: "admin".to_owned(),
        views: vec![ViewId(7)],
        origins: vec!["https://a.test".to_owned()],
    };
    assert_eq!(
        serde_json::to_value(cleared).expect("serializes"),
        json!({ "session": "admin", "views": [7], "origins": ["https://a.test"] })
    );
}

#[test]
fn tool_names_follow_the_core_form() {
    for name in [ASSIGN_TOOL, CLEAR_TOOL] {
        assert!(name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
    }
}
