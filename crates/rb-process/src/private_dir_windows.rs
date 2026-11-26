//! The private folder of [`super::create_private_dir`] on Windows: the DACL of
//! [`super::user_only_sddl`] for the token's user ([`crate::current_user_sid`]), set with `SetNamedSecurityInfoW`, which also
//! hands the inheritable entry to what the folder already holds. Win32 has no safe wrapper for
//! the security API, so this file carries the unsafe blocks, each with its SAFETY note.

use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
    SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR,
};

/// Sets the user-only DACL on `dir`, protected from the parent's entries.
#[allow(
    unsafe_code,
    reason = "GetSecurityDescriptorDacl and SetNamedSecurityInfoW have no safe wrapper"
)]
pub fn restrict(dir: &Path) -> io::Result<()> {
    let descriptor = Descriptor::from_sddl(&super::user_only_sddl(&crate::current_user_sid()?))?;
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl: *mut ACL = null_mut();
    // SAFETY: a valid descriptor (it lives until the end of this function) and valid out
    // pointers; the DACL points into the descriptor.
    let read =
        unsafe { GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut dacl, &mut defaulted) };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    if present == 0 || dacl.is_null() {
        return Err(io::Error::other(
            "the private folder's descriptor has no DACL",
        ));
    }
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: a NUL-terminated wide path and the DACL of the live descriptor; owner, group and
    // SACL are not set (null).
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null(),
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(status).unwrap_or(i32::MAX),
        ));
    }
    Ok(())
}

/// A security descriptor made from SDDL, freed on drop.
struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    #[allow(
        unsafe_code,
        reason = "ConvertStringSecurityDescriptorToSecurityDescriptorW has no safe wrapper"
    )]
    fn from_sddl(sddl: &str) -> io::Result<Self> {
        let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: a NUL-terminated wide string and a valid out pointer; the size is not wanted.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if converted == 0 || descriptor.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for Descriptor {
    #[allow(
        unsafe_code,
        reason = "LocalFree releases what ConvertStringSecurityDescriptorToSecurityDescriptorW allocated"
    )]
    fn drop(&mut self) {
        // SAFETY: allocated by the conversion above with LocalAlloc, freed once.
        unsafe { LocalFree(self.0) };
    }
}

#[cfg(test)]
#[path = "private_dir_windows_tests.rs"]
mod tests;
