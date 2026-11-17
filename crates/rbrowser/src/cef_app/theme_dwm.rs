//! What Windows itself draws of rBrowser's theme (RB-080-95): the title bar's dark mode
//! (`DwmSetWindowAttribute` with `DWMWA_USE_IMMERSIVE_DARK_MODE`, Windows 10 20H1 and later;
//! an older Windows refuses it and keeps its light bar) and whether Windows uses the dark mode
//! for apps (`AppsUseLightTheme` under `HKCU\...\Themes\Personalize`), which resolves the
//! theme `system`. The colours of the views are `theme.rs`'. UI thread.

use std::ffi::c_void;

use cef::*;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};

/// Where Windows keeps the apps' light or dark mode.
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
/// 1: light, 0: dark; missing (an older Windows): light.
const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

/// `text` as a NUL-terminated UTF-16 string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Draws the title bar of `window` dark or light. A window without a native handle yet, or a
/// Windows without the attribute, keeps its bar; neither is an error worth more than a line.
#[allow(unsafe_code, reason = "DwmSetWindowAttribute has no safe wrapper")]
pub fn set_dark_title_bar(window: &Window, dark: bool) {
    let handle = window.window_handle().0.cast::<c_void>();
    if handle.is_null() {
        return;
    }
    let Ok(attribute) = u32::try_from(DWMWA_USE_IMMERSIVE_DARK_MODE) else {
        return;
    };
    // A Win32 BOOL.
    let value: i32 = i32::from(dark);
    let Ok(size) = u32::try_from(std::mem::size_of::<i32>()) else {
        return;
    };
    // SAFETY: `handle` is the native window CEF gave for a live top-level window (this runs on
    // the UI thread while the window exists); `value` is a BOOL that outlives the call and
    // `size` is its size, as the attribute documents.
    let result = unsafe {
        DwmSetWindowAttribute(handle, attribute, (&raw const value).cast::<c_void>(), size)
    };
    if result < 0 {
        tracing::debug!(result, "the title bar keeps its colours");
    }
}

/// Whether Windows uses the dark mode for apps; light when it cannot be read.
#[allow(unsafe_code, reason = "RegGetValueW has no safe wrapper")]
pub fn system_dark() -> bool {
    let key = wide(PERSONALIZE);
    let name = wide(APPS_USE_LIGHT_THEME);
    let mut light: u32 = 1;
    let Ok(mut size) = u32::try_from(std::mem::size_of::<u32>()) else {
        return false;
    };
    // SAFETY: two NUL-terminated UTF-16 strings and two out pointers that outlive the call;
    // `size` is the size of `light`, the DWORD that RRF_RT_REG_DWORD restricts the value to;
    // the type is not asked for (null).
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut light).cast::<c_void>(),
            &raw mut size,
        )
    };
    status == ERROR_SUCCESS && light == 0
}
