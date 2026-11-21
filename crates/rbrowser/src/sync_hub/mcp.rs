//! MCP's `sync_set` (RB-030-01 registers it): the arguments, the answer and the function behind
//! it, plus the desync state `views_list` shows per view. The commands `sync.toggle`,
//! `sync.skip_next`, `sync.resync`, `sync.toggle_view` and `sync.toggle_focus` (RB-020-05) are
//! `covered(sync_set)`.

use rb_commands::CommandError;
use rb_config::ScrollMode;
use rb_engine::{Target, ViewId};
use rb_ipc::{SyncFeature, SyncState};
use rb_sync::{SyncKind, ViewDesync};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::commands::{kind_of, resync};
use super::for_host;
use crate::control::Core;
use crate::host::Host;

/// The tool's name.
pub const SYNC_SET_TOOL: &str = "sync_set";
/// The tool's description for the MCP tool list.
pub const SYNC_SET_DESCRIPTION: &str = "Switch the synchronisation between the views: \
     `action: \"set\"` with `enabled` turns sync on or off as a whole (`target: \"all\"`), one \
     kind of it (`feature`: navigation, scroll, click, input, keyboard, hover, forms - the \
     state of form controls, focus - the focused field) or one view (`target: <viewId>`, \
     excluded from sync while off; with `feature: \"navigation\"` only its navigation, which \
     then neither leads nor follows); `action: \"resync\"` has a desynced view (or every one, \
     `target: \"all\"`) load the active view's URL; `action: \"skipNextClick\"` leaves the next \
     click unsynced. `scrollMode` (\"anchor\" or \"wheel\") switches how scroll sync moves the \
     views until the project's settings change. Answers the sync state: the switches, the \
     excluded and the desynced views with the reason and the selector that failed, and whether \
     the next click is skipped.";

/// What `sync_set` does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncAction {
    /// Switch sync, a kind of it or a view on or off (`enabled`).
    #[default]
    Set,
    /// Load the active view's URL in the target view(s); their desync badge goes.
    Resync,
    /// Do not sync the next click.
    SkipNextClick,
}

/// The arguments of `sync_set`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncSetArgs {
    /// `"all"` (absent) or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// What to do; `set` when absent.
    #[serde(default)]
    pub action: SyncAction,
    /// `set` on `"all"`: the kind to switch; sync as a whole when absent or `all`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<SyncFeature>,
    /// `set`: on or off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// How scroll sync moves the views from now on (RB-020-04); with it, `set` needs no
    /// `enabled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_mode: Option<ScrollMode>,
}

/// Runs `sync_set` with `args` (its JSON); answers the sync state.
pub fn sync_set(core: &Core, args: &Value) -> Result<SyncState, CommandError> {
    let args: SyncSetArgs = if args.is_null() {
        SyncSetArgs::default()
    } else {
        serde_json::from_value(args.clone())
            .map_err(|error| CommandError::InvalidArgs(error.to_string()))?
    };
    let host = core.host();
    let sync =
        for_host(host).ok_or_else(|| CommandError::Failed("sync is not started".to_owned()))?;
    let view = match args.target {
        None | Some(Target::All) => None,
        Some(Target::View(view)) => Some(view),
    };
    match args.action {
        SyncAction::Set if args.enabled.is_none() && args.scroll_mode.is_some() => {}
        SyncAction::Set => {
            let enabled = args
                .enabled
                .ok_or_else(|| CommandError::InvalidArgs("`set` needs `enabled`".to_owned()))?;
            let kind = args.feature.and_then(kind_of);
            match (view, kind) {
                (None, None) => sync.set_enabled(enabled),
                (None, Some(kind)) => sync.set_kind(kind, enabled),
                (Some(view), None) => sync
                    .exclude(view, !enabled)
                    .map_err(|error| CommandError::InvalidArgs(error.to_string()))?,
                // RB-020-06: one view's navigation decoupled or coupled again.
                (Some(view), Some(SyncKind::Navigation)) => {
                    crate::nav_sync::couple(host, view, enabled)
                        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
                }
                (Some(_), Some(_)) => {
                    return Err(CommandError::InvalidArgs(
                        "a `feature` is switched for every view, not one".to_owned(),
                    ));
                }
            }
        }
        SyncAction::Resync => {
            resync(&sync, view)?;
        }
        SyncAction::SkipNextClick => sync.skip_next_click(),
    }
    if let Some(mode) = args.scroll_mode {
        crate::scroll_sync::set_mode(host, Some(mode));
        sync.scroll_mode_changed();
        crate::engines::webkit::on_scroll_mode_changed(core);
    }
    current_state(core)
}

/// The sync state `sync_set` answers. MCP reads it again once the change is done
/// ([`super::HostSync::settled`]).
pub fn current_state(core: &Core) -> Result<SyncState, CommandError> {
    let host = core.host();
    let sync =
        for_host(host).ok_or_else(|| CommandError::Failed("sync is not started".to_owned()))?;
    let mut state = sync.state(&host.with_config(|config| config.sync.clone()));
    state.features.scroll_mode = crate::scroll_sync::mode(host);
    Ok(state)
}

/// `view`'s desync for `views_list`; `None` while it is in sync.
pub fn view_desync(host: &Host, view: ViewId) -> Option<ViewDesync> {
    let sync = for_host(host)?;
    sync.state(&host.with_config(|config| config.sync.clone()))
        .desynced
        .into_iter()
        .find(|desync| desync.view == view)
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
