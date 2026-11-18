//! Self-update in the app (RB-090-02, switched on with RB-090-14).
//!
//! Two switches: the Cargo feature `self-update` of this crate (on by default; without it
//! `rb-update` is not even linked, so the binary holds no update code and no update address) and
//! the protected setting `updates.enabled` (on by default). Without the feature the settings
//! window shows General > Updates with every update key locked and the hint [`INACTIVE_KEY`], a
//! write to one of [`LOCKED_KEYS`] is refused ([`is_locked`]), and both commands answer that this
//! build carries no self-update.
//!
//! With the feature (`active.rs`, `flow.rs`, `state.rs`): before the window a start ends or takes
//! back an interrupted update and takes its share of the instance lock ([`before_window`]); the
//! core proves a switched update and, with `updates.check` at `onStart`, checks the channel
//! ([`on_start`]); an offer is a notice with its button and the answer of `update_status`; "Install
//! and restart" downloads, verifies, hands over to `rbrowser-updater` and closes the window; and
//! with `updates.autoInstall` at `onQuit` quitting hands over a ready download ([`on_quit`]).
//!
//! Commands `update.check` (`Leader , U`) and `update.install` (`Leader , I`); MCP
//! `update_status`, `update_check` and `update_install` (`mcp.rs`). MCP never installs: its
//! install puts the offer with its button before the person, and only their click runs
//! `update.install` ([`refuse_mcp`] keeps the command itself from MCP).

use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::{UpdateCheck, UserSettings};
use serde_json::Value;

use crate::control::Core;

#[cfg(feature = "self-update")]
mod active;
#[cfg(feature = "self-update")]
mod check;
#[cfg(feature = "self-update")]
mod flow;
pub mod mcp;
#[cfg(feature = "self-update")]
mod notices;
#[cfg(feature = "self-update")]
mod state;

/// Whether this build carries self-update: the Cargo feature `self-update`.
pub const SELF_UPDATE_BUILT: bool = cfg!(feature = "self-update");

/// The keys the window locks and a write refuses while the feature is not built.
pub const LOCKED_KEYS: &[&str] = &[
    "updates.enabled",
    "updates.channel",
    "updates.check",
    "updates.autoInstall",
];

/// The hint under a locked key, and the reason a write to one is refused.
pub const INACTIVE_KEY: &str = "settings.updates.inactive";

/// `update.check`: look for an update now.
pub const CHECK_COMMAND: &str = "update.check";
/// `update.install`: install the offered update and restart (or, for a package manager's
/// installation, show its command; for an unknown one, the download).
pub const INSTALL_COMMAND: &str = "update.install";

/// Whether `key` is locked in this build.
#[must_use]
pub fn is_locked(key: &str) -> bool {
    locked_in(SELF_UPDATE_BUILT, key)
}

/// [`is_locked`] for a build with or without the feature.
#[must_use]
pub fn locked_in(built: bool, key: &str) -> bool {
    !built && LOCKED_KEYS.contains(&key)
}

/// The gate `rb-update` checks before it reaches the network: open only in a build with the
/// feature and with `updates.enabled` on.
#[must_use]
pub fn gate_open(built: bool, settings: &UserSettings) -> bool {
    built && settings.updates.enabled
}

/// Whether updates are checked by themselves: then the notice about an old CEF (RB-010-10) stays
/// silent - the next release brings the new one; with `updates.check` off it speaks.
#[must_use]
pub fn checks_by_itself(built: bool, settings: &UserSettings) -> bool {
    gate_open(built, settings) && settings.updates.check != UpdateCheck::Off
}

/// How this installation updates itself, as Settings > About names it.
#[must_use]
pub fn about_state(built: bool, settings: &UserSettings) -> rb_ipc::AboutUpdates {
    use rb_ipc::AboutUpdates;
    if !built {
        AboutUpdates::NotBuilt
    } else if !settings.updates.enabled {
        AboutUpdates::Off
    } else if settings.updates.check == UpdateCheck::Off {
        AboutUpdates::Manual
    } else {
        AboutUpdates::OnStart
    }
}

/// Refuses the command `id` to an MCP client when it is [`INSTALL_COMMAND`]: installing is a
/// person's click; `update_install` asks them.
///
/// # Errors
///
/// [`CommandError::Failed`] for [`INSTALL_COMMAND`].
pub fn refuse_mcp(id: &str) -> Result<(), CommandError> {
    if id == INSTALL_COMMAND {
        return Err(CommandError::Failed(format!(
            "{INSTALL_COMMAND} is a person's click; MCP asks with {}",
            mcp::INSTALL_TOOL
        )));
    }
    Ok(())
}

/// The command definitions: `Leader , U` and `Leader , I`.
///
/// # Errors
///
/// A malformed id, which only a broken build meets.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    [
        (CHECK_COMMAND, 'U', mcp::CHECK_TOOL),
        (INSTALL_COMMAND, 'I', mcp::INSTALL_TOOL),
    ]
    .into_iter()
    .map(|(id, key, tool)| {
        Ok(CommandDef {
            id: CommandId::new(id)?,
            title_key: format!("commands.{id}"),
            group: Group::Settings,
            default_bindings: vec![Binding::Leader(Chord {
                keys: vec![Key::Char(Group::Settings.leader_key()), Key::Char(key)],
            })],
            mcp: McpCoverage::Covered(tool.to_owned()),
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
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        run(&core, &id)
    })
}

#[cfg(feature = "self-update")]
fn run(core: &Core, id: &str) -> Result<Value, CommandError> {
    match id {
        CHECK_COMMAND => Ok(check::check_in_background(core)),
        INSTALL_COMMAND => Ok(flow::install(core)),
        other => Err(CommandError::UnknownCommand(other.to_owned())),
    }
}

#[cfg(not(feature = "self-update"))]
fn run(_core: &Core, id: &str) -> Result<Value, CommandError> {
    Err(CommandError::Failed(format!(
        "{id}: this build carries no self-update"
    )))
}

/// Before the window opens: ends or takes back an interrupted update and takes this window's
/// share of the instance lock. `Some(code)` ends the process with it - the older version was put
/// back and runs instead, or the program folder cannot be trusted to hold one version.
#[must_use]
pub fn before_window() -> Option<i32> {
    #[cfg(feature = "self-update")]
    {
        active::before_window()
    }
    #[cfg(not(feature = "self-update"))]
    None
}

/// The core is up: with the feature the switched update is proven ([`start_work`]); with
/// `updates.enabled` on and `updates.check = onStart` the channel is checked. Without the
/// feature nothing at all - no request, no file, nothing spawned.
pub fn on_start(core: &Core) {
    #[cfg(feature = "self-update")]
    {
        let (proves, checks) = start_work(SELF_UPDATE_BUILT, &core.user_settings());
        if proves {
            active::on_start(core, checks);
        }
    }
    #[cfg(not(feature = "self-update"))]
    let _ = core;
}

/// What the start does: whether it proves a switched update (always in a build with the
/// feature: a version switched at quit and started with updates switched off since must not be
/// taken back at its second start), and whether it checks the channel (`updates.enabled` and
/// `updates.check = onStart`).
#[must_use]
pub fn start_work(built: bool, settings: &UserSettings) -> (bool, bool) {
    (
        built,
        gate_open(built, settings) && settings.updates.check == UpdateCheck::OnStart,
    )
}

/// The message loop ended: with `updates.autoInstall` at `onQuit`, a ready download is handed to
/// the updater, which switches once this process is gone. Never waits for a download.
pub fn on_quit() {
    #[cfg(feature = "self-update")]
    flow::on_quit();
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
