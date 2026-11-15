//! What macOS itself draws of rBrowser's theme (RB-090-15; the Windows twin is
//! `../theme_dwm.rs`): whether the system uses the dark appearance, which resolves the theme
//! `system`. The title bar follows the system's appearance by itself; drawing it in the
//! theme's own mode is not on macOS yet. The colours of the views are `theme.rs`'. UI thread.

use cef::*;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

/// The title bar of `window` in the theme's mode: macOS draws it in the system's.
pub fn set_dark_title_bar(_window: &Window, _dark: bool) {}

/// Whether the app's effective appearance (the system's, as nothing overrides it) is a dark
/// one (`NSAppearanceNameDarkAqua` or its high-contrast and vibrant kin); light off the main
/// thread.
pub fn system_dark() -> bool {
    let Some(main_thread) = MainThreadMarker::new() else {
        return false;
    };
    let appearance = NSApplication::sharedApplication(main_thread).effectiveAppearance();
    appearance.name().to_string().contains("Dark")
}
