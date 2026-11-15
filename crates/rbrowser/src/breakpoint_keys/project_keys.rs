//! The project's own breakpoint shortcuts (`breakpoints[].shortcut`, RB-010-16): read with the
//! config, held to the same rules as every binding - a direct combination, not reserved, held by
//! no command, given to one breakpoint only - and what breaks a rule is a conflict the user is
//! told about (with a link to Settings > Keyboard Shortcuts). The registry always wins: a
//! project shortcut only sees keys no command took.

use rb_commands::validate::{
    REJECTED_CONFLICT, REJECTED_INVALID, bindings_conflict, check_binding,
};
use rb_commands::{Binding, CommandError, Key, Platform, ReservedList, Shortcut};
use rb_config::ProjectConfig;
use rb_ipc::ProjectShortcut;

/// The reason key of a leader chord in `shortcut`: a project shortcut is a direct combination.
pub const REJECTED_CHORD: &str = "sizes.conflict.chord";
/// The i18n key of the notification about one conflict; placeholders `{breakpoint}`,
/// `{binding}`, `{reason}` (an i18n key) and `{other}`.
pub const CONFLICT_KEY: &str = "sizes.conflict.notice";

/// A project shortcut in force.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectKey {
    /// The combination.
    pub shortcut: Shortcut,
    /// The breakpoint it sets.
    pub breakpoint: String,
}

/// A project shortcut that is not in force, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The breakpoint whose `shortcut` it is.
    pub breakpoint: String,
    /// The text of `shortcut`.
    pub binding: String,
    /// The i18n key of the reason (`shortcuts.reserved.*`, `shortcuts.rejected.*`).
    pub reason_key: String,
    /// The command or breakpoint that holds it, for a conflict.
    pub other: Option<String>,
}

/// The project shortcuts of `config`: those in force and the conflicts. `held` are the
/// effective bindings of the registry's commands, `(command id, binding)`.
pub fn project_keys(
    config: &ProjectConfig,
    held: &[(String, Binding)],
    reserved: &ReservedList,
    platform: Platform,
) -> (Vec<ProjectKey>, Vec<Conflict>) {
    let mut candidates: Vec<ProjectKey> = Vec::new();
    let mut conflicts = Vec::new();
    for breakpoint in &config.breakpoints {
        let Some(text) = breakpoint.shortcut.as_deref() else {
            continue;
        };
        let conflict = |reason_key: &str, other: Option<String>| Conflict {
            breakpoint: breakpoint.id.clone(),
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
        if let Some((command, _)) = held
            .iter()
            .find(|(_, held)| bindings_conflict(held, &binding))
        {
            conflicts.push(conflict(REJECTED_CONFLICT, Some(command.clone())));
            continue;
        }
        candidates.push(ProjectKey {
            shortcut,
            breakpoint: breakpoint.id.clone(),
        });
    }
    // A combination two breakpoints claim belongs to neither.
    let mut keys = Vec::new();
    for key in &candidates {
        let others: Vec<&ProjectKey> = candidates
            .iter()
            .filter(|other| other.shortcut == key.shortcut && other.breakpoint != key.breakpoint)
            .collect();
        match others.first() {
            Some(other) => conflicts.push(Conflict {
                breakpoint: key.breakpoint.clone(),
                binding: key.shortcut.to_string(),
                reason_key: REJECTED_CONFLICT.to_owned(),
                other: Some(other.breakpoint.clone()),
            }),
            None => keys.push(key.clone()),
        }
    }
    (keys, conflicts)
}

/// Every project shortcut of `config` in config order, for the shortcut editor: in force (in
/// `keys`) or with the reason of its conflict.
pub fn listing(
    config: &ProjectConfig,
    keys: &[ProjectKey],
    conflicts: &[Conflict],
) -> Vec<ProjectShortcut> {
    config
        .breakpoints
        .iter()
        .filter_map(|breakpoint| {
            let text = breakpoint.shortcut.as_deref()?;
            let id = breakpoint.id.as_str();
            if let Some(conflict) = conflicts.iter().find(|c| c.breakpoint == id) {
                return Some(ProjectShortcut {
                    breakpoint: id.to_owned(),
                    binding: conflict.binding.clone(),
                    reason_key: Some(conflict.reason_key.clone()),
                    other: conflict.other.clone(),
                });
            }
            let binding = keys
                .iter()
                .find(|key| key.breakpoint == id)
                .map_or_else(|| text.to_owned(), |key| key.shortcut.to_string());
            Some(ProjectShortcut {
                breakpoint: id.to_owned(),
                binding,
                reason_key: None,
                other: None,
            })
        })
        .collect()
}

/// Whether `shortcut` can be a project shortcut at all: a combination with Ctrl, Alt or Win,
/// or a function key. Plain typing never is (the reserved list says so too); this lets the
/// keyboard path skip the config for every typed character.
pub fn may_be_project_key(shortcut: &Shortcut) -> bool {
    let m = shortcut.modifiers;
    m.ctrl || m.alt || m.meta || matches!(shortcut.key, Key::F(_))
}

/// The breakpoint `shortcut` sets, when one project shortcut in force is `shortcut`.
pub fn breakpoint_for<'a>(keys: &'a [ProjectKey], shortcut: &Shortcut) -> Option<&'a str> {
    keys.iter()
        .find(|key| key.shortcut == *shortcut)
        .map(|key| key.breakpoint.as_str())
}

#[cfg(test)]
#[path = "project_keys_tests.rs"]
mod tests;
