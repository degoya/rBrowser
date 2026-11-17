//! The commands of rBrowser's own theme (RB-080-95) in the View group (`V`): `ui.theme.cycle`
//! (`Leader V 0`, the toolbar's switch) steps system, light, dark; `ui.theme.set { theme }`
//! (`Leader V =`) sets one - without `theme` (a shortcut, the palette) it goes back to
//! following Windows. Both write `ui.theme` like Settings does, and the status bar says what the
//! theme is now. Neither has a tool of its own: `settings_set` writes `ui.theme` and
//! `settings_get` reads it.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::Theme;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::{THEME_KEY, next, parse, word};
use crate::control::Core;
use crate::settings_window::Caller;

/// Steps the theme: system, light, dark.
pub const CYCLE: &str = "ui.theme.cycle";
/// Sets the theme (`theme`: `system`, `light`, `dark`; none: `system`).
pub const SET: &str = "ui.theme.set";
/// Why neither command has a tool.
pub const OMITTED: &str = "app display setting; settings_set writes ui.theme";

/// Id, the key after `V`.
const ROWS: [(&str, char); 2] = [(CYCLE, '0'), (SET, '=')];

/// The definitions; title key `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, key)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::View,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::View.leader_key()), Key::Char(*key)],
                })],
                mcp: McpCoverage::Omitted(OMITTED.to_owned()),
            })
        })
        .collect()
}

/// Registers both commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id, &args)
    })
}

/// Runs `ui.theme.cycle` or `ui.theme.set` with `args`.
///
/// # Errors
///
/// An unknown id, a `theme` that is none of the three, or the setting could not be written.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    let theme = match id {
        CYCLE => next(core.user_settings().ui.theme),
        SET => theme_of(args)?,
        other => return Err(CommandError::UnknownCommand(other.to_owned())),
    };
    let answer = crate::settings_window::set_setting(
        core,
        THEME_KEY,
        &json!(word(theme)),
        rb_settings::Scope::User,
        Caller::Shell,
    );
    if let CoreToShell::SettingRejected { .. } = answer {
        return Err(CommandError::Failed(format!(
            "{THEME_KEY} could not be written"
        )));
    }
    core.notify(notice(theme));
    Ok(json!({ "theme": word(theme) }))
}

/// The theme `ui.theme.set` was given; `system` without one.
fn theme_of(args: &Value) -> Result<Theme, CommandError> {
    match args.get("theme") {
        None | Some(Value::Null) => Ok(Theme::System),
        Some(Value::String(text)) => parse(text).ok_or_else(|| invalid(text)),
        Some(other) => Err(invalid(&other.to_string())),
    }
}

fn invalid(given: &str) -> CommandError {
    CommandError::InvalidArgs(format!("`theme` is system, light or dark, not {given}"))
}

/// The status bar's notice: the theme in effect now, linked to its setting.
#[must_use]
pub fn notice(theme: Theme) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: notice_key(theme).to_owned(),
        args: BTreeMap::new(),
        settings_key: Some(THEME_KEY.to_owned()),
    })
}

/// The i18n key of the notice for `theme`.
#[must_use]
pub fn notice_key(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "theme.name.system",
        Theme::Light => "theme.name.light",
        Theme::Dark => "theme.name.dark",
    }
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
