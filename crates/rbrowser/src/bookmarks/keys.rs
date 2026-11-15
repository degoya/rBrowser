//! The bookmarks' own shortcuts (`bookmarks[].shortcut`, RB-070-06), held to the rules of the
//! breakpoints' (RB-010-16): a direct combination, not reserved, held by no command and no
//! breakpoint shortcut, given to one bookmark only. What breaks a rule is a conflict the user is
//! told about with its reason; the registry and the breakpoints always win.

use rb_commands::validate::{
    REJECTED_CONFLICT, REJECTED_INVALID, bindings_conflict, check_binding,
};
use rb_commands::{Binding, CommandError, Platform, ReservedList, Shortcut};
use rb_config::ProjectConfig;

pub use crate::breakpoint_keys::REJECTED_CHORD;

/// The i18n key of the notification about one conflict; placeholders `{bookmark}`, `{binding}`,
/// `{reason}` (an i18n key) and `{other}`.
pub const CONFLICT_KEY: &str = "bookmarks.conflict.notice";

/// A bookmark shortcut in force.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkKey {
    /// The combination.
    pub shortcut: Shortcut,
    /// The bookmark it opens.
    pub bookmark: String,
}

/// A bookmark shortcut that is not in force, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The bookmark whose `shortcut` it is.
    pub bookmark: String,
    /// The text of `shortcut`.
    pub binding: String,
    /// The i18n key of the reason (`shortcuts.reserved.*`, `shortcuts.rejected.*`).
    pub reason_key: String,
    /// The command, breakpoint or bookmark that holds it, for a conflict.
    pub other: Option<String>,
}

/// The bookmark shortcuts of `config`: those in force and the conflicts. `held` are the
/// bindings in effect before them, `(holder, binding)`: the registry's commands and the
/// breakpoints' own shortcuts.
pub fn bookmark_keys(
    config: &ProjectConfig,
    held: &[(String, Binding)],
    reserved: &ReservedList,
    platform: Platform,
) -> (Vec<BookmarkKey>, Vec<Conflict>) {
    let mut candidates: Vec<BookmarkKey> = Vec::new();
    let mut conflicts = Vec::new();
    for bookmark in &config.bookmarks {
        let Some(text) = bookmark.shortcut.as_deref() else {
            continue;
        };
        let conflict = |reason_key: &str, other: Option<String>| Conflict {
            bookmark: bookmark.id.clone(),
            binding: text.to_owned(),
            reason_key: reason_key.to_owned(),
            other,
        };
        let shortcut = match text.parse::<Binding>() {
            Ok(Binding::Direct(shortcut)) => shortcut,
            Ok(Binding::Leader(_)) => {
                conflicts.push(conflict(REJECTED_CHORD, None));
                continue;
            }
            Err(_) => {
                conflicts.push(conflict(REJECTED_INVALID, None));
                continue;
            }
        };
        let binding = Binding::Direct(shortcut);
        match check_binding(&binding, reserved, platform) {
            Err(CommandError::Reserved(reason_key)) => {
                conflicts.push(conflict(&reason_key, None));
                continue;
            }
            Err(_) => {
                conflicts.push(conflict(REJECTED_INVALID, None));
                continue;
            }
            Ok(()) => {}
        }
        if let Some((holder, _)) = held
            .iter()
            .find(|(_, held)| bindings_conflict(held, &binding))
        {
            conflicts.push(conflict(REJECTED_CONFLICT, Some(holder.clone())));
            continue;
        }
        candidates.push(BookmarkKey {
            shortcut,
            bookmark: bookmark.id.clone(),
        });
    }
    let mut keys = Vec::new();
    for key in &candidates {
        match candidates
            .iter()
            .find(|other| other.shortcut == key.shortcut && other.bookmark != key.bookmark)
        {
            Some(other) => conflicts.push(Conflict {
                bookmark: key.bookmark.clone(),
                binding: key.shortcut.to_string(),
                reason_key: REJECTED_CONFLICT.to_owned(),
                other: Some(other.bookmark.clone()),
            }),
            None => keys.push(key.clone()),
        }
    }
    (keys, conflicts)
}

/// The bookmark `shortcut` opens, when one bookmark shortcut in force is `shortcut`.
pub fn bookmark_for<'a>(keys: &'a [BookmarkKey], shortcut: &Shortcut) -> Option<&'a str> {
    keys.iter()
        .find(|key| key.shortcut == *shortcut)
        .map(|key| key.bookmark.as_str())
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
