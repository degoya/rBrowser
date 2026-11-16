//! The command `sync.toggle_scroll` (`Leader S C`): scroll sync on or off, click sync and the
//! rest untouched. MCP: `sync_set { feature: "scroll", enabled }`.

use rb_commands::CommandError;
use rb_sync::SyncKind;
use serde::Deserialize;
use serde_json::{Value, json};

use super::TOGGLE_SCROLL;
use crate::control::Core;
use crate::sync_hub::for_host;

/// The arguments of `sync.toggle_scroll`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToggleArgs {
    /// On or off; absent flips.
    enabled: Option<bool>,
}

/// Runs the command `id` when it is `sync.toggle_scroll`; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if id != TOGGLE_SCROLL {
        return None;
    }
    let Some(sync) = for_host(core.host()) else {
        return Some(Err(CommandError::Failed("sync is not started".to_owned())));
    };
    let args: ToggleArgs = if args.is_null() {
        ToggleArgs::default()
    } else {
        match serde_json::from_value(args.clone()) {
            Ok(args) => args,
            Err(error) => return Some(Err(CommandError::InvalidArgs(error.to_string()))),
        }
    };
    let on = args
        .enabled
        .unwrap_or_else(|| !sync.hub().with(|hub| hub.switches().scroll));
    sync.set_kind(SyncKind::Scroll, on);
    Some(Ok(json!({ "scroll": on })))
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
