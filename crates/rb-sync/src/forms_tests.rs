use super::*;
use crate::event::{ChangeData, InputData, KeyData};
use crate::switches::tests::target;

fn password_input() -> SyncEvent {
    SyncEvent::Input(InputData {
        input_type: "insertText".into(),
        data: Some("2".into()),
        value: Some("hunter2".into()),
        sensitive: true,
        target: target("#password"),
    })
}

fn password_change() -> SyncEvent {
    SyncEvent::Change(ChangeData {
        value: Some("hunter2".into()),
        checked: None,
        selected: None,
        sensitive: true,
        target: target("#password"),
    })
}

#[test]
fn a_sensitive_value_is_masked_for_mcp_and_the_recorder() {
    for event in [password_input(), password_change()] {
        let shown = redact(&event);
        let json = serde_json::to_string(&shown).expect("serializes");
        assert!(!json.contains("hunter2"), "{json}");
        assert!(json.contains(MASK), "{json}");
        // The target stays: MCP and the recorder still say which field.
        assert!(json.contains("#password"), "{json}");
    }
}

#[test]
fn other_values_stay_as_they_are() {
    let city = SyncEvent::Change(ChangeData {
        value: Some("Berlin".into()),
        checked: None,
        selected: None,
        sensitive: false,
        target: target("#city"),
    });
    assert_eq!(redact(&city), city);
    let key = SyncEvent::Keydown(KeyData {
        key: "a".into(),
        code: "KeyA".into(),
        key_code: 65,
        location: 0,
        repeat: false,
        modifiers: 0,
        sensitive: false,
        target: None,
    });
    assert_eq!(redact(&key), key);
}

/// A key typed into a password field (the security fix of RB-020-01's capture) is masked like
/// the field's value: the key, its code and its key code.
#[test]
fn a_sensitive_key_is_masked_for_mcp_and_the_recorder() {
    let key = KeyData {
        key: "h".into(),
        code: "KeyH".into(),
        key_code: 72,
        location: 0,
        repeat: false,
        modifiers: 0,
        sensitive: true,
        target: Some(target("#password")),
    };
    for event in [SyncEvent::Keydown(key.clone()), SyncEvent::Keyup(key)] {
        assert!(is_sensitive(&event));
        let shown = redact(&event);
        let json = serde_json::to_string(&shown).expect("serializes");
        assert!(
            !json.contains("\"h\"") && !json.contains("KeyH") && !json.contains("72"),
            "{json}"
        );
        assert!(json.contains(MASK) && json.contains("#password"), "{json}");
    }
}

#[test]
fn is_sensitive_names_password_events_only() {
    assert!(is_sensitive(&password_input()));
    assert!(is_sensitive(&password_change()));
    assert!(!is_sensitive(&SyncEvent::Focus(crate::event::TargetData {
        target: target("#password"),
    })));
}

/// A file input's change carries no value: nothing to set, skipped (RB-020-05).
#[test]
fn a_change_without_state_is_a_skipped_file_input() {
    let file = ChangeData {
        value: None,
        checked: None,
        selected: None,
        sensitive: false,
        target: target("#avatar"),
    };
    assert!(is_file_skip(&file));
    let select = ChangeData {
        selected: Some(vec![0]),
        ..file.clone()
    };
    assert!(!is_file_skip(&select));
}
