//! Which keys the recorder looks at (RB-050-01): of a password field only the named keys, in
//! clear; as `press` steps only named keys and shortcuts, never typed text. Split from
//! [`super::cleanup`], which applies them.

use crate::event::{KeyData, MOD_ALT, MOD_CTRL, MOD_META, SyncEvent};

/// Keys that never are a step of their own.
const SILENT_KEYS: &[&str] = &[
    "Shift",
    "Control",
    "Alt",
    "AltGraph",
    "Meta",
    "CapsLock",
    "Dead",
    "Process",
    "Unidentified",
];

/// What the recorder looks at of the captured `event`. A key in a password field is decided on
/// before anything is masked: a named key (Enter, Tab, Escape, an arrow) goes on in clear, any
/// other (a character, Backspace, Delete) and its key-up are dropped - so neither a mask nor the
/// password's length reaches a step. Every other event as [`crate::redact`] shows it.
pub(crate) fn shown(event: &SyncEvent) -> Option<SyncEvent> {
    match event {
        SyncEvent::Keydown(key) if key.sensitive => is_named_key(&key.key).then(|| {
            SyncEvent::Keydown(KeyData {
                sensitive: false,
                ..key.clone()
            })
        }),
        SyncEvent::Keyup(key) if key.sensitive => None,
        other => Some(crate::redact(other)),
    }
}

/// Whether `key` is a key's name (`Enter`, `ArrowLeft`, `F5`), not text: ASCII letters and
/// digits, an upper-case letter first, more than one character; Backspace and Delete edit a
/// field's text and count as text here.
fn is_named_key(key: &str) -> bool {
    key.len() > 1
        && key.starts_with(|c: char| c.is_ascii_uppercase())
        && key.chars().all(|c| c.is_ascii_alphanumeric())
        && !matches!(key, "Backspace" | "Delete")
}

/// Whether `key` going down is a `press` step: a named key (Enter, Tab, Escape, an arrow) or a
/// shortcut (Ctrl or Meta with a character) - never text input (a character alone, with Shift,
/// Alt or AltGr, which reports as Ctrl+Alt), a modifier on its own, a repeat, or Backspace or
/// Delete in a field. So no character typed into a field - a password's included - is a step.
pub(crate) fn is_press(key: &KeyData) -> bool {
    // A masked key never is a step ([`shown`] lets a password field's named keys through).
    if key.sensitive || key.repeat || SILENT_KEYS.contains(&key.key.as_str()) {
        return false;
    }
    let ctrl_or_meta = key.modifiers & (MOD_CTRL | MOD_META) != 0;
    let altgr = key.modifiers & MOD_CTRL != 0 && key.modifiers & MOD_ALT != 0;
    if key.key.chars().count() == 1 {
        return ctrl_or_meta && !altgr;
    }
    !(key.target.is_some() && matches!(key.key.as_str(), "Backspace" | "Delete") && !ctrl_or_meta)
}
