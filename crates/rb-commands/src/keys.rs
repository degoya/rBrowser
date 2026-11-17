//! Keys, direct shortcuts, leader chords and bindings, with their text form.
//!
//! **Text form** (stored in the user settings, the config's breakpoint `shortcut`, plugin
//! manifests, the reserved list, and shown in the UI): modifiers in the order
//! `Ctrl+Shift+Alt+Win`, then the key, joined by `+` - `Ctrl+Shift+1`, `F9`. A leader chord is
//! the leader, a space, then the keys after it separated by spaces - `Ctrl+Shift+Space L 1`.
//! Letters are upper case. Key names: `F1`...`F24`, `Space`, `Enter`, `Escape`, `Tab`,
//! `Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`, `PageDown`, `Left`, `Right`, `Up`,
//! `Down`, `Plus` (for `+`); every other printable key is its character (`,` `.` `[` `]` `-`).
//! [`std::fmt::Display`] writes this form and [`std::str::FromStr`] reads it back. Parsing is
//! lenient: modifiers in any order and case (`Control`, `Meta` as aliases), key aliases (`Esc`,
//! `Return`, `Del`, `PgUp`, `ArrowLeft`), a trailing `+` for the plus key, and `Leader` for
//! the leader (plugin manifests write `Leader K S`).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CommandError;

/// One key, independent of modifiers. Letters are stored upper case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    /// A printable key: `A`-`Z`, `0`-`9`, punctuation.
    Char(char),
    /// A function key `F1`-`F24`.
    F(u8),
    /// Space bar.
    Space,
    /// Enter.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char('+') => f.write_str("Plus"),
            Key::Char(c) => write!(f, "{}", c.to_ascii_uppercase()),
            Key::F(n) => write!(f, "F{n}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// The modifier keys of a direct shortcut. `meta` is the Windows key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Modifiers {
    /// Ctrl.
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt (`Ctrl+Alt` is AltGr on de/fr/es layouts and reserved).
    pub alt: bool,
    /// The Windows key (reserved for the OS).
    pub meta: bool,
}

impl Modifiers {
    /// No modifier.
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };
    /// Ctrl.
    pub const CTRL: Modifiers = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    /// Ctrl+Shift.
    pub const CTRL_SHIFT: Modifiers = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
}

/// A direct key combination: modifiers plus one key, e.g. `Ctrl+Shift+1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Shortcut {
    /// The modifiers held.
    pub modifiers: Modifiers,
    /// The key pressed.
    pub key: Key,
}

/// The leader chord `Ctrl+Shift+Space` (the docs write it as the command sign, U+2318). Pressed
/// twice it opens the command palette.
pub const LEADER: Shortcut = Shortcut {
    modifiers: Modifiers::CTRL_SHIFT,
    key: Key::Space,
};

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modifiers;
        for (held, name) in [
            (m.ctrl, "Ctrl+"),
            (m.shift, "Shift+"),
            (m.alt, "Alt+"),
            (m.meta, "Win+"),
        ] {
            if held {
                f.write_str(name)?;
            }
        }
        write!(f, "{}", self.key)
    }
}

impl Key {
    /// Parses one key name or character (`F9`, `Esc`, `Plus`, `,`, `a`). Letters are stored
    /// upper case.
    pub fn parse_name(name: &str) -> Option<Key> {
        let mut chars = name.chars();
        if let (Some(c), None) = (chars.next(), chars.clone().next()) {
            return (!c.is_whitespace() && !c.is_control())
                .then(|| Key::Char(c.to_ascii_uppercase()));
        }
        let lower = name.to_ascii_lowercase();
        let named = match lower.as_str() {
            "space" => Key::Space,
            "enter" | "return" => Key::Enter,
            "escape" | "esc" => Key::Escape,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "insert" | "ins" => Key::Insert,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "pgup" => Key::PageUp,
            "pagedown" | "pgdn" => Key::PageDown,
            "left" | "arrowleft" => Key::Left,
            "right" | "arrowright" => Key::Right,
            "up" | "arrowup" => Key::Up,
            "down" | "arrowdown" => Key::Down,
            "plus" => Key::Char('+'),
            _ => {
                let number: u8 = lower.strip_prefix('f')?.parse().ok()?;
                return (1..=24).contains(&number).then_some(Key::F(number));
            }
        };
        Some(named)
    }
}

/// Splits `Ctrl+Shift+K` into its parts; a trailing `+` (`Ctrl++`) is the plus key.
fn split_parts(text: &str) -> Option<Vec<&str>> {
    let (head, plus_key) = match text.strip_suffix("++") {
        Some(head) => (head, true),
        None if text == "+" => return Some(vec!["+"]),
        None => (text, false),
    };
    let mut parts: Vec<&str> = head.split('+').collect();
    if plus_key {
        parts.push("+");
    }
    parts.iter().all(|part| !part.is_empty()).then_some(parts)
}

fn parse_shortcut(text: &str) -> Option<Shortcut> {
    if text.chars().any(char::is_whitespace) {
        return None;
    }
    let parts = split_parts(text)?;
    let (key_name, modifier_names) = parts.split_last()?;
    let mut modifiers = Modifiers::NONE;
    for name in modifier_names {
        let slot = match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => &mut modifiers.ctrl,
            "shift" => &mut modifiers.shift,
            "alt" => &mut modifiers.alt,
            "win" | "meta" => &mut modifiers.meta,
            _ => return None,
        };
        if *slot {
            return None;
        }
        *slot = true;
    }
    let key = Key::parse_name(key_name)?;
    Some(Shortcut { modifiers, key })
}

impl FromStr for Shortcut {
    type Err = CommandError;

    /// Parses the text form of a direct combination.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_shortcut(text.trim()).ok_or_else(|| CommandError::InvalidBinding(text.to_owned()))
    }
}

/// Whether `token` names the leader: its combination in any spelling, `Leader`, or the command
/// sign U+2318.
fn is_leader_token(token: &str) -> bool {
    token.eq_ignore_ascii_case("leader")
        || token == "\u{2318}"
        || parse_shortcut(token) == Some(LEADER)
}

/// Parses the keys after the leader: one or two keys without modifiers.
fn parse_chord(text: &str) -> Option<Chord> {
    let mut tokens = text.split_whitespace();
    if !is_leader_token(tokens.next()?) {
        return None;
    }
    let keys = tokens
        .map(|token| match token {
            "+" => Some(Key::Char('+')),
            _ if token.contains('+') => None,
            _ => Key::parse_name(token),
        })
        .collect::<Option<Vec<Key>>>()?;
    (1..=2).contains(&keys.len()).then_some(Chord { keys })
}

/// The keys pressed after the leader, one or two (`L 1`, `S S`, `]`), without modifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    /// The keys after the leader, in order.
    pub keys: Vec<Key>,
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{LEADER}")?;
        for key in &self.keys {
            write!(f, " {key}")?;
        }
        Ok(())
    }
}

impl FromStr for Chord {
    type Err = CommandError;

    /// Parses `Ctrl+Shift+Space <key> [<key>]` (or `Leader <key> [<key>]`).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_chord(text).ok_or_else(|| CommandError::InvalidBinding(text.to_owned()))
    }
}

/// How a command is triggered from the keyboard: a direct shortcut or a leader chord.
/// Serialized as its text form.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Binding {
    /// A direct combination.
    Direct(Shortcut),
    /// The leader followed by keys.
    Leader(Chord),
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Binding::Direct(shortcut) => shortcut.fmt(f),
            Binding::Leader(chord) => chord.fmt(f),
        }
    }
}

impl FromStr for Binding {
    type Err = CommandError;

    /// Parses either text form: more than one word is a leader chord, one word a direct
    /// combination.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.split_whitespace().nth(1).is_some() {
            text.parse().map(Binding::Leader)
        } else {
            text.parse().map(Binding::Direct)
        }
    }
}

impl TryFrom<String> for Binding {
    type Error = CommandError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Binding> for String {
    fn from(binding: Binding) -> Self {
        binding.to_string()
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
