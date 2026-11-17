//! `ShellExecuteW("open", <url>)` on a thread of its own with COM initialised, as the shell
//! wants for its handlers.

use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// A `ShellExecuteW` answer above this is success.
const SUCCESS_ABOVE: isize = 32;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(
    unsafe_code,
    reason = "CoInitializeEx and ShellExecuteW have no safe wrapper; the strings are NUL-terminated \
              buffers that outlive the calls"
)]
pub(super) fn open(url: &str) -> Result<(), String> {
    let verb = wide("open");
    let file = wide(url);
    // SAFETY: the reserved pointer is null as documented; the flags are valid COINIT values.
    let com = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as _,
        )
    };
    // SAFETY: no owner window (null); `verb` and `file` are NUL-terminated UTF-16 buffers alive
    // until the call returns; no parameters and no directory (null).
    let answer = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if com >= 0 {
        // SAFETY: balances the successful CoInitializeEx above on this thread.
        unsafe { CoUninitialize() };
    }
    let code = answer as isize;
    if code > SUCCESS_ABOVE {
        Ok(())
    } else {
        Err(format!("ShellExecuteW answered {code}"))
    }
}
