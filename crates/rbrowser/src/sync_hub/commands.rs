//! The commands `sync.*` (group `S`) and the shell's `setSync`.

use std::sync::Arc;

use rb_commands::CommandError;
use rb_engine::{ViewHost, ViewId};
use rb_ipc::{CoreToShell, ShellToCore, SyncFeature};
use rb_sync::SyncKind;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{HostSync, RESYNC, SKIP_NEXT, TOGGLE, TOGGLE_VIEW, for_host};
use crate::control::Core;
use crate::ipc::Failure;

/// The arguments of `sync.toggle`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToggleArgs {
    /// On or off; absent flips.
    enabled: Option<bool>,
}

/// The arguments of `sync.resync` and `sync.toggle_view`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewArgs {
    /// The view; absent: every desynced view (`resync`), the active view (`toggle_view`).
    view: Option<ViewId>,
    /// `toggle_view`: in sync or not; absent flips.
    enabled: Option<bool>,
}

/// Runs the command `id` when it is a `sync.*` command; `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    if ![TOGGLE, SKIP_NEXT, RESYNC, TOGGLE_VIEW].contains(&id) {
        return None;
    }
    let Some(sync) = for_host(core.host()) else {
        return Some(Err(CommandError::Failed("sync is not started".to_owned())));
    };
    Some(match id {
        TOGGLE => parse::<ToggleArgs>(args).map(|args| toggle(&sync, args.enabled)),
        SKIP_NEXT => {
            sync.skip_next_click();
            Ok(Value::Null)
        }
        RESYNC => parse::<ViewArgs>(args).and_then(|args| resync(&sync, args.view)),
        _ => parse::<ViewArgs>(args).and_then(|args| toggle_view(core, &sync, &args)),
    })
}

/// Answers the shell's `setSync`; `None` for any other message or before the hub runs.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::SetSync { feature, enabled } = message else {
        return None;
    };
    let sync = for_host(core.host())?;
    match kind_of(*feature) {
        None => sync.set_enabled(*enabled),
        Some(kind) => sync.set_kind(kind, *enabled),
    }
    Some(Ok(CoreToShell::Ack))
}

/// The kind `feature` switches; `None` for sync as a whole.
pub fn kind_of(feature: SyncFeature) -> Option<SyncKind> {
    match feature {
        SyncFeature::All => None,
        SyncFeature::Navigation => Some(SyncKind::Navigation),
        SyncFeature::Scroll => Some(SyncKind::Scroll),
        SyncFeature::Click => Some(SyncKind::Click),
        SyncFeature::Input => Some(SyncKind::Input),
        SyncFeature::Keyboard => Some(SyncKind::Keyboard),
        SyncFeature::Hover => Some(SyncKind::Hover),
        SyncFeature::Forms => Some(SyncKind::Forms),
        SyncFeature::Focus => Some(SyncKind::Focus),
    }
}

fn parse<T: DeserializeOwned + Default>(args: &Value) -> Result<T, CommandError> {
    if args.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

fn toggle(sync: &HostSync, enabled: Option<bool>) -> Value {
    let on = enabled.unwrap_or_else(|| !sync.hub().with(|hub| hub.switches().enabled));
    sync.set_enabled(on);
    json!({ "enabled": on })
}

/// Resyncs `view`, or every desynced view; answers the views resynced.
pub(crate) fn resync(sync: &Arc<HostSync>, view: Option<ViewId>) -> Result<Value, CommandError> {
    let views = match view {
        Some(view) => vec![view],
        None => sync.hub().with(|hub| hub.desynced()),
    };
    for view in &views {
        sync.resync(*view)
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    }
    Ok(json!({ "views": views }))
}

fn toggle_view(core: &Core, sync: &HostSync, args: &ViewArgs) -> Result<Value, CommandError> {
    let view = args
        .view
        .or_else(|| core.host().active_view())
        .ok_or_else(|| CommandError::Failed("there is no active view".to_owned()))?;
    let excluded = match args.enabled {
        Some(enabled) => !enabled,
        None => !sync.hub().with(|hub| hub.is_excluded(view)),
    };
    sync.exclude(view, excluded)
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    Ok(json!({ "view": view, "enabled": !excluded }))
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
