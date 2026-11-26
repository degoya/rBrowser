//! The token's user SID on Windows. Win32 has no safe wrapper for the token API, so this file
//! carries the unsafe blocks, each with its SAFETY note.

use std::io;
use std::os::windows::io::{
    AsHandle as _, AsRawHandle as _, BorrowedHandle, FromRawHandle as _, OwnedHandle,
};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// The current user's SID as text (`S-1-5-21-...`).
#[allow(
    unsafe_code,
    reason = "OpenProcessToken has no safe wrapper; the handle goes into an OwnedHandle"
)]
pub fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: the pseudo handle of this process and a valid out pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `token` is the open handle just made and owned by nothing else; the `OwnedHandle`
    // closes it exactly once.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    user_sid_of(token.as_handle())
}

/// The user SID of the access token `token` as text; an error for a handle that is no token
/// opened for `TOKEN_QUERY` (RB-099-17 tests that case with a file's handle).
#[allow(unsafe_code, reason = "GetTokenInformation has no safe wrapper")]
fn user_sid_of(token: BorrowedHandle<'_>) -> io::Result<String> {
    let mut length = 0_u32;
    // SAFETY: an open handle (a token or not: the call checks its type) and no buffer; asks
    // for the size only and fails with ERROR_INSUFFICIENT_BUFFER.
    unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut length) };
    // u64 items keep the buffer aligned for TOKEN_USER.
    let mut buffer = vec![0_u64; (length as usize).div_ceil(8).max(1)];
    // SAFETY: an open handle; the buffer holds at least `length` bytes and is aligned for
    // TOKEN_USER.
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetTokenInformation wrote a TOKEN_USER at the start of the aligned buffer; its
    // SID points into the same buffer, which lives until the end of this function.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    // SAFETY: a valid SID inside `buffer`, which outlives the call.
    unsafe { sid_text(sid) }
}

/// `sid` as text (`S-1-...`), also for the DACL the tests read back (`crate::dacl`).
///
/// # Safety
///
/// `sid` points to readable memory that holds a SID (the call validates it before it reads
/// past the header) and stays alive for the call.
#[allow(
    unsafe_code,
    reason = "ConvertSidToStringSidW and LocalFree have no safe wrapper"
)]
pub(crate) unsafe fn sid_text(sid: PSID) -> io::Result<String> {
    let mut text: windows_sys::core::PWSTR = null_mut();
    // SAFETY: a readable SID (the caller's promise) and a valid out pointer; the string is freed
    // with LocalFree below.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 || text.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `text` is a NUL-terminated wide string from ConvertSidToStringSidW.
    let length = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
    // SAFETY: `length` characters before the NUL, read once, then freed.
    let sid_text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    // SAFETY: allocated by ConvertSidToStringSidW with LocalAlloc, freed once.
    unsafe { LocalFree(text.cast()) };
    Ok(sid_text)
}

#[cfg(test)]
#[path = "user_sid_windows_tests.rs"]
mod tests;
