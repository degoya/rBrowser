//! The shortcut editor's requests (RB-010-17): set, reset, check, export and import the user's
//! bindings. A change is applied to the registry and written back to the user settings
//! (`shortcuts.bindings`), keeping everything else in the file as it is - the entries the
//! registry did not take included (a plugin command not loaded yet, a refused binding). Over a
//! settings file that could not be read nothing is written: the change holds for the session.

use std::collections::BTreeMap;
use std::path::Path;

use rb_commands::registry::{NOTICE_BINDING_DROPPED, Notice};
use rb_commands::user_bindings::ShortcutExport;
use rb_commands::validate::rejection_key;
use rb_commands::{Binding, CommandError, CommandId, CommandRegistry};
use rb_ipc::{BindingCheck, CoreToShell, IpcErrorCode, Notification, NotificationLevel};
use serde_json::{Map, Value};

use super::Core;
use crate::cdp::lock;

/// A refused request: the code and message for `onFailure`.
pub type Refusal = (IpcErrorCode, String);

/// The i18n key of the notice that a change was not saved (the settings file did not load).
pub const NOT_SAVED_KEY: &str = "shortcuts.notice.not_saved";

/// One change of the user's bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingChange {
    /// The command's bindings become these (an empty list unbinds it).
    Set(String, Vec<String>),
    /// The command's defaults come back; every command's without one.
    Reset(Option<String>),
}

impl BindingChange {
    /// What the change is about, for a refusal.
    pub fn subject(&self) -> &str {
        match self {
            Self::Set(id, _) | Self::Reset(Some(id)) => id,
            Self::Reset(None) => "every command",
        }
    }
}

impl Core {
    /// `setBindings`: the user's bindings of `id` (an empty list unbinds it).
    pub fn set_bindings(&self, id: &str, texts: &[String]) -> Result<CoreToShell, Refusal> {
        self.apply_bindings(&[BindingChange::Set(id.to_owned(), texts.to_vec())])
            .map_err(|(_, refused)| refused)
    }

    /// Applies every change of a batch or none (`settings_set` with several commands over MCP,
    /// RB-030-03): when one is refused the user's bindings are put back and nothing is written.
    /// The refusal names the change's subject.
    pub fn apply_bindings(
        &self,
        changes: &[BindingChange],
    ) -> Result<CoreToShell, (String, Refusal)> {
        {
            let mut registry = lock(&self.registry);
            let mut skipped = lock(&self.skipped);
            let registry_before = registry.user_snapshot();
            let skipped_before = skipped.clone();
            for change in changes {
                if let Err(refused) = apply(&mut registry, &mut skipped, change) {
                    registry.restore_user(registry_before);
                    *skipped = skipped_before;
                    return Err((change.subject().to_owned(), refused));
                }
            }
        }
        self.bindings_changed();
        Ok(self.commands_sent())
    }

    /// `resetBinding`: the defaults of `id`, or of every command.
    pub fn reset_binding(&self, id: Option<&str>) -> Result<CoreToShell, Refusal> {
        self.apply_bindings(&[BindingChange::Reset(id.map(str::to_owned))])
            .map_err(|(_, refused)| refused)
    }

    /// `checkBinding`: whether `text` may be bound to `id`, and why not.
    pub fn check_binding(&self, id: &str, text: &str) -> Result<CoreToShell, Refusal> {
        let id = command_id(id)?;
        let checked = text
            .parse::<Binding>()
            .and_then(|binding| lock(&self.registry).check_for(&id, &binding));
        Ok(CoreToShell::BindingCheck(match checked {
            Ok(()) => BindingCheck {
                ok: true,
                reason_key: None,
                conflict: None,
            },
            Err(error) => BindingCheck {
                ok: false,
                reason_key: Some(rejection_key(&error)),
                conflict: match error {
                    CommandError::Conflict { other, .. } => Some(other),
                    _ => None,
                },
            },
        }))
    }

    /// `exportBindings`: the user's bindings as a shortcut file.
    pub fn export_bindings(&self) -> CoreToShell {
        CoreToShell::BindingsExport {
            json: lock(&self.registry).export_json(),
        }
    }

    /// `importBindings`: a shortcut file in place of the user's bindings; what does not fit
    /// becomes a notification.
    pub fn import_bindings(&self, json: &str) -> Result<CoreToShell, Refusal> {
        let notices = {
            let mut registry = lock(&self.registry);
            let notices = registry.import_json(json).map_err(refusal)?;
            let file: ShortcutExport = serde_json::from_str(json)
                .map_err(|error| (IpcErrorCode::BadRequest, error.to_string()))?;
            *lock(&self.skipped) = skipped_entries(&registry, &file.bindings);
            notices
        };
        self.notices(notices);
        self.bindings_changed();
        Ok(self.commands_sent())
    }

    /// The registry's commands after an edit of the bindings, sent to the shell as well: its
    /// tooltips name shortcuts and follow the edit at once (RB-080-60).
    fn commands_sent(&self) -> CoreToShell {
        let commands = self.commands();
        self.send(&commands);
        commands
    }

    /// Keeps the user settings in step with the registry and writes them back - under the
    /// settings' one-change lock (RB-099-19): a setting change reads the user file and
    /// the settings in effect and writes them back whole, so a binding written beside it would be
    /// put back to the old one.
    fn bindings_changed(&self) {
        let _one_at_a_time = lock(&crate::settings_window::ONE_CHANGE);
        let mut bindings = lock(&self.skipped).clone();
        bindings.extend(lock(&self.registry).user_bindings());
        lock(&self.user).shortcuts.bindings = bindings.clone();
        let Some(path) = &self.user_path else {
            return;
        };
        if self.user_load_failed {
            tracing::warn!(
                path = %path.display(),
                "the shortcuts were not saved: the settings file did not load"
            );
            self.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Warning,
                message_key: NOT_SAVED_KEY.to_owned(),
                args: BTreeMap::new(),
                settings_key: Some("shortcuts.bindings".to_owned()),
            }));
            return;
        }
        let value = serde_json::to_value(&bindings).unwrap_or(Value::Null);
        if let Err(error) = write_key(path, &["shortcuts", "bindings"], value) {
            tracing::warn!(%error, path = %path.display(), "the shortcuts were not saved");
        }
    }
}

/// Applies one `change` to `registry` and drops what it settles from `skipped`.
fn apply(
    registry: &mut CommandRegistry,
    skipped: &mut BTreeMap<String, Vec<String>>,
    change: &BindingChange,
) -> Result<(), Refusal> {
    match change {
        BindingChange::Set(id, texts) => {
            let id = command_id(id)?;
            let bindings = texts
                .iter()
                .map(|text| text.parse::<Binding>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(refusal)?;
            registry.set_bindings(&id, bindings).map_err(refusal)?;
            skipped.remove(id.as_str());
        }
        BindingChange::Reset(Some(id)) => {
            let id = command_id(id)?;
            registry.reset(&id);
            skipped.remove(id.as_str());
        }
        BindingChange::Reset(None) => {
            registry.reset_all();
            // What belongs to a command not loaded yet stays for it.
            skipped.retain(|id, _| !is_registered(registry, id));
        }
    }
    Ok(())
}

/// The default bindings `notices` say were left unbound (`NOTICE_BINDING_DROPPED`), by
/// command, for the editor's `droppedDefaults` (RB-060-11).
pub(super) fn dropped_defaults(notices: &[Notice]) -> BTreeMap<String, Vec<String>> {
    let mut dropped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for notice in notices {
        if notice.message_key == NOTICE_BINDING_DROPPED
            && let Some(binding) = &notice.binding
        {
            dropped
                .entry(notice.command.clone())
                .or_default()
                .push(binding.clone());
        }
    }
    dropped
}

/// The entries of `source` (`shortcuts.bindings` of a file) that `registry` did not take as
/// written: an unknown command (a plugin not loaded yet) or bindings refused at load.
pub(super) fn skipped_entries(
    registry: &CommandRegistry,
    source: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    source
        .iter()
        .filter(|(id, texts)| {
            let Some(id) = CommandId::new(id.as_str())
                .ok()
                .filter(|id| registry.get(id).is_some())
            else {
                return true;
            };
            let parsed: Result<Vec<Binding>, _> = texts.iter().map(|text| text.parse()).collect();
            !parsed.is_ok_and(|list| list == registry.bindings(&id))
        })
        .map(|(id, texts)| (id.clone(), texts.clone()))
        .collect()
}

fn is_registered(registry: &CommandRegistry, id: &str) -> bool {
    CommandId::new(id).is_ok_and(|id| registry.get(&id).is_some())
}

fn command_id(id: &str) -> Result<CommandId, Refusal> {
    CommandId::new(id).map_err(|error| (IpcErrorCode::NotFound, error.to_string()))
}

fn refusal(error: CommandError) -> Refusal {
    let code = match error {
        CommandError::UnknownCommand(_) => IpcErrorCode::NotFound,
        CommandError::InvalidBinding(_) | CommandError::InvalidArgs(_) => IpcErrorCode::BadRequest,
        CommandError::Reserved(_) | CommandError::Conflict { .. } => IpcErrorCode::Refused,
        _ => IpcErrorCode::Internal,
    };
    (code, error.to_string())
}

/// Sets `value` at `key` (nested objects) in the JSON file at `path`, keeping the rest; a
/// missing file starts empty.
pub fn write_key(path: &Path, key: &[&str], value: Value) -> Result<(), rb_config::ConfigError> {
    let mut document = if path.exists() {
        rb_config::read_json(path)?
    } else {
        Value::Object(Map::new())
    };
    let mut node = &mut document;
    for segment in key {
        if !node.is_object() {
            *node = Value::Object(Map::new());
        }
        node = node
            .as_object_mut()
            .expect("made an object above")
            .entry((*segment).to_owned())
            .or_insert(Value::Null);
    }
    *node = value;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| rb_config::ConfigError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    rb_config::write_json(path, &document)
}

#[cfg(test)]
#[path = "bindings_tests.rs"]
mod tests;
