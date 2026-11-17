//! Shortcuts over `settings_set` (RB-030-03): `shortcuts.bindings.<commandId>` with a list of
//! bindings in text form (`["Ctrl+Shift+Space S S"]`, `[]` unbinds, `null` restores the
//! defaults), or `shortcuts.bindings` with an object of such lists (`null` restores every
//! default). Every binding is checked as the shortcut editor checks it - the reservation list
//! (`Ctrl+T` is the browser's), conflicts with another command - before anything changes, and
//! the change goes through the editor's own path, so the registry and the settings file stay in
//! step. User scope only: shortcuts are a person's, never a project's. A command MCP may not run
//! (`omitted("protected: ...")`: trust, plugins, MCP itself) cannot be rebound over MCP either.

use rb_commands::McpCoverage;
use rb_ipc::{BindingCheck, CoreToShell};
use rb_mcp::{SettingScope, ToolFailure};
use serde_json::Value;

use crate::control::Core;
use crate::settings_window::SHORTCUTS_KEY;

/// The code of a binding the editor would refuse (the reason's own key is in the message).
pub const REFUSED: &str = "settings.rejected.shortcut";
/// The code of a command whose bindings are not MCP's (the settings' own refusal code).
pub const PROTECTED: &str = "settings.rejected.protected";

/// One change of the bindings.
pub use crate::control::BindingChange as Change;

/// Whether `key` is a shortcut key `settings_set` hands here.
pub fn is_shortcut_key(key: &str) -> bool {
    key == SHORTCUTS_KEY
        || key
            .strip_prefix(SHORTCUTS_KEY)
            .is_some_and(|rest| rest.starts_with('.') && rest.len() > 1)
}

/// The changes `value` at `key` asks for.
///
/// # Errors
///
/// A value of the wrong shape.
pub fn changes(key: &str, value: &Value) -> Result<Vec<Change>, ToolFailure> {
    let command = key
        .strip_prefix(SHORTCUTS_KEY)
        .and_then(|rest| rest.strip_prefix('.'));
    match (command, value) {
        (Some(id), Value::Null) => Ok(vec![Change::Reset(Some(id.to_owned()))]),
        (Some(id), list) => Ok(vec![Change::Set(id.to_owned(), texts(id, list)?)]),
        (None, Value::Null) => Ok(vec![Change::Reset(None)]),
        (None, Value::Object(map)) => map
            .iter()
            .map(|(id, list)| match list {
                Value::Null => Ok(Change::Reset(Some(id.clone()))),
                list => texts(id, list).map(|texts| Change::Set(id.clone(), texts)),
            })
            .collect(),
        (None, _) => Err(invalid(
            "shortcuts.bindings takes an object of command ids to lists of bindings",
        )),
    }
}

fn texts(id: &str, list: &Value) -> Result<Vec<String>, ToolFailure> {
    list.as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| invalid(&format!("the bindings of `{id}` are a list of texts")))
}

fn invalid(message: &str) -> ToolFailure {
    ToolFailure::new("settings.rejected.invalid", message).with_setting(SHORTCUTS_KEY)
}

/// The failure for a binding the editor refuses: the reason in English, the other command
/// on a conflict.
pub fn refusal(id: &str, text: &str, check: &BindingCheck) -> ToolFailure {
    let reason = check
        .reason_key
        .as_deref()
        .map_or_else(|| "refused".to_owned(), rb_i18n::t);
    let conflict = check
        .conflict
        .as_deref()
        .map(|other| format!(" (bound to `{other}`)"))
        .unwrap_or_default();
    ToolFailure::new(
        REFUSED,
        format!("`{text}` cannot be bound to `{id}`: {reason}{conflict}"),
    )
    .with_setting(SHORTCUTS_KEY)
}

/// Applies `value` at the shortcut `key` in `scope` through the shortcut editor's path, after
/// every binding passed its check - every change or none.
///
/// # Errors
///
/// Another scope than user, a value of the wrong shape, an unknown command, a reserved or
/// taken binding.
pub fn set(core: &Core, key: &str, value: &Value, scope: SettingScope) -> Result<(), ToolFailure> {
    if scope != SettingScope::User {
        return Err(ToolFailure::new(
            "settings.rejected.scope",
            "shortcuts are the user's: use scope user",
        )
        .with_setting(SHORTCUTS_KEY));
    }
    let changes = changes(key, value)?;
    let protected = protected_commands(core);
    for change in &changes {
        let Change::Set(id, texts) = change else {
            continue;
        };
        if protected.iter().any(|known| known == id) {
            return Err(ToolFailure::new(
                PROTECTED,
                format!(
                    "`{id}` is protected: only a person can change its shortcut in rBrowser's \
                     settings"
                ),
            )
            .with_setting(SHORTCUTS_KEY));
        }
        for text in texts {
            match core.check_binding(id, text) {
                Ok(CoreToShell::BindingCheck(check)) if check.ok => {}
                Ok(CoreToShell::BindingCheck(check)) => return Err(refusal(id, text, &check)),
                Ok(_) => {}
                Err((_, message)) => return Err(unknown(id, &message)),
            }
        }
    }
    // The whole batch or nothing: the check above ran against the registry before the batch,
    // so two commands asking for one chord both passed it.
    core.apply_bindings(&changes)
        .map(drop)
        .map_err(|(id, (_, message))| unknown(&id, &message))
}

/// The commands MCP may not run (`omitted("protected: ...")`): their bindings are not MCP's
/// either.
fn protected_commands(core: &Core) -> Vec<String> {
    let CoreToShell::Commands { commands, .. } = core.commands() else {
        return Vec::new();
    };
    commands
        .into_iter()
        .filter(|info| matches!(&info.mcp, McpCoverage::Omitted(reason) if reason.starts_with("protected")))
        .map(|info| info.id)
        .collect()
}

fn unknown(id: &str, message: &str) -> ToolFailure {
    ToolFailure::new(
        "settings.rejected.invalid",
        format!("the bindings of `{id}` were refused: {message}"),
    )
    .with_setting(SHORTCUTS_KEY)
}

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;
