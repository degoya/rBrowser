use rb_sync::{MOD_CTRL, MouseButton, SyncEvent};
use serde_json::{Value, json};

use super::*;

fn target() -> Value {
    json!({ "element": { "kind": "css", "path": "#b" }, "offset": { "x": 0.5, "y": 0.5 } })
}

/// An event as the agent sends it.
fn event(mut fields: Value) -> SyncEvent {
    if fields.get("target").is_none() && fields["type"] != "keydown" && fields["type"] != "keyup" {
        fields["target"] = target();
    }
    serde_json::from_value(fields).expect("an event")
}

fn click(kind: &str, button: &str, detail: u32) -> SyncEvent {
    event(json!({ "type": kind, "button": button, "detail": detail, "modifiers": MOD_CTRL }))
}

const TOUCH: Options = Options {
    touch: true,
    context_menu: false,
};

#[test]
fn a_click_is_a_click_with_its_count_and_modifiers() {
    match plan(&click("click", "left", 1), Options::default()) {
        Action::Pointer { gesture, .. } => assert_eq!(
            gesture,
            Gesture::Click {
                button: MouseButton::Left,
                count: 1,
                modifiers: MOD_CTRL
            }
        ),
        other => panic!("a click, not {other:?}"),
    }
    match plan(&click("click", "left", 2), Options::default()) {
        Action::Pointer {
            gesture: Gesture::Click { count, .. },
            ..
        } => assert_eq!(count, 2),
        other => panic!("a click, not {other:?}"),
    }
}

#[test]
fn a_click_a_key_or_a_label_made_is_not_replayed_twice() {
    assert_eq!(
        plan(&click("click", "left", 0), Options::default()),
        Action::Skip
    );
}

#[test]
fn dblclick_comes_from_the_second_click() {
    assert_eq!(
        plan(&click("dblclick", "left", 2), Options::default()),
        Action::Skip
    );
}

#[test]
fn a_touch_view_gets_a_tap_and_no_double_tap() {
    match plan(&click("click", "left", 1), TOUCH) {
        Action::Pointer { gesture, .. } => {
            assert_eq!(
                gesture,
                Gesture::Tap {
                    modifiers: MOD_CTRL
                }
            );
        }
        other => panic!("a tap, not {other:?}"),
    }
    assert_eq!(plan(&click("click", "left", 2), TOUCH), Action::Skip);
    assert_eq!(
        plan(&event(json!({ "type": "hover" })), TOUCH),
        Action::Unsupported
    );
}

#[test]
fn the_context_menu_follows_its_switch() {
    let menu = click("contextmenu", "right", 0);
    assert_eq!(plan(&menu, Options::default()), Action::Skip);
    let on = Options {
        touch: false,
        context_menu: true,
    };
    match plan(&menu, on) {
        Action::Pointer { gesture, .. } => assert_eq!(
            gesture,
            Gesture::Click {
                button: MouseButton::Right,
                count: 1,
                modifiers: MOD_CTRL
            }
        ),
        other => panic!("a right click, not {other:?}"),
    }
}

#[test]
fn hover_is_a_hover() {
    match plan(&event(json!({ "type": "hover" })), Options::default()) {
        Action::Pointer { gesture, .. } => assert_eq!(gesture, Gesture::Hover),
        other => panic!("a hover, not {other:?}"),
    }
}

#[test]
fn keys_need_no_resolution() {
    let down = event(json!({
        "type": "keydown", "key": "Tab", "code": "Tab", "keyCode": 9, "location": 0,
        "repeat": false, "modifiers": 0,
    }));
    match plan(&down, Options::default()) {
        Action::Key(params) => assert_eq!(params["type"], "rawKeyDown"),
        other => panic!("a key, not {other:?}"),
    }
    let up = event(json!({
        "type": "keyup", "key": "Tab", "code": "Tab", "keyCode": 9, "location": 0,
        "repeat": false, "modifiers": 0,
    }));
    match plan(&up, Options::default()) {
        Action::Key(params) => assert_eq!(params["type"], "keyUp"),
        other => panic!("a key, not {other:?}"),
    }
}

#[test]
fn only_typed_text_is_inserted() {
    let typed = event(json!({ "type": "input", "inputType": "insertText", "data": "x" }));
    assert!(
        matches!(plan(&typed, Options::default()), Action::Text { ref text, .. } if text == "x")
    );
    for (input_type, data) in [
        ("deleteContentBackward", None),
        ("insertFromPaste", Some("pasted")),
        ("insertCompositionText", Some("\u{304b}")),
        ("insertLineBreak", None),
        ("insertText", Some("")),
    ] {
        let mut fields = json!({ "type": "input", "inputType": input_type });
        if let Some(data) = data {
            fields["data"] = json!(data);
        }
        assert_eq!(
            plan(&event(fields), Options::default()),
            Action::Skip,
            "{input_type}"
        );
    }
}

#[test]
fn a_password_never_shows_in_the_plan_s_debug_output() {
    let typed = event(json!({
        "type": "input", "inputType": "insertText", "data": "s3cret", "value": "s3cret",
        "sensitive": true,
    }));
    let action = plan(&typed, Options::default());
    assert!(matches!(action, Action::Text { .. }));
    assert!(!format!("{action:?}").contains("s3cret"));
}

#[test]
fn the_rest_is_left_to_other_jobs() {
    for fields in [
        json!({ "type": "pointer", "phase": "down", "button": "left", "buttons": 1,
                "pointerType": "mouse", "modifiers": 0 }),
        json!({ "type": "wheel", "deltaX": 0.0, "deltaY": 40.0, "deltaMode": 0, "modifiers": 0 }),
        json!({ "type": "scroll", "x": 0.0, "y": 10.0, "maxX": 0.0, "maxY": 900.0 }),
        json!({ "type": "submit", "method": "post", "action": "http://localhost/f" }),
    ] {
        let fields = if fields["type"] == "scroll" {
            fields
        } else {
            let mut fields = fields;
            fields["target"] = target();
            fields
        };
        let parsed: SyncEvent = serde_json::from_value(fields).expect("an event");
        assert_eq!(plan(&parsed, Options::default()), Action::Unsupported);
    }
}

/// RB-020-05: change and focus are form sync's, a file input's change is skipped.
#[test]
fn change_and_focus_are_form_sync() {
    let change = event(json!({ "type": "change", "checked": true, "target": target() }));
    let focus = event(json!({ "type": "focus", "target": target() }));
    let file = event(json!({ "type": "change", "target": target() }));
    assert!(matches!(
        plan(&change, Options::default()),
        Action::Form(crate::form_sync::FormAction::Apply(_))
    ));
    assert!(matches!(
        plan(&focus, Options::default()),
        Action::Form(crate::form_sync::FormAction::Focus(_))
    ));
    assert!(matches!(
        plan(&file, Options::default()),
        Action::Form(crate::form_sync::FormAction::FileSkipped { .. })
    ));
}
