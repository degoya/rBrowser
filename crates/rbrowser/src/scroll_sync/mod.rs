//! Scroll sync for Chromium views (RB-020-04). A scroll in the active view brings the same
//! **content** to the same edge in every other view - the same page is far taller in a narrow
//! view, so pixels or percentages would land elsewhere - in the document and in inner scroll
//! containers (sidebars, modals, tables), vertically and horizontally, without an echo.
//!
//! | Part | Where |
//! | --- | --- |
//! | Capture: one event per scroller and frame, anchor and offset, end position after a rest | `agent/src/scroll/{anchor,capture}.ts` |
//! | Apply in a follower: the anchor onto the edge, exact start and end, percent fallback | `agent/src/scroll/apply.ts` |
//! | Routing, `sync.scroll`, exclusion, never a desync | `rb_sync::Hub` (RB-020-02) |
//! | The replay of `scroll` (anchor mode) and `wheel` (wheel mode) events | [`replay`] |
//! | The mode at runtime (`sync.scrollMode`, `sync_set { scrollMode }`) | [`mode`], [`set_mode`] |
//! | The command `sync.toggle_scroll` (`Leader S C`) | [`commands`] |
//! | MCP's `scroll` (RB-030-01 registers it) | [`mcp`] |
//!
//! `CdpReplay` hands every `scroll` and `wheel` event to [`replay::replay`] before its own plan.
//! The protocol and the rules are `docs/sync-protocol.md` ("Scroll sync").

pub mod commands;
pub mod mcp;
pub mod replay;

use std::sync::{Arc, Mutex, Weak};

use rb_config::ScrollMode;

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;

pub use commands::run_command;

/// `Leader S C`: scroll sync on or off (`{ "enabled"?: bool }`, a flip without).
pub const TOGGLE_SCROLL: &str = "sync.toggle_scroll";

/// The mode in effect per window: the one `sync_set` set at runtime, else `sync.scrollMode` as
/// last read - so a scroll event does not copy the project config. Dropped when the project's
/// `sync.*` change.
fn modes() -> &'static Mutex<Vec<(Weak<Host>, ScrollMode)>> {
    static MODES: Mutex<Vec<(Weak<Host>, ScrollMode)>> = Mutex::new(Vec::new());
    &MODES
}

/// The scroll mode in effect in `host`'s window: the runtime one, else `sync.scrollMode`.
pub fn mode(host: &Arc<Host>) -> ScrollMode {
    let mut entries = lock(modes());
    entries.retain(|(weak, _)| weak.strong_count() > 0);
    if let Some((_, mode)) = entries
        .iter()
        .find(|(weak, _)| std::ptr::eq(weak.as_ptr(), &**host))
    {
        return *mode;
    }
    let mode = host.config().sync.scroll_mode;
    entries.push((Arc::downgrade(host), mode));
    mode
}

/// Sets the scroll mode of `host`'s window at runtime; `None` goes back to `sync.scrollMode`.
pub fn set_mode(host: &Arc<Host>, mode: Option<ScrollMode>) {
    let mut entries = lock(modes());
    entries.retain(|(weak, _)| weak.strong_count() > 0 && !std::ptr::eq(weak.as_ptr(), &**host));
    if let Some(mode) = mode {
        entries.push((Arc::downgrade(host), mode));
    }
}

/// The project's `sync.*` changed: the mode starts from `sync.scrollMode` again, as the other
/// switches start from the project (`sync_hub::on_config_changed`). Runs on the caller's thread
/// and spawns nothing.
pub fn on_config_changed(core: &Core) {
    set_mode(core.host(), None);
}

#[cfg(test)]
mod fake;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
