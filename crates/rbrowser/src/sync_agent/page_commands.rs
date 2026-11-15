//! The page API's toggle commands (RB-020-08) for the one registry: the message bus
//! (`Leader S B`), the page API (`Leader S A`) and the user-agent suffix (`Leader S I`), each
//! flipping its switch for the session over the project's setting (`bus.enabled`,
//! `pageApi.enabled`, `pageApi.uaSuffix`) until that setting changes. MCP changes the settings
//! themselves; the commands have no tool of their own.

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::page_state::Switch;
use super::page_tasks::refresh_all;
use crate::control::Core;

/// Message bus on/off.
pub const TOGGLE_BUS: &str = "bus.toggle";
/// Page API on/off (from the next document on).
pub const TOGGLE_API: &str = "page_api.toggle";
/// The ` rBrowser/<version>` user-agent suffix on/off.
pub const TOGGLE_UA_SUFFIX: &str = "page_api.toggle_ua_suffix";
/// Why the toggles have no MCP tool of their own.
pub const TOGGLES_OMITTED: &str =
    "settings: MCP sets bus.enabled, pageApi.enabled and pageApi.uaSuffix with the settings tools";

/// Id, the key after `Leader S`, switch.
const TOGGLES: [(&str, char, Switch); 3] = [
    (TOGGLE_BUS, 'B', Switch::Bus),
    (TOGGLE_API, 'A', Switch::Api),
    (TOGGLE_UA_SUFFIX, 'I', Switch::UaSuffix),
];

/// The definitions, title key `commands.<id>`.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    TOGGLES
        .iter()
        .map(|(id, key, _)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Sync,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: vec![Key::Char('S'), Key::Char(*key)],
                })],
                mcp: McpCoverage::Omitted(TOGGLES_OMITTED.to_owned()),
            })
        })
        .collect()
}

/// Registers the toggles; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let switch = switch_of(def.id.as_str())?;
            registry
                .register(def, handler(Arc::downgrade(core), switch))
                .err()
        })
        .collect()
}

/// The switch command `id` flips.
fn switch_of(id: &str) -> Option<Switch> {
    TOGGLES
        .iter()
        .find(|(own, _, _)| *own == id)
        .map(|(_, _, switch)| *switch)
}

fn handler(core: Weak<Core>, switch: Switch) -> Handler {
    Arc::new(move |_: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        let host = core.host().clone();
        let on = host.page_api().toggle(switch, &host.config());
        match switch {
            Switch::UaSuffix => host.reapply_user_agent(),
            Switch::Api | Switch::Bus => {
                core.runtime()
                    .spawn(async move { refresh_all(&host).await });
            }
        }
        core.notify(CoreToShell::Notification(notice(switch, on)));
        Ok(json!({ "enabled": on }))
    })
}

/// What the shell shows after a toggle, with a link to the setting.
fn notice(switch: Switch, on: bool) -> Notification {
    let (message_key, settings_key) = match (switch, on) {
        (Switch::Bus, true) => ("page.notice.bus_on", "bus.enabled"),
        (Switch::Bus, false) => ("page.notice.bus_off", "bus.enabled"),
        (Switch::Api, true) => ("page.notice.api_on", "pageApi.enabled"),
        (Switch::Api, false) => ("page.notice.api_off", "pageApi.enabled"),
        (Switch::UaSuffix, true) => ("page.notice.ua_suffix_on", "pageApi.uaSuffix"),
        (Switch::UaSuffix, false) => ("page.notice.ua_suffix_off", "pageApi.uaSuffix"),
    };
    let args = match switch {
        Switch::UaSuffix => [("version".to_owned(), env!("CARGO_PKG_VERSION").to_owned())].into(),
        Switch::Api | Switch::Bus => Default::default(),
    };
    Notification {
        level: NotificationLevel::Info,
        message_key: message_key.to_owned(),
        args,
        settings_key: Some(settings_key.to_owned()),
    }
}

#[cfg(test)]
#[path = "page_commands_tests.rs"]
mod tests;
