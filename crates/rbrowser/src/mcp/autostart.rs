//! `mcp.stdioAutoStart` (RB-030-02): when an MCP client starts `rbrowser mcp` for a project
//! whose rBrowser does not run, the bridge starts it in the background (`rbrowser --project
//! <root> --background`, no console, no inherited handles - stdout belongs to JSON-RPC) and waits
//! for its MCP server. Off by default; only with the MCP server on (`mcp.enabled`), since the
//! started window would serve nothing otherwise, and only for a named project.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rb_config::McpSettings;
use rb_mcp::stdio::Locator;
use rb_process::{Detached as _, NoConsoleWindow as _};

/// How long the bridge waits for the started rBrowser's MCP server.
pub const WAIT: Duration = Duration::from_secs(30);
/// How often it looks.
const POLL: Duration = Duration::from_millis(250);

/// Whether `rbrowser mcp` starts rBrowser for `root` when none runs.
pub fn wanted(settings: &McpSettings, root: Option<&Path>) -> bool {
    settings.stdio_auto_start && settings.enabled && root.is_some()
}

/// The arguments that open `root` without taking the focus.
pub fn arguments(root: &Path) -> Vec<OsString> {
    vec![
        OsString::from("--project"),
        root.as_os_str().to_owned(),
        OsString::from("--background"),
    ]
}

/// Starts this program for `root` and waits up to [`WAIT`] until `locator` finds its server.
/// Whether it was found; a failure is logged, the bridge answers as without an instance.
pub fn start_and_wait(root: &Path, locator: &Locator) -> bool {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            tracing::warn!(%error, "rbrowser mcp cannot start rBrowser: no program path");
            return false;
        }
    };
    let mut command = Command::new(&exe);
    command
        .args(arguments(root))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .no_console_window()
        .detached(true);
    let started = match command.spawn() {
        Ok(child) => Ok(child),
        // A job that allows no breakaway: started inside it.
        Err(error) if cfg!(windows) && error.raw_os_error() == Some(5) => {
            command.detached(false);
            command.spawn()
        }
        Err(error) => Err(error),
    };
    if let Err(error) = started {
        tracing::warn!(%error, project = %root.display(), "rbrowser mcp could not start rBrowser");
        return false;
    }
    tracing::info!(project = %root.display(), "rbrowser mcp started rBrowser; waiting for its MCP server");
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        if locator.find().is_ok() {
            return true;
        }
        std::thread::sleep(POLL);
    }
    tracing::warn!(project = %root.display(), "the started rBrowser did not open its MCP server in time");
    false
}

#[cfg(test)]
#[path = "autostart_tests.rs"]
mod tests;
