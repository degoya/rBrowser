use rb_sync::{KeyData, MOD_ALT, MOD_CTRL, MOD_SHIFT, SyncEvent};
use serde_json::json;

use super::*;
use crate::replay::plan::{Action, Options, plan};

/// A captured key as the agent sends it (built from JSON, so a field a later protocol version
/// adds with a default does not break the test).
fn key(key: &str, code: &str, key_code: u32, location: u32, modifiers: u8) -> KeyData {
    serde_json::from_value(json!({
        "key": key, "code": code, "keyCode": key_code, "location": location,
        "repeat": false, "modifiers": modifiers,
    }))
    .expect("a key")
}

fn down(data: &KeyData) -> serde_json::Value {
    key_event(
        &KeyInput::from(data),
        KeyPhase::Down,
        replay_text(&data.key),
    )
}

#[test]
fn enter_goes_down_with_its_carriage_return_so_the_default_action_runs() {
    let params = down(&key("Enter", "Enter", 13, 0, 0));
    assert_eq!(params["type"], "keyDown");
    assert_eq!(params["text"], "\r");
    assert_eq!(params["unmodifiedText"], "\r");
    assert_eq!(params["windowsVirtualKeyCode"], 13);
    assert_eq!(params["isKeypad"], false);
}

#[test]
fn numpad_enter_is_a_keypad_key() {
    let params = down(&key("Enter", "NumpadEnter", 13, 3, 0));
    assert_eq!(params["text"], "\r");
    assert_eq!(params["isKeypad"], true);
    assert_eq!(params["location"], 3);
}

#[test]
fn control_keys_go_down_raw_without_text() {
    for (name, code, key_code) in [
        ("Tab", "Tab", 9),
        ("Escape", "Escape", 27),
        ("Backspace", "Backspace", 8),
        ("ArrowLeft", "ArrowLeft", 37),
        ("Delete", "Delete", 46),
    ] {
        let params = down(&key(name, code, key_code, 0, 0));
        assert_eq!(params["type"], "rawKeyDown", "{name}");
        assert!(params.get("text").is_none(), "{name}");
        assert_eq!(params["key"], name);
        assert_eq!(params["code"], code);
        assert_eq!(params["windowsVirtualKeyCode"], key_code);
        assert_eq!(params["nativeVirtualKeyCode"], key_code);
    }
}

#[test]
fn modifiers_and_auto_repeat_travel() {
    let mut data = key("Tab", "Tab", 9, 0, MOD_SHIFT);
    data.repeat = true;
    let params = down(&data);
    assert_eq!(params["modifiers"], u64::from(MOD_SHIFT));
    assert_eq!(params["autoRepeat"], true);
    let select_all = down(&key("a", "KeyA", 65, 0, MOD_CTRL));
    assert_eq!(select_all["type"], "rawKeyDown");
    assert_eq!(select_all["modifiers"], u64::from(MOD_CTRL));
    assert!(select_all.get("text").is_none());
}

#[test]
fn key_up_is_key_up() {
    let data = key("Enter", "Enter", 13, 0, 0);
    let params = key_event(&KeyInput::from(&data), KeyPhase::Up, None);
    assert_eq!(params["type"], "keyUp");
    assert!(params.get("text").is_none());
}

#[test]
fn printable_keys_type_nothing_themselves() {
    // Their text comes from the `input` event (Input.insertText), layout-independent.
    for printable in ["a", "A", " ", "\u{fc}", "\u{df}", "@", "\u{20ac}", "1"] {
        assert_eq!(replay_text(printable), None, "{printable:?}");
        assert!(is_printable(printable), "{printable:?}");
    }
    for named in ["Enter", "Tab", "AltGraph", "Dead", "Unidentified", ""] {
        assert!(!is_printable(named), "{named:?}");
    }
}

#[test]
fn alt_gr_characters_go_down_raw_with_ctrl_and_alt() {
    // German layout: AltGr is reported as Ctrl+Alt on Windows; "@" is AltGr+Q, "\u{20ac}" AltGr+E.
    let alt_gr = down(&key("AltGraph", "AltRight", 18, 2, MOD_CTRL | MOD_ALT));
    assert_eq!(alt_gr["type"], "rawKeyDown");
    assert_eq!(alt_gr["location"], 2);
    let at = down(&key("@", "KeyQ", 81, 0, MOD_CTRL | MOD_ALT));
    assert_eq!(at["type"], "rawKeyDown");
    assert_eq!(at["key"], "@");
    assert_eq!(at["modifiers"], u64::from(MOD_CTRL | MOD_ALT));
    assert!(at.get("text").is_none());
}

#[test]
fn german_text_with_alt_gr_arrives_whole_through_insert_text() {
    // "Gr\u{fc}\u{df}e @ \u{20ac}" typed on a German keyboard: per character a keydown (no
    // text), the input event and a keyup. Only the input events carry text.
    let typed = "Gr\u{fc}\u{df}e @ \u{20ac}";
    let target =
        json!({ "element": { "kind": "css", "path": "#q" }, "offset": { "x": 0.5, "y": 0.5 } });
    let mut text = String::new();
    for character in typed.chars() {
        let alt_gr = matches!(character, '@' | '\u{20ac}');
        let modifiers = if alt_gr { MOD_CTRL | MOD_ALT } else { 0 };
        let character = character.to_string();
        let keydown: SyncEvent = serde_json::from_value(json!({
            "type": "keydown", "key": character, "code": "KeyX", "keyCode": 88,
            "location": 0, "repeat": false, "modifiers": modifiers,
        }))
        .expect("a keydown");
        match plan(&keydown, Options::default()) {
            Action::Key(params) => assert!(params.get("text").is_none()),
            other => panic!("a key, not {other:?}"),
        }
        let input: SyncEvent = serde_json::from_value(json!({
            "type": "input", "inputType": "insertText", "data": character, "target": target,
        }))
        .expect("an input");
        match plan(&input, Options::default()) {
            Action::Text { text: inserted, .. } => text.push_str(&inserted),
            other => panic!("text, not {other:?}"),
        }
    }
    assert_eq!(text, typed);
}

#[test]
fn named_keys_have_their_code_and_virtual_key() {
    let enter = named_key("Enter").expect("Enter");
    assert_eq!((enter.code.as_str(), enter.key_code), ("Enter", 13));
    assert_eq!(named_key("Tab").map(|k| k.key_code), Some(9));
    assert_eq!(named_key("Escape").map(|k| k.key_code), Some(27));
    assert_eq!(
        named_key("ArrowDown").map(|k| k.code),
        Some("ArrowDown".to_owned())
    );
    assert_eq!(named_key("F5").map(|k| k.key_code), Some(116));
    let space = named_key(" ").expect("space");
    assert_eq!((space.code.as_str(), space.key_code), ("Space", 32));
    let letter = named_key("k").expect("a letter");
    assert_eq!((letter.code.as_str(), letter.key_code), ("KeyK", 75));
    let digit = named_key("7").expect("a digit");
    assert_eq!((digit.code.as_str(), digit.key_code), ("Digit7", 55));
    assert!(named_key("NoSuchKey").is_none());
}
