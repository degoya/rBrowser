//! The right mouse button on a label strip on macOS (RB-090-15; the Windows twin is
//! `../strip_clicks.rs`, a `WH_MOUSE` hook of the UI thread). Not on macOS yet: a strip's menu
//! opens from its menu button and the view's context menu only.

/// The hook's place on macOS: nothing to install yet.
pub fn install() {
    tracing::debug!("the label strips' right-click is not on macOS yet");
}

/// The hook's place on macOS: nothing to remove.
pub fn uninstall() {}
