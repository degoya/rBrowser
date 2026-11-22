//! A native window handle as a plain number - comparable, `Send`, kept in the embedding's and
//! the mouse hook's state: Windows' `HWND`, macOS' `NSView` pointer (RB-090-15), an X11 window
//! id on Linux (RB-097-05).

use cef::sys::cef_window_handle_t;

/// The handle as a number; 0 for none.
#[cfg(windows)]
pub fn value(handle: cef_window_handle_t) -> usize {
    handle.0 as usize
}

/// The handle as a number; 0 for none.
#[cfg(not(windows))]
pub fn value(handle: cef_window_handle_t) -> usize {
    handle as usize
}
