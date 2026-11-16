//! The toolbar in a narrow window (RB-080-52). The shell page fits its row itself; two things
//! are the core's:
//!
//! - **The rows.** With `ui.toolbar.overflow = wrap` the page asks for a second row
//!   (`toolbarRows`): the shell view becomes [`height`] DIP high and the window's box moves the
//!   views down ([`crate::host::ui::UiBackend::relayout_shell`]). The page draws its second row
//!   only after the `ack`, so it never draws below its view. A request for more rows than the
//!   setting allows is refused.
//! - **The command** `ui.toolbar.overflow.open` (`Leader P T`): sent to the shell as
//!   `uiCommand`, where the overflow menu takes the focus and opens. No tool: the menu is a
//!   display, and every entry in it is a command with a tool of its own.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_config::ToolbarOverflow;
use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore};
use serde_json::Value;

use crate::control::Core;
use crate::ipc::Failure;
use crate::shell::SHELL_HEIGHT;

/// Open the toolbar's overflow menu.
pub const OPEN_MENU: &str = "ui.toolbar.overflow.open";
/// Why `ui.toolbar.overflow.open` has no tool.
pub const OPEN_OMITTED: &str = "display only; every entry of the menu is a command with its tool";
/// The key after `P`: T for toolbar.
const OPEN_KEY: char = 'T';

/// The rows the shell view has room for.
static ROWS: AtomicU32 = AtomicU32::new(1);

/// The rows the toolbar has: 1, or 2 while it wraps.
#[must_use]
pub fn rows() -> u32 {
    ROWS.load(Ordering::Acquire)
}

/// The shell view's height in DIP: [`SHELL_HEIGHT`] per row at the text size in effect
/// (RB-090-26, [`crate::text_scale`]).
#[must_use]
pub fn height() -> u32 {
    crate::text_scale::dip(SHELL_HEIGHT).saturating_mul(rows())
}

/// The rows a `toolbarRows` request gets under `overflow`.
///
/// # Errors
///
/// `rows` is 0 or more than the setting allows.
pub fn checked_rows(rows: u8, overflow: ToolbarOverflow) -> Result<u32, String> {
    let rows = u32::from(rows);
    if rows == 0 || rows > overflow.max_rows() {
        return Err(format!(
            "the toolbar has 1 to {} rows, not {rows}",
            overflow.max_rows()
        ));
    }
    Ok(rows)
}

/// Answers `toolbarRows`; `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::ToolbarRows { rows } = message else {
        return None;
    };
    let overflow = core.user_settings().ui.toolbar.overflow;
    Some(match checked_rows(*rows, overflow) {
        Ok(rows) => {
            if ROWS.swap(rows, Ordering::AcqRel) != rows {
                core.ui().relayout_shell();
            }
            Ok(CoreToShell::Ack)
        }
        Err(reason) => Err((IpcErrorCode::Refused, reason)),
    })
}

/// The definition; title key `commands.<id>`.
///
/// # Errors
///
/// An id the registry would refuse (a test holds it valid).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(OPEN_MENU)?,
        title_key: format!("commands.{OPEN_MENU}"),
        group: Group::Panels,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(Group::Panels.leader_key()), Key::Char(OPEN_KEY)],
        })],
        mcp: McpCoverage::Omitted(OPEN_OMITTED.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        // The menu lives in the shell page; it needs the keyboard to be used.
        core.ui().focus_shell();
        core.send(&CoreToShell::UiCommand {
            id: OPEN_MENU.to_owned(),
        });
        Ok(Value::Null)
    })
}

#[cfg(test)]
#[path = "toolbar_rows_tests.rs"]
mod tests;
