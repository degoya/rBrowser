//! `app.restart` (RB-094-23, `Leader , S`, the settings window's "Restart now" beside a key that
//! applies at the next start, `x-restart`): closes the window as a quit does and, once the
//! process has shut CEF down and let go of the profile, starts rBrowser again with this start's
//! arguments, so the same project opens. MCP never runs it (`omitted`): a restart ends every
//! MCP session, and it is a person's choice.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_process::{Detached, NoConsoleWindow};
use serde_json::Value;

use crate::control::Core;

/// The command.
pub const COMMAND: &str = "app.restart";
/// Its leader key in the settings group (`Leader , S`: start again).
pub const LEADER_KEY: char = 'S';
/// Why the command has no MCP tool.
pub const MCP_OMITTED: &str =
    "protected: a restart ends every MCP session; only a person restarts rBrowser";

/// A restart was asked for; the quit starts rBrowser again.
static REQUESTED: AtomicBool = AtomicBool::new(false);

/// The command definition: `Leader , S`, no MCP tool.
///
/// # Errors
///
/// A malformed id, which only a broken build meets.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(COMMAND)?,
        title_key: format!("commands.{COMMAND}"),
        group: Group::Settings,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![
                Key::Char(Group::Settings.leader_key()),
                Key::Char(LEADER_KEY),
            ],
        })],
        mcp: McpCoverage::Omitted(MCP_OMITTED.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        request(&core);
        Ok(Value::Null)
    })
}

/// Refuses the command to an MCP client.
///
/// # Errors
///
/// [`CommandError::Failed`] for [`COMMAND`].
pub fn refuse_mcp(id: &str) -> Result<(), CommandError> {
    if id == COMMAND {
        return Err(CommandError::Failed(format!("{COMMAND}: {MCP_OMITTED}")));
    }
    Ok(())
}

/// Asks for the restart and closes the window; the app quits as it always does.
pub fn request(core: &Core) {
    REQUESTED.store(true, Ordering::SeqCst);
    tracing::info!("rBrowser restarts at the person's request");
    core.ui().close_window();
}

/// Whether a restart was asked for.
#[must_use]
pub fn requested() -> bool {
    REQUESTED.load(Ordering::SeqCst)
}

/// The start of the next rBrowser: `exe` with the arguments of `args` after the program's name.
fn relaunch_command(exe: &Path, args: impl IntoIterator<Item = OsString>) -> Command {
    let mut command = Command::new(exe);
    command.args(args.into_iter().skip(1)).no_console_window();
    command
}

/// The last step of the quit, after CEF shut down and the profile is free: starts rBrowser again
/// when a restart was asked for. A start that fails is logged; the app ends either way.
pub fn relaunch_if_requested() {
    relaunch_after(requested(), crate::instance::stop_listening, start_again);
}

/// When `requested`: `stop` this process's instance listener, then `start` the next process.
/// The order matters: a listener still there takes the next start's hand-over, and the new
/// process ends without a window.
fn relaunch_after(requested: bool, stop: impl FnOnce(), start: impl FnOnce()) {
    if requested {
        stop();
        start();
    }
}

/// Starts this program again with this start's arguments.
fn start_again() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            tracing::error!(%error, "rBrowser cannot find its program to start again");
            return;
        }
    };
    let started = relaunch_command(&exe, std::env::args_os())
        .detached(true)
        .spawn()
        // A job that allows no breakaway refuses with error 5: start inside it then.
        .or_else(|_| {
            relaunch_command(&exe, std::env::args_os())
                .detached(false)
                .spawn()
        });
    if let Err(error) = started {
        tracing::error!(%error, "rBrowser did not start again");
    }
}

#[cfg(test)]
#[path = "restart_tests.rs"]
mod tests;
