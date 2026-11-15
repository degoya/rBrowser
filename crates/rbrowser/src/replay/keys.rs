//! Keys to `Input.dispatchKeyEvent` (RB-020-03).
//!
//! A replayed key goes down as `rawKeyDown` - the page sees `keydown`, the editing commands of
//! the key run (Backspace, arrows, Tab, Ctrl+A) - and up as `keyUp`. Printable keys carry no
//! text: the characters come from the `input` event through `Input.insertText`, so they arrive
//! the same whatever the keyboard layout - AltGr characters (`@`, `\u{20ac}` on a German
//! keyboard) included. Only Enter goes down as `keyDown` with its `"\r"`, the key press that
//! runs the default action (implicit submission, a link's or button's activation).

use rb_sync::KeyData;
use serde_json::{Map, Value, json};

/// `KeyboardEvent.location` of the numeric keypad.
const LOCATION_NUMPAD: u32 = 3;

/// Down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    /// `keydown`.
    Down,
    /// `keyup`.
    Up,
}

/// The fields of a key that `Input.dispatchKeyEvent` takes (from a captured [`KeyData`] or a
/// named key of MCP's `press_key`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct KeyInput<'a> {
    /// `KeyboardEvent.key`.
    pub key: &'a str,
    /// `KeyboardEvent.code`.
    pub code: &'a str,
    /// `KeyboardEvent.keyCode`, the Windows virtual key.
    pub key_code: u32,
    /// `KeyboardEvent.location`.
    pub location: u32,
    /// Auto-repeat.
    pub repeat: bool,
    /// CDP modifier bits.
    pub modifiers: u8,
}

impl<'a> From<&'a KeyData> for KeyInput<'a> {
    fn from(data: &'a KeyData) -> Self {
        Self {
            key: &data.key,
            code: &data.code,
            key_code: data.key_code,
            location: data.location,
            repeat: data.repeat,
            modifiers: data.modifiers,
        }
    }
}

/// The key in a log line: its code, never the character (it may be part of a password).
impl std::fmt::Debug for KeyInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyInput")
            .field("code", &self.code)
            .field("modifiers", &self.modifiers)
            .finish_non_exhaustive()
    }
}

/// Whether `key` is a character (one Unicode scalar, not a control character), not a named key.
pub fn is_printable(key: &str) -> bool {
    let mut chars = key.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if !c.is_control())
}

/// The text a replayed key types through `Input.dispatchKeyEvent`: Enter's `"\r"`, nothing for
/// every other key (printable keys type through `Input.insertText`).
pub fn replay_text(key: &str) -> Option<&'static str> {
    (key == "Enter").then_some("\r")
}

/// The parameters of `Input.dispatchKeyEvent` for `key` in `phase`; `text` only on the way down.
pub fn key_event(key: &KeyInput<'_>, phase: KeyPhase, text: Option<&str>) -> Value {
    let text = text.filter(|_| phase == KeyPhase::Down);
    let kind = match (phase, text) {
        (KeyPhase::Up, _) => "keyUp",
        (KeyPhase::Down, Some(_)) => "keyDown",
        (KeyPhase::Down, None) => "rawKeyDown",
    };
    let mut params = Map::new();
    params.insert("type".into(), json!(kind));
    params.insert("key".into(), json!(key.key));
    params.insert("code".into(), json!(key.code));
    params.insert("windowsVirtualKeyCode".into(), json!(key.key_code));
    params.insert("nativeVirtualKeyCode".into(), json!(key.key_code));
    params.insert("modifiers".into(), json!(key.modifiers));
    params.insert("autoRepeat".into(), json!(key.repeat));
    params.insert("isKeypad".into(), json!(key.location == LOCATION_NUMPAD));
    if key.location != 0 {
        params.insert("location".into(), json!(key.location));
    }
    if let Some(text) = text {
        params.insert("text".into(), json!(text));
        params.insert("unmodifiedText".into(), json!(text));
    }
    Value::Object(params)
}

/// A key MCP's `press_key` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedKey {
    /// `KeyboardEvent.key`.
    pub key: String,
    /// `KeyboardEvent.code` (US layout).
    pub code: String,
    /// The Windows virtual key.
    pub key_code: u32,
}

/// The named keys: `key`, `code`, virtual key.
const NAMED: &[(&str, &str, u32)] = &[
    ("Backspace", "Backspace", 8),
    ("Tab", "Tab", 9),
    ("Enter", "Enter", 13),
    ("Shift", "ShiftLeft", 16),
    ("Control", "ControlLeft", 17),
    ("Alt", "AltLeft", 18),
    ("Pause", "Pause", 19),
    ("CapsLock", "CapsLock", 20),
    ("Escape", "Escape", 27),
    (" ", "Space", 32),
    ("PageUp", "PageUp", 33),
    ("PageDown", "PageDown", 34),
    ("End", "End", 35),
    ("Home", "Home", 36),
    ("ArrowLeft", "ArrowLeft", 37),
    ("ArrowUp", "ArrowUp", 38),
    ("ArrowRight", "ArrowRight", 39),
    ("ArrowDown", "ArrowDown", 40),
    ("Insert", "Insert", 45),
    ("Delete", "Delete", 46),
    ("Meta", "MetaLeft", 91),
    ("ContextMenu", "ContextMenu", 93),
];

/// The key `name` (`Enter`, `ArrowDown`, `F5`, `a`, `7`, `" "`); `None` for an unknown name.
pub fn named_key(name: &str) -> Option<NamedKey> {
    let found = |key: &str, code: String, key_code: u32| NamedKey {
        key: key.to_owned(),
        code,
        key_code,
    };
    if let Some((key, code, key_code)) = NAMED.iter().find(|(key, ..)| *key == name) {
        return Some(found(key, (*code).to_owned(), *key_code));
    }
    if let Some(number) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
        && (1..=24).contains(&number)
    {
        return Some(found(name, name.to_owned(), 111 + number));
    }
    let mut chars = name.chars();
    let (Some(character), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if character.is_ascii_alphabetic() {
        let upper = character.to_ascii_uppercase();
        return Some(found(name, format!("Key{upper}"), u32::from(upper)));
    }
    if character.is_ascii_digit() {
        return Some(found(
            name,
            format!("Digit{character}"),
            u32::from(character),
        ));
    }
    is_printable(name).then(|| found(name, String::new(), 0))
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
