//! Showing and activating the main window on macOS (RB-090-15; the Windows twin is
//! `../activate.rs`). `--background` shows the window without bringing the app to the front;
//! giving the focus back to the app that had it is not on macOS yet. UI thread.

use cef::*;

use super::views::UI;

/// Shows the new main window; without `background` it also takes the focus.
pub fn show(window: &Window, background: bool) {
    window.show();
    if !background {
        window.activate();
    }
}

/// Brings the main window to the front and focuses it.
pub fn activate() {
    if let Some(window) = UI.with(|ui| ui.borrow().window.clone()) {
        window.activate();
    }
}
