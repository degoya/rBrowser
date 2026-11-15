//! The rules one binding must meet, shared by the registry (registration, the editor) and the
//! lint: the shape of a leader chord, the reservation list, and conflicts between bindings.

use crate::{Binding, CommandError, Group, Key, LEADER, Platform, ReservedList};

/// The i18n key of the refusal of the bare leader as a direct binding.
pub const REJECTED_LEADER: &str = "shortcuts.rejected.leader";
/// The i18n key of the refusal of a binding another command holds.
pub const REJECTED_CONFLICT: &str = "shortcuts.rejected.conflict";
/// The i18n key of the refusal of a malformed binding.
pub const REJECTED_INVALID: &str = "shortcuts.rejected.invalid";

/// The group whose leader key is `key`, if any.
pub fn group_of_key(key: Key) -> Option<Group> {
    Group::ALL
        .into_iter()
        .find(|group| Key::Char(group.leader_key()) == key)
}

/// Checks one binding on its own: a direct combination must not be the leader or reserved on
/// `platform`; a leader chord has one or two keys, no `Escape` (it cancels), a single key that
/// is no group key (the group would never open), and a two-key chord starts with a group key.
pub fn check_binding(
    binding: &Binding,
    reserved: &ReservedList,
    platform: Platform,
) -> Result<(), CommandError> {
    match binding {
        Binding::Direct(shortcut) if *shortcut == LEADER => {
            Err(CommandError::Reserved(REJECTED_LEADER.to_owned()))
        }
        Binding::Direct(shortcut) => match reserved.find(shortcut, platform) {
            Some(entry) => Err(CommandError::Reserved(entry.reason_key.clone())),
            None => Ok(()),
        },
        Binding::Leader(chord) => {
            let invalid = || CommandError::InvalidBinding(binding.to_string());
            match chord.keys.as_slice() {
                keys if keys.contains(&Key::Escape) => Err(invalid()),
                [single] => match group_of_key(*single) {
                    Some(group) => Err(CommandError::Conflict {
                        binding: binding.to_string(),
                        other: group.title_key(),
                    }),
                    None => Ok(()),
                },
                [first, _] if group_of_key(*first).is_some() => Ok(()),
                _ => Err(invalid()),
            }
        }
    }
}

/// Whether two bindings cannot both be held: equal, or one leader chord is the start of the
/// other (`Leader S` would fire before `Leader S S` could be typed).
pub fn bindings_conflict(a: &Binding, b: &Binding) -> bool {
    match (a, b) {
        (Binding::Leader(a), Binding::Leader(b)) => {
            a.keys.starts_with(&b.keys) || b.keys.starts_with(&a.keys)
        }
        _ => a == b,
    }
}

/// The i18n key that explains a refusal to the user: the reserved reason, the conflict or the
/// malformed binding.
pub fn rejection_key(error: &CommandError) -> String {
    match error {
        CommandError::Reserved(key) => key.clone(),
        CommandError::Conflict { .. } => REJECTED_CONFLICT.to_owned(),
        _ => REJECTED_INVALID.to_owned(),
    }
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
