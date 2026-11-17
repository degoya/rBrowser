//! The user's bindings: set, remove, reset, load from the user settings
//! (`shortcuts.bindings`, command id -> binding texts), export and import as JSON.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::registry::{NOTICE_UNKNOWN_COMMAND, NOTICE_USER_BINDING_REFUSED, Notice, texts};
use crate::validate::{bindings_conflict, check_binding, rejection_key};
use crate::{Binding, CommandError, CommandId, CommandRegistry};

/// The format version of an exported shortcut file.
pub const EXPORT_VERSION: u32 = 1;

/// An exported shortcut file: the user's bindings, in the shape of `shortcuts.bindings`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutExport {
    /// [`EXPORT_VERSION`].
    pub version: u32,
    /// Command id -> binding texts; an empty list unbinds the command.
    pub bindings: BTreeMap<String, Vec<String>>,
}

/// The user's bindings at one moment ([`CommandRegistry::user_snapshot`]), to put back with
/// [`CommandRegistry::restore_user`] when a batch of changes fails halfway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserSnapshot(BTreeMap<CommandId, Vec<Binding>>);

impl CommandRegistry {
    /// The user's bindings as they are now.
    #[must_use]
    pub fn user_snapshot(&self) -> UserSnapshot {
        UserSnapshot(self.user.clone())
    }

    /// Puts the user's bindings of `snapshot` back, every change since undone.
    pub fn restore_user(&mut self, snapshot: UserSnapshot) {
        self.user = snapshot.0;
    }

    /// Replaces the bindings of `id` with the user's (an empty list unbinds it; the palette
    /// still reaches it). Each binding passes [`CommandRegistry::check_for`] and they do not
    /// collide with each other. Bindings equal to the defaults clear the user's entry.
    pub fn set_bindings(
        &mut self,
        id: &CommandId,
        bindings: Vec<Binding>,
    ) -> Result<(), CommandError> {
        if !self.commands.contains_key(id) {
            return Err(CommandError::UnknownCommand(id.to_string()));
        }
        for (index, binding) in bindings.iter().enumerate() {
            self.check_for(id, binding)?;
            if bindings[..index]
                .iter()
                .any(|earlier| bindings_conflict(earlier, binding))
            {
                return Err(CommandError::Conflict {
                    binding: binding.to_string(),
                    other: id.to_string(),
                });
            }
        }
        if bindings == self.default_bindings(id) {
            self.user.remove(id);
        } else {
            self.user.insert(id.clone(), bindings);
        }
        Ok(())
    }

    /// Restores the defaults of `id`.
    pub fn reset(&mut self, id: &CommandId) {
        self.user.remove(id);
    }

    /// Restores every default.
    pub fn reset_all(&mut self) {
        self.user.clear();
    }

    /// Whether the user changed the bindings of `id`.
    pub fn is_changed(&self, id: &CommandId) -> bool {
        self.user.contains_key(id)
    }

    /// The user's bindings in the shape of the user settings' `shortcuts.bindings`.
    pub fn user_bindings(&self) -> BTreeMap<String, Vec<String>> {
        self.user
            .iter()
            .map(|(id, bindings)| (id.to_string(), texts(bindings)))
            .collect()
    }

    /// Applies the user's bindings from the settings on top of the defaults, replacing what
    /// was set before. All are applied together, so two commands may swap keys. What does not
    /// fit (an unknown command, a binding that does not parse or is reserved, a collision) is
    /// skipped with a notice, and that command keeps its defaults. The notices are returned
    /// only, not kept in [`CommandRegistry::notices`], so each is shown once.
    pub fn load_user_bindings(&mut self, bindings: &BTreeMap<String, Vec<String>>) -> Vec<Notice> {
        self.user.clear();
        let mut notices = Vec::new();
        for (text_id, texts) in bindings {
            // A retired command's binding binds nothing; it is no fault of the person's.
            if crate::retired::successor(text_id).is_some() {
                continue;
            }
            let known = CommandId::new(text_id)
                .ok()
                .filter(|id| self.commands.contains_key(id));
            let Some(id) = known else {
                notices.push(notice(NOTICE_UNKNOWN_COMMAND, text_id, None));
                continue;
            };
            let checked: Result<Vec<Binding>, CommandError> = texts
                .iter()
                .map(|text| {
                    let binding: Binding = text.parse()?;
                    check_binding(&binding, &self.reserved, self.platform)?;
                    Ok(binding)
                })
                .collect();
            match checked {
                Ok(list) if list == self.default_bindings(&id) => {}
                Ok(list) => {
                    self.user.insert(id, list);
                }
                Err(error) => {
                    notices.push(notice(NOTICE_USER_BINDING_REFUSED, text_id, Some(&error)))
                }
            }
        }
        // Collisions: drop one user entry at a time until none is left; each drop restores
        // defaults, which the next round checks again.
        while let Some((id, error)) = self.first_user_conflict() {
            self.user.remove(&id);
            notices.push(notice(
                NOTICE_USER_BINDING_REFUSED,
                id.as_str(),
                Some(&error),
            ));
        }
        notices
    }

    /// The first user entry that collides with itself or with another command.
    fn first_user_conflict(&self) -> Option<(CommandId, CommandError)> {
        for (id, list) in &self.user {
            for (index, binding) in list.iter().enumerate() {
                let own = list[..index]
                    .iter()
                    .any(|earlier| bindings_conflict(earlier, binding));
                let other = self.commands.keys().find(|other| {
                    *other != id
                        && self
                            .bindings(other)
                            .iter()
                            .any(|held| bindings_conflict(held, binding))
                });
                if own || other.is_some() {
                    let other = other.unwrap_or(id).to_string();
                    let binding = binding.to_string();
                    return Some((id.clone(), CommandError::Conflict { binding, other }));
                }
            }
        }
        None
    }

    /// The user's bindings as a shortcut file (pretty JSON).
    pub fn export_json(&self) -> String {
        let export = ShortcutExport {
            version: EXPORT_VERSION,
            bindings: self.user_bindings(),
        };
        serde_json::to_string_pretty(&export).unwrap_or_else(|_| String::from("{}"))
    }

    /// Reads a shortcut file and applies it in place of the user's bindings; returns the
    /// notices for what was skipped.
    pub fn import_json(&mut self, json: &str) -> Result<Vec<Notice>, CommandError> {
        let export: ShortcutExport = serde_json::from_str(json)
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
        if export.version != EXPORT_VERSION {
            return Err(CommandError::InvalidArgs(format!(
                "unsupported shortcut file version {}",
                export.version
            )));
        }
        Ok(self.load_user_bindings(&export.bindings))
    }
}

fn notice(message_key: &str, command: &str, error: Option<&CommandError>) -> Notice {
    Notice {
        message_key: message_key.to_owned(),
        command: command.to_owned(),
        binding: error.and_then(|error| match error {
            CommandError::InvalidBinding(text) => Some(text.clone()),
            CommandError::Conflict { binding, .. } => Some(binding.clone()),
            _ => None,
        }),
        reason_key: error.map(rejection_key),
    }
}

#[cfg(test)]
#[path = "user_bindings_tests.rs"]
mod tests;
