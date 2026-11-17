//! The commands of the Web Vitals and resources (RB-060-10): `debug.vitals` and
//! `debug.resources` switch the measuring behind the vitals bar by writing `vitals.enabled` and
//! `resources.enabled` in the user settings. In the DevTools group (`D`); the job's `Leader D V`
//! is `devtools.open`, so the vitals take `Leader D C` (Core Web Vitals). No MCP tool: a display
//! switch, and the data are `view_metrics`. Each switch names its new state in the status bar,
//! since the labels may have nothing to show yet (RB-080-18). `debug.vitals` stands in the
//! DevTools menu, not under the page menu's Checks - it switches measuring, not the bar; the bar
//! switched on switches measuring on through [`measure_for_bar`] (RB-090-72).

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use crate::control::Core;
use crate::settings_window::Caller;

/// Measuring the Web Vitals on or off.
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
/// The status message after the vitals bar switched on switched measuring on too (RB-090-72).
pub const BAR_MEASURING_KEY: &str = "vitals.notice.bar_measuring";
/// The setting `debug.vitals` switches.
const VITALS_ENABLED: &str = "vitals.enabled";

/// Id, the key after `D`, the setting it switches.
const ROWS: [(&str, char, &str); 2] = [
    (VITALS, 'C', VITALS_ENABLED),
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
    let now = if key == VITALS_ENABLED {
        !user.vitals.enabled
    } else {
        !user.resources.enabled
    };
    switch(core, key, now, notice_key(key, now))?;
    Ok(json!({ "key": key, "enabled": now }))
}

/// For the vitals bar just switched on (`ui.vitals_bar.toggle`, RB-090-72): a bar without
/// measuring stays empty, so `vitals.enabled` off is switched on and the status bar says so.
/// `Ok(false)` when measuring was on already. The bar's setting written in the settings stays
/// apart from measuring; its help names the dependency.
///
/// # Errors
///
/// `vitals.enabled` could not be written.
pub fn measure_for_bar(core: &Core) -> Result<bool, CommandError> {
    if core.user_settings().vitals.enabled {
        return Ok(false);
    }
    switch(core, VITALS_ENABLED, true, BAR_MEASURING_KEY)?;
    Ok(true)
}

/// Writes `key` as `now` in the user settings, shows the labels at once and names the change
/// in the status bar with `message_key`.
fn switch(core: &Core, key: &str, now: bool, message_key: &str) -> Result<(), CommandError> {
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
        message_key: message_key.to_owned(),
        args: std::collections::BTreeMap::new(),
        settings_key: Some(key.to_owned()),
    }));
    Ok(())
}

/// The status message for `key` switched to `enabled`.
fn notice_key(key: &str, enabled: bool) -> &'static str {
    match (key == VITALS_ENABLED, enabled) {
        (true, true) => VITALS_ON_KEY,
        (true, false) => VITALS_OFF_KEY,
        (false, true) => RESOURCES_ON_KEY,
        (false, false) => RESOURCES_OFF_KEY,
    }
}

#[cfg(test)]
#[path = "defs_tests.rs"]
mod tests;
