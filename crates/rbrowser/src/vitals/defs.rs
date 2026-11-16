//! The commands of the Web Vitals and resources (RB-060-10): `debug.vitals` and
//! `debug.resources` switch the cell labels' badges by writing `vitals.enabled` and
//! `resources.enabled` in the user settings. In the DevTools group (`D`); the job's `Leader D V`
//! is `devtools.open`, so the vitals take `Leader D C` (Core Web Vitals). No MCP tool: a display
//! switch, and the data are `view_metrics`. Each switch names its new state in the status bar,
//! since the labels may have nothing to show yet (RB-080-18).

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use crate::control::Core;
use crate::settings_window::Caller;

/// Badges on or off.
pub const VITALS: &str = "debug.vitals";
/// Resource readings on or off.
pub const RESOURCES: &str = "debug.resources";
/// Why neither has an MCP tool.
pub const OMITTED: &str = "display switch; the data are view_metrics";
/// The status message after `debug.vitals` switched the badges on.
pub const VITALS_ON_KEY: &str = "vitals.notice.vitals_on";
/// ... and off.
pub const VITALS_OFF_KEY: &str = "vitals.notice.vitals_off";
/// The status message after `debug.resources` switched the readings on.
pub const RESOURCES_ON_KEY: &str = "vitals.notice.resources_on";
/// ... and off.
pub const RESOURCES_OFF_KEY: &str = "vitals.notice.resources_off";

/// Id, the key after `D`, the setting it switches.
const ROWS: [(&str, char, &str); 2] = [
    (VITALS, 'C', "vitals.enabled"),
    (RESOURCES, 'R', "resources.enabled"),
];

/// The definitions; title key `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds them valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    ROWS.iter()
        .map(|(id, key, _)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Devtools,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Devtools.leader_key()), Key::Char(*key)],
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

/// The setting `id` switches.
#[must_use]
pub fn setting_of(id: &str) -> Option<&'static str> {
    ROWS.iter()
        .find(|(own, _, _)| *own == id)
        .map(|(_, _, key)| *key)
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        toggle(&core, &id)
    })
}

/// Writes the switched setting, shows the labels at once and names the new state in the status
/// bar.
fn toggle(core: &Core, id: &str) -> Result<Value, CommandError> {
    let key = setting_of(id).ok_or_else(|| CommandError::UnknownCommand(id.to_owned()))?;
    let user = core.user_settings();
    let now = if key == "vitals.enabled" {
        !user.vitals.enabled
    } else {
        !user.resources.enabled
    };
    let answer = crate::settings_window::set_setting(
        core,
        key,
        &Value::Bool(now),
        rb_settings::Scope::User,
        Caller::Shell,
    );
    if let CoreToShell::SettingRejected { .. } = answer {
        return Err(CommandError::Failed(format!("{key} could not be written")));
    }
    super::publish(core);
    core.notify(CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: notice_key(key, now).to_owned(),
        args: std::collections::BTreeMap::new(),
        settings_key: Some(key.to_owned()),
    }));
    Ok(json!({ "key": key, "enabled": now }))
}

/// The status message for `key` switched to `enabled`.
fn notice_key(key: &str, enabled: bool) -> &'static str {
    match (key == "vitals.enabled", enabled) {
        (true, true) => VITALS_ON_KEY,
        (true, false) => VITALS_OFF_KEY,
        (false, true) => RESOURCES_ON_KEY,
        (false, false) => RESOURCES_OFF_KEY,
    }
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
