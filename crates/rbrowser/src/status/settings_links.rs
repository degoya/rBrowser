//! Every settings mention is a link (RB-094-21): a notice whose text names a settings path
//! ("Settings > Engines > Android") links to that setting in the status bar, its history, the
//! error toast and MCP's `status_messages` - also when the code that raised it named none, and
//! when the text arrives as a failure's reason (`commands.notice.failed`, `{error}`).
//!
//! The table is `crates/rbrowser/data/settings-links.json`: a text's i18n key and the place it
//! opens (a key, a category, a section or a key prefix, [`crate::settings_window::target`]).
//! The settings lint (`scripts/lib/settings_links.py`) holds it to every text of the
//! catalogues, the plugins' included, and checks that each place opens.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use rb_ipc::Notification;
use serde::Deserialize;

/// The table, compiled in.
const TABLE_JSON: &str = include_str!("../../data/settings-links.json");

/// The fewest characters of a text's own words an argument must hold to count as that text,
/// so a short template ("{a}: {b}") never matches by accident.
const MIN_LITERAL_CHARS: usize = 16;

#[derive(Deserialize)]
struct Table {
    links: BTreeMap<String, String>,
}

/// The table's links, text key to place; empty when it does not read (a test holds it read).
fn links() -> &'static BTreeMap<String, String> {
    static LINKS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    LINKS.get_or_init(|| {
        serde_json::from_str::<Table>(TABLE_JSON)
            .map(|table| table.links)
            .unwrap_or_default()
    })
}

/// Gives `notification` the setting its text names, when it carries none.
pub fn link_settings(notification: &mut Notification) {
    if notification.settings_key.is_none() {
        notification.settings_key =
            target_of(&notification.message_key, notification.args.values()).map(str::to_owned);
    }
}

/// The place a notice of `message_key` with the argument values `args` names: its own text's,
/// else the place of the first text of the table an argument holds - as its code or as its
/// sentence; `None` for none.
pub fn target_of<'a>(
    message_key: &str,
    args: impl IntoIterator<Item = &'a String>,
) -> Option<&'static str> {
    let links = links();
    if let Some(target) = links.get(message_key) {
        return Some(target.as_str());
    }
    args.into_iter().find_map(|value| {
        links
            .iter()
            .find(|(key, _)| holds(value, key))
            .map(|(_, target)| target.as_str())
    })
}

/// Whether `value` holds the text of `key`: the key itself as a code, or its sentence.
fn holds(value: &str, key: &str) -> bool {
    holds_code(value, key) || holds_text(value, &rb_i18n::t(key))
}

/// Whether `value` holds every literal piece of `text` in order, its placeholders filled with
/// anything; never for a text of fewer than [`MIN_LITERAL_CHARS`] own characters.
fn holds_text(value: &str, text: &str) -> bool {
    let pieces = literal_pieces(text);
    let literal: usize = pieces.iter().map(|piece| piece.chars().count()).sum();
    if literal < MIN_LITERAL_CHARS {
        return false;
    }
    let mut rest = value;
    for piece in pieces {
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    true
}

/// Whether `value` names the code `key` as a word of its own (`firstrun.error.off`, not
/// `firstrun.error.offline`, nor `firstrun.error.off.more`; a full stop after it ends a sentence).
fn holds_code(value: &str, key: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    value.match_indices(key).any(|(at, _)| {
        let before = value[..at].chars().next_back();
        let mut after = value[at + key.len()..].chars();
        let continues = match after.next() {
            Some('.') => after.next().is_some_and(word),
            Some(c) => word(c),
            None => false,
        };
        !before.is_some_and(|c| word(c) || c == '.') && !continues
    })
}

/// The pieces of `text` outside its placeholders (`{path}`, `{count, plural, one {#} ...}`).
fn literal_pieces(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, c) in text.char_indices() {
        match c {
            '{' => {
                if depth == 0 && at > start {
                    pieces.push(&text[start..at]);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    start = at + 1;
                }
            }
            _ => {}
        }
    }
    if depth == 0 && start < text.len() {
        pieces.push(&text[start..]);
    }
    pieces
}

#[cfg(test)]
#[path = "settings_links_tests.rs"]
mod tests;
