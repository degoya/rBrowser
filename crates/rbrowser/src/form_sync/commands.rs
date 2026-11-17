//! Form sync's command and the views' context menu entry (RB-020-05).
//!
//! | Command | Chord | MCP |
//! | --- | --- | --- |
//! | `sync.toggle_focus` (`{ enabled? }`, a flip without) | `Ctrl+Shift+Space S F` | `sync_set { feature: "focus" }` |
//! | `sync.skip_next` (RB-020-02; also the context menu entry) | `Ctrl+Shift+Space S N` | `sync_set { action: "skipNextClick" }` |

use rb_commands::CommandError;
use rb_engine::{ViewHost, ViewId};
use rb_sync::SyncKind;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::control::Core;
use crate::sync_hub::{SKIP_NEXT, for_host};

/// `Leader S F`: focus sync on or off.
pub const TOGGLE_FOCUS: &str = "sync.toggle_focus";
/// The views' context menu entry that runs `sync.skip_next` for its view.
pub const SKIP_NEXT_MENU_KEY: &str = "sync.menu.skipNextClick";

/// The arguments of `sync.toggle_focus`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToggleArgs {
    /// On or off; absent flips.
    enabled: Option<bool>,
}

/// Runs `id` when it is form sync's command; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if id != TOGGLE_FOCUS {
        return None;
    }
    let Some(sync) = for_host(core.host()) else {
        return Some(Err(CommandError::Failed("sync is not started".to_owned())));
    };
    let args = if args.is_null() {
        ToggleArgs::default()
    } else {
        match serde_json::from_value::<ToggleArgs>(args.clone()) {
            Ok(args) => args,
            Err(error) => return Some(Err(CommandError::InvalidArgs(error.to_string()))),
        }
    };
    let on = args
        .enabled
        .unwrap_or_else(|| !sync.hub().with(|hub| hub.switches().focus));
    sync.set_kind(SyncKind::Focus, on);
    Some(Ok(json!({ "focus": on })))
}

/// The context menu entry of `view`: it becomes the active view, and its next click is not
/// synced (`sync.skip_next`).
pub fn skip_next_click_in(core: &Core, view: ViewId) -> Result<(), CommandError> {
    core.host()
        .set_active_view(view)
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    core.run_command(SKIP_NEXT, Value::Null).map(|_| ())
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
