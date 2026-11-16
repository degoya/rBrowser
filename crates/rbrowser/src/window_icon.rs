//! The rBrowser icon for the windows CEF makes (the main window, the pages window, the page
//! windows): CEF's Views windows show Chromium's default icon unless the app sets one, whatever
//! icon the executable carries. PNGs cut from `resources/rbrowser.ico`, one per scale factor.

/// The title bar's icon in DIP.
pub const SMALL_DIP: u32 = 16;
/// The taskbar's and Alt+Tab's icon in DIP.
pub const LARGE_DIP: u32 = 32;

const PNG_16: &[u8] = include_bytes!("../resources/window-icon-16.png");
const PNG_32: &[u8] = include_bytes!("../resources/window-icon-32.png");
const PNG_64: &[u8] = include_bytes!("../resources/window-icon-64.png");

/// The title bar's icon: (scale factor, PNG).
pub const SMALL: [(f32, &[u8]); 2] = [(1.0, PNG_16), (2.0, PNG_32)];
/// The taskbar's icon: (scale factor, PNG).
pub const LARGE: [(f32, &[u8]); 2] = [(1.0, PNG_32), (2.0, PNG_64)];

#[cfg(test)]
#[path = "window_icon_tests.rs"]
mod tests;
