//! The token's user SID on Windows. Win32 has no safe wrapper for the token API, so this file
//! carries the unsafe blocks, each with its SAFETY note.

use std::io;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// The current user's SID as text (`S-1-5-21-...`).
#[allow(
    unsafe_code,
    reason = "OpenProcessToken, GetTokenInformation and ConvertSidToStringSidW have no safe wrapper"
)]
pub fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: the pseudo handle of this process and a valid out pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(token);
    let mut length = 0_u32;
    // SAFETY: asks for the size only (no buffer); fails with ERROR_INSUFFICIENT_BUFFER.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut length) };
    // u64 items keep the buffer aligned for TOKEN_USER.
    let mut buffer = vec![0_u64; (length as usize).div_ceil(8).max(1)];
    // SAFETY: the buffer holds at least `length` bytes and is aligned for TOKEN_USER.
    let read = unsafe {
        GetTokenInformation(
            token.0,
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
    let mut text: windows_sys::core::PWSTR = null_mut();
    // SAFETY: a valid SID and a valid out pointer; the string is freed with LocalFree below.
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

/// A token handle, closed on drop.
struct Token(HANDLE);

impl Drop for Token {
    #[allow(unsafe_code, reason = "CloseHandle has no safe wrapper")]
    fn drop(&mut self) {
        // SAFETY: a handle from OpenProcessToken, closed once.
        unsafe { CloseHandle(self.0) };
    }
}
