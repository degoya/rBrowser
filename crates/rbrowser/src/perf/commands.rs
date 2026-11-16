//! `performance.toggle_throttle` (`Leader V Q`, RB-020-09): the throttling of hidden views on
//! or off. It is the setting `performance.throttleHidden`, kept in the settings file; MCP sets
//! it with `settings_set`. Argument: `{ "enabled"?: bool }` (without it: a flip).

use std::collections::BTreeMap;

use rb_commands::CommandError;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use crate::control::Core;

/// The command's id.
pub const TOGGLE_THROTTLE: &str = "performance.toggle_throttle";
/// The i18n keys of its notice.
const THROTTLE_ON_KEY: &str = "perf.throttleOn";
const THROTTLE_OFF_KEY: &str = "perf.throttleOff";
/// The setting it changes.
const THROTTLE_SETTING: &str = "performance.throttleHidden";

/// Runs `id` when it is this module's; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    (id == TOGGLE_THROTTLE).then(|| toggle(core, args))
}

fn toggle(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let enabled = match args.get("enabled") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(on)) => Some(*on),
        Some(_) => {
            return Err(CommandError::InvalidArgs(
                "`enabled` is true or false".to_owned(),
            ));
        }
    };
    let mut user = core.user_settings();
    let on = enabled.unwrap_or(!user.performance.throttle_hidden);
    if let Some(path) = core.user_settings_path() {
        let written = crate::settings_window::user_file::update(path, |root| {
            let section = root.entry("performance").or_insert_with(|| json!({}));
            if !section.is_object() {
                *section = json!({});
            }
            section["throttleHidden"] = json!(on);
        });
        if let Err(error) = written {
            tracing::warn!(%error, "performance.throttleHidden was not saved");
        }
    }
    user.performance.throttle_hidden = on;
    core.replace_user_settings(user);
    core.notify(CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: if on {
            THROTTLE_ON_KEY
        } else {
            THROTTLE_OFF_KEY
        }
        .to_owned(),
        args: BTreeMap::new(),
        settings_key: Some(THROTTLE_SETTING.to_owned()),
    }));
    Ok(json!({ "throttleHidden": on }))
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
