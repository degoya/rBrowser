use rb_sync::{ChangeData, SyncEvent, TargetData};
use serde_json::json;

use super::*;

fn target(path: &str) -> ElementTarget {
    serde_json::from_value(
        json!({ "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } }),
    )
    .expect("a target")
}

fn change(value: Option<&str>, checked: Option<bool>, selected: Option<Vec<u32>>) -> ChangeData {
    ChangeData {
        value: value.map(str::to_owned),
        checked,
        selected,
        sensitive: false,
        target: target("#field"),
    }
}

#[test]
fn every_control_s_change_is_applied_with_its_state() {
    for data in [
        change(Some("de"), None, Some(vec![1])),
        change(None, None, Some(vec![0, 2])),
        change(Some("express"), Some(true), None),
        change(Some("80"), None, None),
    ] {
        assert_eq!(
            plan(&SyncEvent::Change(data.clone())),
            Some(FormAction::Apply(data))
        );
    }
}

#[test]
fn a_file_input_s_change_is_skipped_with_its_field() {
    assert_eq!(
        plan(&SyncEvent::Change(change(None, None, None))),
        Some(FormAction::FileSkipped {
            field: "#field".to_owned()
        })
    );
}

#[test]
fn focus_is_focused_and_other_events_are_not_form_sync() {
    assert_eq!(
        plan(&SyncEvent::Focus(TargetData {
            target: target("#q")
        })),
        Some(FormAction::Focus(target("#q")))
    );
    let click: SyncEvent = serde_json::from_value(json!({
        "type": "click", "button": "left", "detail": 1, "modifiers": 0,
        "target": { "element": { "kind": "css", "path": "#q" }, "offset": { "x": 0.5, "y": 0.5 } },
    }))
    .expect("a click");
    assert_eq!(plan(&click), None);
}

#[test]
fn the_command_is_the_agent_s_apply_or_focus() {
    let data = change(Some("de"), None, Some(vec![1]));
    assert_eq!(
        FormAction::Apply(data.clone()).command(),
        Some(AgentCommand::Apply(data))
    );
    assert_eq!(
        FormAction::Focus(target("#q")).command(),
        Some(AgentCommand::Focus {
            target: target("#q")
        })
    );
    assert_eq!(
        FormAction::FileSkipped {
            field: "#f".to_owned()
        }
        .command(),
        None
    );
}

#[test]
fn a_password_never_shows_in_debug() {
    let mut data = change(Some("hunter2"), None, None);
    data.sensitive = true;
    let action = plan(&SyncEvent::Change(data)).expect("applied");
    let text = format!("{action:?} {:?}", action.command());
    assert!(!text.contains("hunter2"), "{text}");
}
