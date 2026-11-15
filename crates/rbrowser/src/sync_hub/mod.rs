//! The SyncHub in the app (RB-020-02): [`HostSync`] runs `rb_sync::SyncHub` for one window. It
//! routes the sync agents' events of the active view to the other views, has the followers of a
//! leader's submit load its URL, keeps each agent's configuration in step with the active view
//! and the switches, and publishes the sync state - switches, excluded and desynced views - into
//! the shell's state, where each cell's label strip draws the desync badge in the grid gap
//! (`ui/src/sync/DesyncBadge.vue`).
//!
//! | Part | Where |
//! | --- | --- |
//! | Routing, leader, desync (no runtime) | `rb_sync::Hub` |
//! | Queues per view, desync timeout | `rb_sync::SyncHub` |
//! | The engines' replay (RB-020-03 implements it; [`HostSync::set_replay`]) | `rb_sync::Replay` |
//! | One leader per session (RB-020-07; [`HostSync::set_submit_policy`]) | `rb_sync::SubmitPolicy` |
//! | Commands `sync.*`, IPC `setSync` | [`commands`] |
//! | MCP `sync_set` (RB-030-01 registers it) | [`mcp`] |

pub mod commands;
mod glue;
pub mod mcp;

use std::sync::{Arc, Mutex, Weak};

use rb_config::SyncConfig;
use rb_engine::{ViewHost, ViewId};
use rb_sync::{AgentConfig, SyncSwitches};

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

pub use commands::{ipc, run_command};
pub use glue::HostSync;

/// `Leader S S`: sync on or off (`{ "enabled"?: bool }`, a flip without).
pub const TOGGLE: &str = "sync.toggle";
/// `Leader S N`: the next click is not synced.
pub const SKIP_NEXT: &str = "sync.skip_next";
/// `Leader S R`: every desynced view loads the leader's URL (`{ "view"?: id }` for one; the
/// badge's click).
pub const RESYNC: &str = "sync.resync";
/// `Leader S X`: the active view (`{ "view"?: id, "enabled"?: bool }`) out of sync or back in.
pub const TOGGLE_VIEW: &str = "sync.toggle_view";

/// The hub of every live window, by host.
fn registry() -> &'static Mutex<Vec<(Weak<Host>, Arc<HostSync>)>> {
    static REGISTRY: Mutex<Vec<(Weak<Host>, Arc<HostSync>)>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The hub of `host`'s window, once [`on_start`] ran for it.
pub fn for_host(host: &Host) -> Option<Arc<HostSync>> {
    let mut entries = lock(registry());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), host))
        .map(|(_, sync)| sync.clone())
}

/// Starts the hub of `core`'s window.
pub fn on_start(core: &Core) {
    if for_host(core.host()).is_some() {
        return;
    }
    let sync = HostSync::start(core.host(), core.runtime());
    lock(registry()).push((Arc::downgrade(core.host()), sync));
}

/// The project's `sync.*` changed: the switches, the timeout and the agents follow.
pub fn on_config_changed(core: &Core) {
    if let Some(sync) = for_host(core.host()) {
        sync.config_changed();
    }
}

/// The configuration of `view`'s agent: what [`HostSync`] wants, or - before it runs - the
/// project's switches with only the active view capturing.
pub fn agent_config_for(host: &Host, view: ViewId) -> AgentConfig {
    let sync = host.config().sync;
    match for_host(host) {
        Some(hub) => hub.agent_config(&sync, view),
        None => crate::sync_agent::agent_config(&sync, host.active_view() == Some(view)),
    }
}

/// The switches the project's `sync.*` start with (sync on).
pub fn switches_from(sync: &SyncConfig) -> SyncSwitches {
    SyncSwitches {
        enabled: true,
        navigation: sync.navigation,
        scroll: sync.scroll,
        click: sync.click,
        input: sync.input,
        keyboard: sync.keyboard,
        hover: sync.hover,
        forms: sync.forms,
        focus: sync.focus,
        leader_only_submit: sync.leader_only_submit,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
pub(crate) mod tests;
