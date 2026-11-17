//! From a CEF key event to a [`Shortcut`]: the modifier flags (`cef_event_flags_t`) and the
//! Windows virtual-key code, plus the character the layout types. Portable, so the host's
//! `OnPreKeyEvent` (crates/rbrowser) only copies three fields over.
//!
//! Letters, digits and named keys come from the virtual-key code and do not depend on the
//! layout. Punctuation keys (`VK_OEM_*`) differ per layout, so the character the key types
//! wins when the host knows it (`unmodified_character`); without it the US layout is assumed.
//! While a leader is pending ([`crate::ChordMachine::is_pending`]) the host maps the typed
//! character instead ([`key_from_character`]), so `Leader ]` works where `]` needs AltGr.

use crate::{Key, Modifiers, Shortcut};

/// `EVENTFLAG_SHIFT_DOWN`.
pub const EVENTFLAG_SHIFT_DOWN: u32 = 1 << 1;
/// `EVENTFLAG_CONTROL_DOWN`.
pub const EVENTFLAG_CONTROL_DOWN: u32 = 1 << 2;
/// `EVENTFLAG_ALT_DOWN`.
pub const EVENTFLAG_ALT_DOWN: u32 = 1 << 3;
/// `EVENTFLAG_COMMAND_DOWN`: the Windows key.
pub const EVENTFLAG_COMMAND_DOWN: u32 = 1 << 7;
/// `EVENTFLAG_ALTGR_DOWN`: AltGr, which Windows reports as Ctrl+Alt as well.
pub const EVENTFLAG_ALTGR_DOWN: u32 = 1 << 12;

/// The modifiers of CEF event flags. AltGr counts as Ctrl+Alt, as Windows reports it, so it
/// meets the reserved `Ctrl+Alt+<any>` entries and is never bound.
pub fn modifiers_from_event_flags(flags: u32) -> Modifiers {
    let altgr = flags & EVENTFLAG_ALTGR_DOWN != 0;
    Modifiers {
        ctrl: altgr || flags & EVENTFLAG_CONTROL_DOWN != 0,
        shift: flags & EVENTFLAG_SHIFT_DOWN != 0,
        alt: altgr || flags & EVENTFLAG_ALT_DOWN != 0,
        meta: flags & EVENTFLAG_COMMAND_DOWN != 0,
    }
}

/// The key of a Windows virtual-key code; `character` is the character the key types on the
/// current layout without modifiers, when known. `None` for modifier keys and keys rBrowser
/// never binds (Print Screen, media keys).
pub fn key_from_windows_key_code(code: u32, character: Option<char>) -> Option<Key> {
    let key = match code {
        0x08 => Key::Backspace,
        0x09 => Key::Tab,
        0x0D => Key::Enter,
        0x1B => Key::Escape,
        0x20 => Key::Space,
        0x21 => Key::PageUp,
        0x22 => Key::PageDown,
        0x23 => Key::End,
        0x24 => Key::Home,
        0x25 => Key::Left,
        0x26 => Key::Up,
        0x27 => Key::Right,
        0x28 => Key::Down,
        0x2D => Key::Insert,
        0x2E => Key::Delete,
        0x30..=0x39 | 0x41..=0x5A => Key::Char(char::from_u32(code)?),
        // The number pad's digits are the digits.
        0x60..=0x69 => Key::Char(char::from_u32(code - 0x60 + u32::from(b'0'))?),
        0x6B => Key::Char('+'),
        0x6D => Key::Char('-'),
        0x70..=0x87 => Key::F(u8::try_from(code - 0x70 + 1).ok()?),
        0xBA..=0xC0 | 0xDB..=0xDF | 0xE2 => {
            return character
                .and_then(key_from_character)
                .or_else(|| us_oem_character(code).map(Key::Char));
        }
        _ => return None,
    };
    Some(key)
}

/// What the `VK_OEM_*` keys type on the US layout.
fn us_oem_character(code: u32) -> Option<char> {
    Some(match code {
        0xBA => ';',
        0xBB => '+',
        0xBC => ',',
        0xBD => '-',
        0xBE => '.',
        0xBF => '/',
        0xC0 => '`',
        0xDB => '[',
        0xDC => '\\',
        0xDD => ']',
        0xDE => '\'',
        _ => return None,
    })
}

/// The Windows virtual-key code of `key`, the punctuation as on the US layout - what a native
/// menu takes to show a direct shortcut beside its row (RB-090-63). `None` for a key without
/// one (a letter outside ASCII).
pub fn windows_key_code(key: Key) -> Option<u32> {
    let code = match key {
        Key::Backspace => 0x08,
        Key::Tab => 0x09,
        Key::Enter => 0x0D,
        Key::Escape => 0x1B,
        Key::Space => 0x20,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::End => 0x23,
        Key::Home => 0x24,
        Key::Left => 0x25,
        Key::Up => 0x26,
        Key::Right => 0x27,
        Key::Down => 0x28,
        Key::Insert => 0x2D,
        Key::Delete => 0x2E,
        Key::F(n) if (1..=24).contains(&n) => 0x70 + u32::from(n) - 1,
        Key::F(_) => return None,
        Key::Char(c) if c.is_ascii_digit() || c.is_ascii_uppercase() => u32::from(c),
        Key::Char('+') => 0xBB,
        Key::Char(c) => return (0xBA..=0xDE).find(|code| us_oem_character(*code) == Some(c)),
    };
    Some(code)
}

/// The key a typed character stands for (`]` from AltGr+9 on a German layout). After the
/// leader the host passes this instead of the virtual key, so chords follow what the layout
/// types; `None` for control characters and whitespace.
pub fn key_from_character(character: char) -> Option<Key> {
    (!character.is_control() && !character.is_whitespace())
        .then(|| Key::Char(character.to_ascii_uppercase()))
}

/// The whole mapping: `None` when the key is not one rBrowser binds.
pub fn shortcut_from_cef(
    flags: u32,
    windows_key_code: u32,
    character: Option<char>,
) -> Option<Shortcut> {
    Some(Shortcut {
        modifiers: modifiers_from_event_flags(flags),
        key: key_from_windows_key_code(windows_key_code, character)?,
    })
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
