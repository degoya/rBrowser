//! Showing and activating the main window (RB-010-11, contract 0.1-w1b): `--background` (or
//! `startup.focusOnOpen` off) opens it without taking the focus, a second call's hand-over
//! brings it to the front (the caller allowed it with `AllowSetForegroundWindow`). UI thread.

use cef::*;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

use super::views::UI;

/// Shows the new main window; with `background` the window that had the focus gets it back.
pub fn show(window: &Window, background: bool) {
    if !background {
        window.show();
        return;
    }
    let before = foreground();
    window.show();
    if before != 0 && before != window.window_handle().0 as usize {
        give_back(before);
    }
}

/// Brings the main window to the front and focuses it.
pub fn activate() {
    if let Some(window) = UI.with(|ui| ui.borrow().window.clone()) {
        window.activate();
    }
}

/// The window with the focus now, as a plain value (0: none).
#[allow(
    unsafe_code,
    reason = "GetForegroundWindow has no safe wrapper; it takes no arguments"
)]
fn foreground() -> usize {
    // SAFETY: no arguments; the handle is only compared and handed back below.
    unsafe { GetForegroundWindow() as usize }
}

/// Gives the focus back to `window` (we hold it right after showing ours, so Windows lets us).
#[allow(
    unsafe_code,
    reason = "SetForegroundWindow has no safe wrapper; a stale handle only makes it fail"
)]
fn give_back(window: usize) {
    // SAFETY: a window handle read above; a window closed since makes the call fail, nothing more.
    if unsafe { SetForegroundWindow(window as HWND) } == 0 {
        tracing::debug!("the focus could not be given back after a background start");
    }
}
