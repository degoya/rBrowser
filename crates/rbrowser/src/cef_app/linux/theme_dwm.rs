//! What the Linux desktop draws of rBrowser's theme (RB-097-05; the Windows twin is
//! `../theme_dwm.rs`): whether it prefers a dark theme, which resolves the theme `system`
//! (`crate::desktop_linux`). The window manager draws the title bar in its own theme; drawing it
//! in rBrowser's is not on Linux. The colours of the views are `theme.rs`'. UI thread.

use cef::*;

/// The title bar of `window` in the theme's mode: the window manager draws it in its own.
pub fn set_dark_title_bar(_window: &Window, _dark: bool) {}

/// Whether the desktop prefers a dark theme (`GTK_THEME`, GTK's settings file).
pub fn system_dark() -> bool {
    crate::desktop_linux::system_dark()
}
