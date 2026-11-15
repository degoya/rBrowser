//! `HKCU`/`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\<exe>`: where an installer
//! registered a program, and the uninstall entries (`...\Uninstall\<name>`, `InstallLocation`)
//! of every Firefox edition (RB-080-32). Windows only; the editions' names are matched in
//! [`super::installed`].

use std::ffi::OsString;
use std::mem::MaybeUninit;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey,
    RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};

/// The uninstall lists, the 64-bit view before the 32-bit one.
const UNINSTALL: [&str; 2] = [
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];
/// The longest key name the enumeration reads (the registry's limit is 255).
const MAX_KEY_NAME: usize = 256;

/// The registered path of `exe`, the user's entry before the machine's.
pub fn app_path(exe: &str) -> Option<PathBuf> {
    let subkey = wide(&format!(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}"
    ));
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .find_map(|root| string_value(root, &subkey, None))
        .map(|value| PathBuf::from(value.trim_matches('"')))
        .filter(|path| !path.as_os_str().is_empty())
}

/// Every uninstall entry of the machine and the user with an `InstallLocation`, as
/// `(display key name, folder)`, the machine's entries before the user's.
pub fn uninstall_locations() -> Vec<(String, PathBuf)> {
    let value_name = wide("InstallLocation");
    let mut found = Vec::new();
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for list in UNINSTALL {
            for name in subkeys(root, &wide(list)) {
                let subkey = wide(&format!("{list}\\{name}"));
                if let Some(location) = string_value(root, &subkey, Some(&value_name)) {
                    found.push((name, PathBuf::from(location.trim_matches('"'))));
                }
            }
        }
    }
    found
}

/// The names of the subkeys of `subkey` under `root`; empty when it cannot be opened.
#[allow(unsafe_code, reason = "the registry enumeration has no safe wrapper")]
fn subkeys(root: HKEY, subkey: &[u16]) -> Vec<String> {
    let mut key = MaybeUninit::<HKEY>::uninit();
    // SAFETY: `subkey` is NUL-terminated and outlives the call; `key` receives the handle on
    // success and is read only then.
    let status = unsafe { RegOpenKeyExW(root, subkey.as_ptr(), 0, KEY_READ, key.as_mut_ptr()) };
    if status != ERROR_SUCCESS {
        return Vec::new();
    }
    // SAFETY: RegOpenKeyExW succeeded and wrote the handle.
    let key = unsafe { key.assume_init() };
    let mut names = Vec::new();
    let mut buffer = vec![0u16; MAX_KEY_NAME];
    for index in 0.. {
        let mut length = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        // SAFETY: `buffer` holds `length` UTF-16 units, which RegEnumKeyExW does not exceed; the
        // class and time pointers are null, so nothing else is written.
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                buffer.as_mut_ptr(),
                &mut length,
                null(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status == ERROR_MORE_DATA {
            // A name longer than the buffer: skip it, read on.
            continue;
        }
        if status != ERROR_SUCCESS {
            break;
        }
        let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
        names.push(
            OsString::from_wide(&buffer[..length])
                .to_string_lossy()
                .into_owned(),
        );
    }
    // SAFETY: `key` is the handle RegOpenKeyExW returned, closed once.
    unsafe { RegCloseKey(key) };
    names
}

/// The string value `name` (the default value for `None`) of `subkey` under `root`.
#[allow(unsafe_code, reason = "RegGetValueW has no safe wrapper")]
fn string_value(root: HKEY, subkey: &[u16], name: Option<&[u16]>) -> Option<String> {
    let name = name.map_or(null(), <[u16]>::as_ptr);
    let mut size: u32 = 0;
    // SAFETY: `subkey` and `name` are NUL-terminated and outlive the call (a null name reads
    // the default value); null type and data pointers ask for the size only, written to `size`.
    let status = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            name,
            RRF_RT_REG_SZ,
            null_mut(),
            null_mut(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; usize::try_from(size).ok()?.div_ceil(2)];
    // SAFETY: `buffer` holds at least `size` bytes, the size passed in; RegGetValueW writes at
    // most that many and updates `size`.
    let status = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            name,
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    OsString::from_wide(&buffer[..length]).into_string().ok()
}

fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
