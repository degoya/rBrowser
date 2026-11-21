//! The Windows Credential Manager as the store of Basic auth credentials (RB-094-08): one
//! generic credential per target, persisted for the user on this machine
//! (`CRED_PERSIST_LOCAL_MACHINE`, never roaming), its blob - the password, UTF-8 - encrypted by
//! the system with DPAPI. Win32 has no safe wrapper for `CredReadW`/`CredWriteW`/`CredDeleteW`,
//! so each call carries its own `allow` and SAFETY note. The password is copied out of the
//! system's buffer before `CredFree` and travels nowhere else.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
use windows_sys::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};

use super::store::{Credential, CredentialStore, StoreError};

/// The Credential Manager of the signed-in user.
pub struct WindowsStore;

/// `text` as a NUL-terminated UTF-16 string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The UTF-16 string at `pointer` up to its NUL; empty for null.
#[allow(
    unsafe_code,
    reason = "reads a NUL-terminated string the Credential Manager handed out"
)]
fn from_wide(pointer: *const u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0usize;
    // SAFETY: `pointer` is a NUL-terminated UTF-16 string inside the credential CredReadW
    // returned, alive until CredFree; the walk stops at its NUL.
    unsafe {
        while *pointer.add(length) != 0 {
            length += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length))
    }
}

#[allow(
    unsafe_code,
    reason = "GetLastError has no safe wrapper; it takes no arguments"
)]
fn last_error() -> u32 {
    // SAFETY: no arguments; reads this thread's last error code.
    unsafe { GetLastError() }
}

impl CredentialStore for WindowsStore {
    #[allow(
        unsafe_code,
        reason = "CredReadW and CredFree have no safe wrapper; the buffer is freed once"
    )]
    fn read(&self, target: &str) -> Result<Option<Credential>, StoreError> {
        let name = wide(target);
        let mut found: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `name` is NUL-terminated and outlives the call; `found` is a valid out pointer.
        let ok = unsafe { CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &raw mut found) };
        if ok == 0 {
            let code = last_error();
            return if code == ERROR_NOT_FOUND {
                Ok(None)
            } else {
                Err(StoreError::Failed(code))
            };
        }
        if found.is_null() {
            return Ok(None);
        }
        // SAFETY: CredReadW succeeded, so `found` points to a CREDENTIALW the system allocated;
        // its blob holds `CredentialBlobSize` bytes. Both are copied before CredFree, once.
        let (user, password) = unsafe {
            let credential = &*found;
            let blob = if credential.CredentialBlob.is_null() {
                Vec::new()
            } else {
                let size = usize::try_from(credential.CredentialBlobSize).unwrap_or(0);
                std::slice::from_raw_parts(credential.CredentialBlob, size).to_vec()
            };
            let user = from_wide(credential.UserName);
            CredFree(found.cast::<c_void>().cast_const());
            (user, blob)
        };
        let Ok(password) = String::from_utf8(password) else {
            return Ok(None);
        };
        Ok(Credential::new(&user, &password).ok())
    }

    #[allow(
        unsafe_code,
        reason = "CredWriteW has no safe wrapper; every pointer outlives the call"
    )]
    fn write(&self, target: &str, credential: &Credential) -> Result<(), StoreError> {
        let mut name = wide(target);
        let mut user = wide(credential.user());
        let mut blob = credential.password().as_bytes().to_vec();
        let size = u32::try_from(blob.len()).map_err(|_| StoreError::Failed(0))?;
        let entry = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: name.as_mut_ptr(),
            CredentialBlobSize: size,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user.as_mut_ptr(),
            ..CREDENTIALW::default()
        };
        // SAFETY: `entry` and the buffers it points to (`name`, `user`, `blob`) live until the
        // call returns; the system copies them.
        let ok = unsafe { CredWriteW(&raw const entry, 0) };
        // The copy of the password goes before an error can be returned.
        blob.fill(0);
        if ok == 0 {
            return Err(StoreError::Failed(last_error()));
        }
        Ok(())
    }

    #[allow(
        unsafe_code,
        reason = "CredDeleteW has no safe wrapper; the name outlives the call"
    )]
    fn remove(&self, target: &str) -> Result<bool, StoreError> {
        let name = wide(target);
        // SAFETY: `name` is NUL-terminated and outlives the call.
        let ok = unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok != 0 {
            return Ok(true);
        }
        match last_error() {
            ERROR_NOT_FOUND => Ok(false),
            code => Err(StoreError::Failed(code)),
        }
    }
}
