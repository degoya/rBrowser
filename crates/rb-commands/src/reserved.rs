//! The reservation list: key combinations rBrowser never binds because the browser, DevTools,
//! the OS, page `accesskey`s or AltGr own them (`docs/shortcuts.md`, "Reserved combinations").
//!
//! The file is `crates/rb-commands/data/reserved-shortcuts.json`; its format is the
//! [`ReservedList`] type (camelCase JSON), one entry per line, each with the owner (`source`)
//! and the reason (`reason`, `reasonKey`). [`ReservedList::find`] returns the first entry that
//! matches, so specific entries (`Ctrl+Alt+Q`, "AltGr character @") stand before the patterns
//! that cover them (`Ctrl+Alt+<any>`). Modifiers match exactly: `Win+<any>` does not cover
//! `Ctrl+Win+X`, which has an entry of its own.

use std::sync::LazyLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CommandError, Key, Modifiers, Shortcut};

/// The repository path of the reservation list.
pub const RESERVED_SHORTCUTS_PATH: &str = "crates/rb-commands/data/reserved-shortcuts.json";

const RESERVED_SHORTCUTS_JSON: &str = include_str!("../data/reserved-shortcuts.json");

/// The whole reservation list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservedList {
    /// The format version, 1.
    pub version: u32,
    /// The reserved combinations.
    pub entries: Vec<ReservedEntry>,
}

/// One reserved combination or pattern.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservedEntry {
    /// The combination in the text form of [`crate::keys`], where the key may be a placeholder:
    /// `<letter>` (A-Z), `<digit>` (0-9), `<arrow>`, `<char>` (every printable key: letters,
    /// digits, punctuation), `<any>` (every key). Examples: `Ctrl+T`, `Alt+<letter>`,
    /// `Ctrl+Alt+<any>`, `Win+<any>`.
    pub combo: String,
    /// The platforms where it is reserved.
    pub platforms: Vec<Platform>,
    /// Who owns it.
    pub source: ReservedSource,
    /// The i18n key of the reason the editor shows (`shortcuts.reserved.*`).
    pub reason_key: String,
    /// The reason in English, for the lint's output and the generated reference.
    pub reason: String,
    /// The one place where rBrowser takes the combination after all (RB-080-94): with the
    /// keyboard focus in `context` it runs `command`; everywhere else - a page, DevTools, the
    /// shell - it stays the owner's. The combination stays refused as a default and in the
    /// shortcut editor; the lint holds the entry to an exact combination and a known command
    /// ([`crate::lint::lint_contexts`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub except: Option<ContextException>,
}

/// Where a reserved combination is rBrowser's after all, and what it runs there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextException {
    /// Where the keyboard focus is.
    pub context: KeyContext,
    /// The command it runs there.
    pub command: String,
}

/// A place of the keyboard focus a reserved combination can be taken in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KeyContext {
    /// The canvas board behind the canvas's views (not a view: a page keeps its keys).
    CanvasBoard,
}

/// A desktop platform. Phase 1 is Windows only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// Windows.
    Windows,
    /// Linux (phase 2).
    Linux,
    /// macOS (phase 2).
    Macos,
}

impl Platform {
    /// The platform whose reservations hold for this build: macOS on macOS (the test build,
    /// RB-090-15), Windows everywhere else - Linux has no list of its own yet, so its tests
    /// and builds hold the Windows one.
    pub const fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::Macos
        } else {
            Platform::Windows
        }
    }
}

/// Who owns a reserved combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReservedSource {
    /// Chrome/Chromium and Firefox.
    Browser,
    /// DevTools.
    Devtools,
    /// OS text editing (`Ctrl+A/C/V`, word selection).
    TextEditing,
    /// The operating system (`Win+*`, `Alt+Tab`, `Alt+F4`).
    Os,
    /// HTML `accesskey` and browser menus (`Alt+<letter>`).
    Accesskey,
    /// AltGr on de/fr/es layouts (`Ctrl+Alt+<key>`).
    Altgr,
}

/// The key half of a reserved combination: one key or a placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyPattern {
    /// Exactly this key.
    Exact(Key),
    /// `<letter>`: A-Z.
    Letter,
    /// `<digit>`: 0-9.
    Digit,
    /// `<arrow>`: the four arrow keys.
    Arrow,
    /// `<char>`: every printable key.
    Char,
    /// `<any>`: every key.
    Any,
}

impl KeyPattern {
    /// Whether `key` falls under the pattern.
    pub(crate) fn matches(self, key: Key) -> bool {
        match self {
            KeyPattern::Exact(exact) => exact == key,
            KeyPattern::Letter => matches!(key, Key::Char(c) if c.is_ascii_uppercase()),
            KeyPattern::Digit => matches!(key, Key::Char(c) if c.is_ascii_digit()),
            KeyPattern::Arrow => matches!(key, Key::Left | Key::Right | Key::Up | Key::Down),
            KeyPattern::Char => matches!(key, Key::Char(_)),
            KeyPattern::Any => true,
        }
    }
}

/// A parsed `combo`: exact modifiers plus a key pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComboPattern {
    /// The modifiers, matched exactly.
    pub(crate) modifiers: Modifiers,
    /// The key or placeholder.
    pub(crate) key: KeyPattern,
}

impl ComboPattern {
    /// Parses a `combo`; `None` when it is not the text form with an optional placeholder.
    pub(crate) fn parse(combo: &str) -> Option<ComboPattern> {
        let placeholder = combo
            .rfind('<')
            .filter(|&at| combo.ends_with('>') && combo.len() - at > 2)
            .map(|at| (&combo[..at], &combo[at..]));
        let Some((prefix, name)) = placeholder else {
            let shortcut: Shortcut = combo.parse().ok()?;
            return Some(ComboPattern {
                modifiers: shortcut.modifiers,
                key: KeyPattern::Exact(shortcut.key),
            });
        };
        let key = match name {
            "<letter>" => KeyPattern::Letter,
            "<digit>" => KeyPattern::Digit,
            "<arrow>" => KeyPattern::Arrow,
            "<char>" => KeyPattern::Char,
            "<any>" => KeyPattern::Any,
            _ => return None,
        };
        // The modifiers parse with a stand-in key in place of the placeholder.
        let modifiers = format!("{prefix}A").parse::<Shortcut>().ok()?.modifiers;
        Some(ComboPattern { modifiers, key })
    }

    /// Whether `shortcut` falls under the pattern.
    pub(crate) fn matches(&self, shortcut: &Shortcut) -> bool {
        self.modifiers == shortcut.modifiers && self.key.matches(shortcut.key)
    }
}

impl ReservedList {
    /// The first entry that reserves `shortcut` on `platform`, if any (placeholders expanded).
    /// An entry whose `combo` does not parse matches nothing; the tests keep the shipped list
    /// free of those.
    pub fn find(&self, shortcut: &Shortcut, platform: Platform) -> Option<&ReservedEntry> {
        self.entries.iter().find(|entry| {
            entry.platforms.contains(&platform)
                && ComboPattern::parse(&entry.combo)
                    .is_some_and(|pattern| pattern.matches(shortcut))
        })
    }
}

impl ReservedList {
    /// The command `shortcut` runs with the focus in `context` on `platform`: the
    /// [`ContextException`] of the entry that reserves it, when that names `context`.
    pub fn context_command(
        &self,
        shortcut: &Shortcut,
        platform: Platform,
        context: KeyContext,
    ) -> Option<&str> {
        self.find(shortcut, platform)
            .and_then(|entry| entry.except.as_ref())
            .filter(|except| except.context == context)
            .map(|except| except.command.as_str())
    }
}

/// The compiled-in list, parsed once. The tests prove it parses; were it ever broken, the
/// list is empty and the lint and tests say so rather than every key press panicking.
pub fn builtin_reserved() -> &'static ReservedList {
    static LIST: LazyLock<ReservedList> = LazyLock::new(|| {
        reserved_list().unwrap_or(ReservedList {
            version: 1,
            entries: Vec::new(),
        })
    });
    &LIST
}

/// The reservation list compiled into the binary.
pub fn reserved_list() -> Result<ReservedList, CommandError> {
    serde_json::from_str(RESERVED_SHORTCUTS_JSON)
        .map_err(|error| CommandError::ReservedList(error.to_string()))
}

#[cfg(test)]
#[path = "reserved_tests.rs"]
mod tests;
