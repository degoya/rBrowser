//! The status bar (RB-080-15): every message of the app - the core's notifications and what the
//! shell reports itself - runs through one channel, [`Core::notify`], into the window's
//! [`StatusLog`] and from there to the bar at the bottom of the main window, its history above
//! it and MCP. The toolbar shows no message text any more.
//!
//! | Part | Where |
//! | --- | --- |
//! | The log, the check of a shell's report | [`log`] |
//! | Commands `status.toggle_bar` (`Leader P S`), `status.history` (`Leader P L`) | [`defs`] |
//! | MCP `status_messages` | [`mcp`] |
//! | Settings `ui.statusBar.*` | `rb_config::StatusBarSettings` |
//! | The bar's view, the history popup's place | `cef_app/status_bar.rs`, `crate::page_window` |
//! | The bar and the history | `ui/src/statusbar/` |

pub mod defs;
pub mod log;
pub mod mcp;

use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore};

use crate::control::Core;
use crate::ipc::Failure;
use crate::page_window::PageWindow;

pub use defs::{HISTORY, TOGGLE_BAR, definitions, register};
pub use log::{StatusLog, checked_report, now_ms};
pub use mcp::STATUS_MESSAGES_TOOL;

/// Answers the status bar's messages: `getStatusMessages`, `reportStatus` and `dismissHistory`.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    match message {
        ShellToCore::GetStatusMessages => Some(Ok(CoreToShell::StatusMessages {
            items: core.status_log().items(),
        })),
        ShellToCore::ReportStatus(report) => Some(match checked_report(report) {
            Ok(report) => {
                // The Videos panel's playback reports also go to the log (RB-110-12).
                crate::video::diagnose::log(&report);
                core.notify(CoreToShell::Notification(report));
                Ok(CoreToShell::Ack)
            }
            Err(why) => Err((IpcErrorCode::BadRequest, why)),
        }),
        ShellToCore::DismissHistory => {
            core.ui().hide_page_window(PageWindow::History);
            Some(Ok(CoreToShell::Ack))
        }
        _ => None,
    }
}

/// The user settings changed: the log keeps `ui.statusBar.historySize` messages, and the window
/// shows or hides the bar when `ui.statusBar.visible` changed.
pub fn on_user_settings_changed(core: &Core) {
    let bar = core.user_settings().ui.status_bar;
    core.status_log().set_capacity(bar.history_capacity());
    if core.status_log().set_shown(bar.visible) {
        core.ui().set_status_bar_visible(bar.visible);
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
