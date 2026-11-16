use rb_engine::{Button, InputEvent, Phase};
use serde_json::json;

use super::*;

fn mouse(phase: Phase, button: Button, modifiers: u8) -> InputEvent {
    InputEvent::Mouse {
        phase,
        x: 10.4,
        y: 20.6,
        button,
        clicks: 1,
        modifiers,
    }
}

#[test]
fn a_press_moves_then_presses_in_css_px() {
    let actions = actions_for(&mouse(Phase::Down, Button::Left, 0)).expect("mouse");
    assert_eq!(
        actions,
        vec![json!({
            "type": "pointer", "id": "mouse", "parameters": { "pointerType": "mouse" },
            "actions": [
                { "type": "pointerMove", "x": 10, "y": 21, "origin": "viewport" },
                { "type": "pointerDown", "button": 0 },
            ],
        })]
    );
    let up = actions_for(&mouse(Phase::Up, Button::Right, 0)).expect("mouse");
    assert_eq!(
        up[0]["actions"][1],
        json!({ "type": "pointerUp", "button": 2 })
    );
    let moved = actions_for(&mouse(Phase::Move, Button::None, 0)).expect("move");
    assert_eq!(moved[0]["actions"].as_array().map(Vec::len), Some(1));
}

#[test]
fn modifiers_are_held_around_the_press_and_released_after() {
    let down = actions_for(&mouse(Phase::Down, Button::Left, MOD_SHIFT | MOD_CTRL)).expect("down");
    assert_eq!(down.len(), 2, "keys first, then the pointer");
    assert_eq!(
        down[0]["actions"],
        json!([
            { "type": "keyDown", "value": "\u{E009}" },
            { "type": "keyDown", "value": "\u{E008}" },
        ])
    );
    let up = actions_for(&mouse(Phase::Up, Button::Left, MOD_SHIFT)).expect("up");
    assert_eq!(
        up[0]["actions"],
        json!([
            { "type": "pause" },
            { "type": "pause" },
            { "type": "keyUp", "value": "\u{E008}" },
        ])
    );
}

#[test]
fn a_wheel_scrolls_at_its_point() {
    let wheel = InputEvent::Wheel {
        x: 5.0,
        y: 6.0,
        delta_x: 0.0,
        delta_y: 600.0,
    };
    assert_eq!(
        actions_for(&wheel).expect("wheel"),
        vec![json!({
            "type": "wheel", "id": "wheel",
            "actions": [{ "type": "scroll", "x": 5, "y": 6, "deltaX": 0, "deltaY": 600, "origin": "viewport" }],
        })]
    );
}

#[test]
fn keys_use_their_text_or_the_webdriver_code() {
    let key = |key: &str, code: &str, text: Option<&str>| InputEvent::Key {
        phase: Phase::Down,
        key: key.to_owned(),
        code: code.to_owned(),
        text: text.map(str::to_owned),
        modifiers: 0,
    };
    let value =
        |event: &InputEvent| actions_for(event).expect("key")[0]["actions"][0]["value"].clone();
    assert_eq!(value(&key("a", "KeyA", Some("a"))), "a");
    assert_eq!(value(&key("\u{e4}", "Quote", Some("\u{e4}"))), "\u{e4}");
    assert_eq!(value(&key("Enter", "Enter", Some("\r"))), "\u{E007}");
    assert_eq!(value(&key("Tab", "Tab", None)), "\u{E004}");
    assert_eq!(value(&key("ArrowLeft", "ArrowLeft", None)), "\u{E012}");
    assert_eq!(value(&key("Shift", "ShiftRight", None)), "\u{E050}");
    assert_eq!(value(&key("F5", "F5", None)), "\u{E035}");
    assert!(actions_for(&key("Dead", "BracketLeft", None)).is_err());
}

#[test]
fn text_types_each_character() {
    let text = InputEvent::Text {
        text: "h\u{e9}".to_owned(),
    };
    assert_eq!(
        actions_for(&text).expect("text")[0]["actions"],
        json!([
            { "type": "keyDown", "value": "h" },
            { "type": "keyUp", "value": "h" },
            { "type": "keyDown", "value": "\u{e9}" },
            { "type": "keyUp", "value": "\u{e9}" },
        ])
    );
}

#[test]
fn the_command_names_the_context() {
    let command = perform_actions(
        "c1",
        vec![json!({ "type": "none", "id": "n", "actions": [] })],
    );
    assert_eq!(command.method, "input.performActions");
    assert_eq!(command.params["context"], "c1");
    assert_eq!(release_actions("c1").params, json!({ "context": "c1" }));
}

fn typed(source: &serde_json::Value) -> Vec<serde_json::Value> {
    source["actions"].as_array().cloned().unwrap_or_default()
}

#[test]
fn held_keys_follow_the_modifiers_of_each_key() {
    let mut held = HeldKeys::default();
    // Shift goes down, `A` comes with it: nothing to add.
    let shift = held.key("Shift", "ShiftRight", true, 8).expect("shift");
    assert_eq!(
        typed(&shift),
        vec![json!({ "type": "keyDown", "value": "\u{E050}" })]
    );
    let a = held.key("A", "KeyA", true, 8).expect("a");
    assert_eq!(typed(&a), vec![json!({ "type": "keyDown", "value": "A" })]);
    // Its keyup never came: the next key without Shift lets go of it first.
    let b = held.key("b", "KeyB", true, 0).expect("b");
    assert_eq!(
        typed(&b),
        vec![
            json!({ "type": "keyUp", "value": "\u{E050}" }),
            json!({ "type": "keyDown", "value": "b" }),
        ]
    );
    assert_eq!(held, HeldKeys::default());
}

#[test]
fn a_modifiers_keyup_releases_both_sides_and_a_click_its_own() {
    let mut held = HeldKeys::default();
    held.key("x", "KeyX", true, 2).expect("ctrl x");
    let up = held.key("Control", "ControlRight", false, 0).expect("up");
    assert_eq!(
        typed(&up),
        vec![
            json!({ "type": "keyUp", "value": "\u{E009}" }),
            json!({ "type": "keyUp", "value": "\u{E051}" }),
        ]
    );
    assert_eq!(held, HeldKeys::default());
    held.key("Alt", "AltLeft", true, 1).expect("alt");
    held.clicked(1);
    assert_eq!(held, HeldKeys::default(), "the click released Alt");
    assert!(held.key("Dead", "Quote", true, 0).is_none(), "a dead key");
}
