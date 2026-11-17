//! A native message box for what the user must see before any window exists: CEF did not
//! start (`crate::start_failure`). Blocks until it is closed.

use windows_sys::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
};

/// Shows `text` under `title` with an error icon and waits for OK.
#[allow(
    unsafe_code,
    reason = "MessageBoxW has no safe wrapper; both strings are NUL-terminated buffers that outlive the call"
)]
pub fn error(title: &str, text: &str) {
    let title = wide(title);
    let text = wide(text);
    // SAFETY: no owner window (null); `text` and `title` are NUL-terminated UTF-16 buffers alive
    // until the call returns.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
