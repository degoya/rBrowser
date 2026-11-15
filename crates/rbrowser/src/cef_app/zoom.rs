//! The shell's pages never zoom (RB-080-50). Chromium keeps a page's zoom per host and across
//! restarts, so a level set once - by an earlier version, where Ctrl + wheel over the canvas
//! board zoomed the toolbar with it - would come back with every start. It is put back to none
//! when a page of the shell has loaded ([`reset`]): once, not on every request. Nothing sets a
//! level after that: the pages take the wheel (`ui/src/zoomGuard.ts`) and their keyboard
//! handler takes Ctrl with Plus, Minus and 0 (`keys.rs`, [`crate::shell::is_zoom_key`]). All
//! pages of the shell share the host `rbrowser://ui`, so the shell view's load covers the
//! status bar and the page windows too; the canvas board resets on its own load as well.

use cef::*;

use super::views::core;

/// Puts the zoom of `browser`, a page of the shell that just loaded, back to none.
pub fn reset(browser: Option<&mut Browser>) {
    let Some(host) = browser.and_then(|browser| browser.host()) else {
        return;
    };
    let level = host.zoom_level();
    if crate::shell::is_zoomed(level) {
        tracing::info!(level, "a page of the shell was zoomed: back to none");
        host.set_zoom_level(0.0);
    }
}

/// The shell view has loaded: its zoom is none, and what waited for the shell is shown.
pub fn shell_loaded(browser: Option<&mut Browser>) {
    reset(browser);
    if let Some(core) = core() {
        core.shell_ready();
    }
}
