//! The About page (RB-010-18, after rCommander's RC-010-19): Settings > About shows which build
//! runs - version, commit, build time, platform, CEF/Chromium and the other engines, the API
//! versions, the data and log folders - with "Copy version info", the links, the author and
//! the licences of every bundled component (`ui/src/about/`).
//!
//! The core's part is the facts ([`facts`]), answered to the shell's `getAbout` and to MCP's
//! `app_info` from the same function, and the command `app.about` (`Ctrl+Shift+Space , A`),
//! which opens the settings at the About category. The licence list is static and lives with
//! the page (`scripts/licenses.sh`); MCP leaves it out (`omitted: static documents, available
//! in NOTICE`).

mod facts;
mod firefox;

use std::path::Path;

use rb_commands::CommandError;
use rb_ipc::{AboutInfo, CoreToShell, ShellToCore};
use serde_json::Value;

pub use facts::{BUILD, Build, Dirs, UNKNOWN, facts, facts_from, platform, split_cef};
pub use firefox::with_firefox;

use crate::control::Core;
use crate::ipc::Failure;

/// The command that opens Settings > About.
pub const COMMAND: &str = "app.about";
/// The settings category the command opens.
pub const CATEGORY: &str = "about";
/// The MCP tool that answers the facts (RB-030-01 registers it). It keeps the data and log
/// folders on purpose: MCP clients are authenticated local agents (threat model T6), the folders
/// help them diagnose, and the paths are no secret on the user's own machine (security review
/// W1c; threat model, residual risks).
pub const APP_INFO_TOOL: &str = "app_info";
/// The tool's description for the MCP tool list.
pub const APP_INFO_DESCRIPTION: &str = "rBrowser's version, commit, build time and platform, the \
     engine versions (CEF/Chromium, Firefox, WebKit, Safari on macOS, Node), the API versions and \
     the data and log folders - the facts of Settings > About, without the licence texts.";
/// Why the licence texts have no MCP tool.
pub const LICENCES_OMITTED: &str = "static documents, available in NOTICE";

/// Runs `app.about`: opens the settings window at About; `None` for any other command.
pub fn run_command(core: &Core, id: &str, _args: &Value) -> Option<Result<Value, CommandError>> {
    (id == COMMAND).then(|| {
        crate::settings_window::open(core, Some(CATEGORY));
        Ok(Value::Null)
    })
}

/// Answers `getAbout` with the facts and the Firefox `core`'s settings start; `None` for any
/// other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    matches!(message, ShellToCore::GetAbout).then(|| Ok(CoreToShell::About(facts_of(core))))
}

/// The facts with the Firefox `core` starts (RB-090-49): the setting, else the first
/// installation of the cache - never a registry or disk walk here, the UI thread asks - and how
/// updates come.
pub fn facts_of(core: &Core) -> AboutInfo {
    let settings = core.user_settings();
    let mut info = with_firefox(
        facts(),
        &crate::engines::firefox::installed::installed(),
        settings.engines.firefox_executable(),
        Path::is_file,
    );
    // Safari's version on a Mac, as a session or the bundle named it (RB-095-03).
    info.safari = crate::engines::safari::version();
    // How this installation updates itself (RB-091-05).
    info.updates = Some(crate::updates::about_state(
        crate::updates::SELF_UPDATE_BUILT,
        &settings,
    ));
    info
}

/// The answer to `message` when it is `getAbout`.
pub fn answer(message: &ShellToCore) -> Option<CoreToShell> {
    matches!(message, ShellToCore::GetAbout).then(|| CoreToShell::About(facts()))
}

/// MCP's `app_info` without a core (the window closed): the build's facts, as JSON.
pub fn app_info() -> Value {
    serde_json::to_value(facts()).unwrap_or(Value::Null)
}

/// MCP's `app_info`: the facts the About page shows, Firefox included, as JSON.
pub fn app_info_of(core: &Core) -> Value {
    serde_json::to_value(facts_of(core)).unwrap_or(Value::Null)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
