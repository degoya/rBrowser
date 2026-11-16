//! The portal (RB-080-05): real devices in the LAN - a phone, a tablet - join the sync. A device
//! scans the QR code in rBrowser's Portal panel, which holds a one-time pairing URL; the portal
//! swaps the token for a device cookie and from then on serves the active view's page through a
//! reverse proxy with the portal agent in it, which follows the grid in JavaScript.
//!
//! | Part | Where |
//! | --- | --- |
//! | Pairing token, device sessions, rate limit (pure) | [`pairing`] |
//! | The project's origins, the upstream, URL rewriting (pure) | [`origins`] |
//! | Request and response heads (pure) | [`rewrite`] |
//! | The agent in HTML, chunked bodies (pure) | [`inject`] |
//! | What a device replays, what it reports (pure) | [`relay`] |
//! | The listener and its routes; the forwarding; a device's socket | [`server`], [`proxy`], `socket` |
//! | Connections at once, in all and per address | [`limits`] |
//! | Devices, sockets and the page behind the lock | [`state`] |
//! | The LAN address; the QR code | [`interface`], [`qr`] |
//! | Start, stop, the panel's state, the glue to the grid | [`app`] |
//! | The MCP tools `portal_*` | [`mcp`] |
//!
//! **Security.** Off by default: `portal.enabled` is a person's setting, protected from MCP,
//! and even then the portal listens only once a person started it (`Leader S P`, `Leader S Q`;
//! no MCP tool starts it), on one LAN address - a private one unless `portal.interface` names
//! another - never `0.0.0.0`. The pairing token (128 bits) works
//! once, for `portal.pairingTtlSec`, compared in constant time, five failures a minute per
//! address; it reaches the panel only, never a log or MCP. Device sessions (256 bits) are
//! `HttpOnly`, `SameSite=Strict` cookies, revoked by `portal.disconnect`. The listener serves
//! nothing of MCP, CDP or the shell, forwards only to the project's `http` origins and refuses
//! a foreign `Host`. Plain HTTP in the LAN: a password typed in the grid never goes to a device.
//! The threat model's portal section is `docs/security/threat-model.md`.

pub mod app;
pub mod inject;
pub mod interface;
pub mod limits;
pub mod mcp;
pub mod origins;
pub mod pairing;
pub mod proxy;
pub mod qr;
pub mod relay;
pub mod rewrite;
pub mod server;
mod socket;
pub mod state;

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use crate::control::Core;

/// `Leader S P`: start (with a fresh QR code) or stop the portal (`{ "enabled"?: bool }`).
pub const TOGGLE: &str = "portal.toggle";
/// `Leader S Q`: a fresh pairing QR code in the Portal panel (starts the portal if needed).
pub const SHOW_QR: &str = "portal.show_qr";
/// `Leader P D`: the Portal panel with the paired devices.
pub const DEVICES: &str = "portal.devices";
/// `Leader S D`: disconnect a device (`{ "device"?: id }`) or every device.
pub const DISCONNECT: &str = "portal.disconnect";
/// The setting that allows the portal.
pub const ENABLED_KEY: &str = "portal.enabled";
/// The setting of the address the portal listens on.
pub const INTERFACE_KEY: &str = "portal.interface";
/// The panel's route in the pages window.
pub const ROUTE: &str = "portal";
/// Why `portal.show_qr` has no MCP tool.
pub const SHOW_QR_OMITTED: &str =
    "protected: shows the one-time pairing code to a person; portal_devices answers the address";
/// Why `portal.toggle` has no MCP tool (`portal_stop` stops it).
pub const TOGGLE_OMITTED: &str = "protected: a LAN listener is a human's decision; MCP cannot start \
     the portal, portal_stop stops it";
/// The portal agent (`agent/dist/rb-portal-agent.js`), copied by `build.rs`; empty when
/// `agent/` was not built, and the devices then load the page without following.
pub const PORTAL_AGENT_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rb-portal-agent.js"));

/// The command definitions.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    [
        (
            TOGGLE,
            Group::Sync,
            'P',
            McpCoverage::Omitted(TOGGLE_OMITTED.to_owned()),
        ),
        (
            SHOW_QR,
            Group::Sync,
            'Q',
            McpCoverage::Omitted(SHOW_QR_OMITTED.to_owned()),
        ),
        (
            DEVICES,
            Group::Panels,
            'D',
            McpCoverage::Covered(mcp::DEVICES_TOOL.to_owned()),
        ),
        (
            DISCONNECT,
            Group::Sync,
            'D',
            McpCoverage::Covered(mcp::DISCONNECT_TOOL.to_owned()),
        ),
    ]
    .into_iter()
    .map(|(id, group, key, mcp)| {
        Ok(CommandDef {
            id: CommandId::new(id)?,
            title_key: format!("commands.{id}"),
            group,
            default_bindings: vec![Binding::Leader(Chord {
                keys: vec![Key::Char(group.leader_key()), Key::Char(key)],
            })],
            mcp,
        })
    })
    .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
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

/// Runs the command `id`.
///
/// # Errors
///
/// The portal is off, or the arguments are wrong.
pub fn run(core: &Arc<Core>, id: &str, args: &Value) -> Result<Value, CommandError> {
    match id {
        TOGGLE => {
            let on = match args.get("enabled") {
                None | Some(Value::Null) => app::current().is_none(),
                Some(Value::Bool(on)) => *on,
                Some(_) => {
                    return Err(CommandError::InvalidArgs(
                        "enabled is true or false".to_owned(),
                    ));
                }
            };
            if on {
                show_pairing(core)
            } else {
                app::stop(core);
                Ok(json!({ "running": false }))
            }
        }
        SHOW_QR => show_pairing(core),
        DEVICES => {
            open_panel(core);
            Ok(Value::Null)
        }
        DISCONNECT => {
            let device = match args.get("device") {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    value
                        .as_u64()
                        .and_then(|id| u32::try_from(id).ok())
                        .ok_or_else(|| {
                            CommandError::InvalidArgs("device is a device id".to_owned())
                        })?,
                ),
            };
            Ok(json!({ "disconnected": disconnect(device) }))
        }
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

/// Disconnects `device`, or every device; how many.
pub fn disconnect(device: Option<u32>) -> usize {
    let Some(server) = app::current() else {
        return 0;
    };
    let ids: Vec<u32> = match device {
        Some(id) => vec![id],
        None => server
            .devices()
            .iter()
            .map(|status| status.info.id)
            .collect(),
    };
    ids.into_iter().filter(|id| server.disconnect(*id)).count()
}

/// Starts the portal if needed, opens a fresh pairing and shows it in the panel.
fn show_pairing(core: &Arc<Core>) -> Result<Value, CommandError> {
    let started = match app::start(core) {
        Ok(started) => started,
        Err(error) => {
            let (key, setting, address) = match &error {
                app::StartError::Disabled => ("portal.notice.off", ENABLED_KEY, None),
                app::StartError::Interface(interface) => {
                    let address = match interface {
                        interface::InterfaceError::NotPrivate(address) => Some(*address),
                        _ => None,
                    };
                    (interface.message_key(), INTERFACE_KEY, address)
                }
                app::StartError::Io(_) => ("portal.notice.failed", ENABLED_KEY, None),
            };
            let mut args: std::collections::BTreeMap<String, String> =
                [("error".to_owned(), error.to_string())]
                    .into_iter()
                    .collect();
            if let Some(address) = address {
                args.insert("address".to_owned(), address.to_string());
            }
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Warning,
                message_key: key.to_owned(),
                args,
                settings_key: Some(setting.to_owned()),
            }));
            return Err(CommandError::Failed(error.to_string()));
        }
    };
    app::new_pairing(&started.server).map_err(|error| CommandError::Failed(error.to_string()))?;
    open_panel(core);
    Ok(json!({ "running": true, "address": started.server.origin() }))
}

fn open_panel(core: &Core) {
    core.ui()
        .open_settings_window(crate::shell::page_url(ROUTE));
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "log_tests.rs"]
mod log_tests;
