//! [`super::process_alive`] on Windows: `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION`
//! and `GetExitCodeProcess`; a process that refuses to open (another user's) runs all the same,
//! one that does not exist does not. Win32 has no safe wrapper for these calls, so this file
//! carries the unsafe blocks, each with its SAFETY note.

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, GetLastError, HANDLE, STILL_ACTIVE,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// A handle, closed on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    #[allow(unsafe_code, reason = "CloseHandle has no safe wrapper")]
    fn drop(&mut self) {
        // SAFETY: a valid handle from OpenProcess, closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Whether `pid` runs.
#[allow(
    unsafe_code,
    reason = "OpenProcess, GetLastError and GetExitCodeProcess have no safe wrapper"
)]
pub fn alive(pid: u32) -> bool {
    // SAFETY: no pointer arguments; a null handle is checked, a valid one closed by `Handle`.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        // SAFETY: no arguments; reads this thread's last error.
        return unsafe { GetLastError() } == ERROR_ACCESS_DENIED;
    }
    let process = Handle(process);
    let mut code = 0u32;
    // SAFETY: a valid process handle with query access and a pointer to a u32 that outlives
    // the call.
    let answered = unsafe { GetExitCodeProcess(process.0, &raw mut code) } != 0;
    answered && code == STILL_ACTIVE as u32
}
