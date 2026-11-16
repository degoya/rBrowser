//! Input replay for Chromium views (RB-020-03): the SyncHub's `rb_sync::Replay` over CDP. A
//! click, a key, typed text or a hover of the active view reaches every other view as **real
//! input** (`Input.dispatchMouseEvent`, `Input.dispatchKeyEvent`, `Input.insertText`,
//! `Input.dispatchTouchEvent`): trusted events, with `:hover`, focus and default actions.
//!
//! | Part | Where |
//! | --- | --- |
//! | Event to action, per view (touch, `sync.contextMenu`) | [`plan`] |
//! | Keys to `Input.dispatchKeyEvent`, MCP's named keys | [`keys`] |
//! | Points (visual viewport, emulation scale) and the `Input.*` commands | [`input`] |
//! | Echo marker, submit guard and resolution in one round trip | [`prologue`] |
//! | The replay itself, `rb_sync::Replay` | [`CdpReplay`] |
//! | The window it runs against | [`ReplayPort`], [`HostPort`] |
//! | The debug metric (K1) | [`metrics`] |
//! | MCP's `click`, `type`, `hover`, `press_key` (RB-030-01 registers them) | [`mcp`] |
//! | `scroll` and `wheel` events (scroll sync, RB-020-04) | `crate::scroll_sync` |
//!
//! The protocol and the rules are `docs/sync-protocol.md` ("Input replay").

mod engine;
pub mod input;
pub mod keys;
pub mod mcp;
pub mod metrics;
pub mod plan;
mod port;
pub mod prologue;

use std::sync::Arc;

use crate::control::Core;
use crate::host::Host;

pub use engine::CdpReplay;
pub use port::{HostPort, ReplayPort, ViewTraits};

impl CdpReplay {
    /// The replay of `host`'s window.
    pub fn for_host(host: &Arc<Host>) -> Self {
        Self::new(Arc::new(HostPort::new(host)))
    }
}

/// The SyncHub of `core`'s window replays through CDP from now on (after `sync_hub::on_start`).
pub fn on_start(core: &Core) {
    match crate::sync_hub::for_host(core.host()) {
        Some(sync) => sync.set_replay(Arc::new(CdpReplay::for_host(core.host()))),
        None => tracing::warn!("no SyncHub: input is not replayed"),
    }
}

#[cfg(test)]
pub(crate) mod fake;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
