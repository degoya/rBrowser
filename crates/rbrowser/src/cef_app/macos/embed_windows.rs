//! The native windows of the external views (Firefox, WebKit) on macOS (RB-090-15; the Windows
//! twin is `../embed_windows.rs`, Win32 calls). Embedding another process's window in a grid
//! cell is not on macOS yet: no window is ever found, so `embed.rs` binds none and the engine's
//! own window stays where its engine put it. Every call is a no-op with the Win32 shape.

use rb_engine::external::{Candidate, WindowRect, WindowStyle};

/// The visible top-level windows of `process_ids`: none on macOS yet.
pub fn top_level_windows_of(_process_ids: &[u32], _class: Option<&str>) -> Vec<Candidate> {
    Vec::new()
}

/// The child windows' rectangles: none.
pub fn child_rects(_handle: usize) -> Vec<WindowRect> {
    Vec::new()
}

/// Takes the menu bar off a window: there is none to take.
pub fn remove_menu(_handle: usize) -> usize {
    0
}

/// Gives a window its menu bar back: nothing to give.
pub fn set_menu(_handle: usize, _menu: usize) {}

/// Whether `handle` names a live window: never, as no window is bound.
pub fn is_window(_handle: usize) -> bool {
    false
}

/// The window's styles: none.
pub fn style(_handle: usize) -> WindowStyle {
    WindowStyle {
        style: 0,
        ex_style: 0,
    }
}

/// Sets the window's styles: nothing to set.
pub fn set_style(_handle: usize, _style: WindowStyle) {}

/// Sets the window's owner: nothing to set.
pub fn set_owner(_handle: usize, _owner: usize) {}

/// Tells the window its frame changed: nothing to tell.
pub fn frame_changed(_handle: usize) {}

/// Clips the window: nothing to clip.
pub fn set_region(_handle: usize, _region: Option<WindowRect>) {}

/// Moves the window: nothing to move.
pub fn place(_handle: usize, _rect: WindowRect) {}

/// Hides the window: nothing to hide.
pub fn hide(_handle: usize) {}

/// Shows the window: nothing to show.
pub fn show(_handle: usize) {}
