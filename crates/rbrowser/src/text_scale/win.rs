//! Windows' text size (*Settings > Accessibility > Text size*, RB-090-26): the DWORD
//! `TextScaleFactor` under `HKCU\Software\Microsoft\Accessibility`, a percentage from 100 to
//! 225, absent while it was never changed. A thread of its own waits on the key's change
//! notification (`RegNotifyChangeKeyValue`, synchronous) and hands every new value on; it lives
//! as long as the process. The window procedure stays CEF's.

use std::ffi::c_void;
use std::mem::MaybeUninit;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD, RegCloseKey,
    RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};

/// Where Windows keeps the text size.
const ACCESSIBILITY: &str = r"Software\Microsoft\Accessibility";
/// The text size in percent; absent: 100 %.
const TEXT_SCALE_FACTOR: &str = "TextScaleFactor";

/// `text` as a NUL-terminated UTF-16 string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Windows' `TextScaleFactor`; `None` when it is absent or cannot be read.
#[allow(unsafe_code, reason = "RegGetValueW has no safe wrapper")]
pub fn read() -> Option<u32> {
    let key = wide(ACCESSIBILITY);
    let name = wide(TEXT_SCALE_FACTOR);
    let mut value: u32 = 0;
    let mut size = u32::try_from(std::mem::size_of::<u32>()).ok()?;
    // SAFETY: two NUL-terminated UTF-16 strings and two out pointers that outlive the call;
    // `size` is the size of `value`, the DWORD that RRF_RT_REG_DWORD restricts the value to;
    // the type is not asked for (null).
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut value).cast::<c_void>(),
            &raw mut size,
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

/// Calls `on_change` with the new value (as [`read`]) whenever a value of the key changes, on
/// a thread of its own. Without the key, or without a thread, the text size is not followed
/// live; a line in the log says so.
pub fn watch(on_change: impl Fn(Option<u32>) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("rb-text-scale".to_owned())
        .spawn(move || {
            let Some(key) = open() else {
                tracing::info!("Windows' text size is read at the start only: no key to watch");
                return;
            };
            while wait_for_change(key) {
                on_change(read());
            }
            tracing::warn!("Windows' text size is no longer followed");
            close(key);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "Windows' text size is read at the start only");
    }
}

/// The accessibility key, opened for reading and change notifications.
#[allow(unsafe_code, reason = "RegOpenKeyExW has no safe wrapper")]
fn open() -> Option<HKEY> {
    let subkey = wide(ACCESSIBILITY);
    let mut key = MaybeUninit::<HKEY>::uninit();
    // SAFETY: `subkey` is NUL-terminated and outlives the call; `key` receives the handle on
    // success and is read only then. KEY_READ includes KEY_NOTIFY.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_READ,
            key.as_mut_ptr(),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    // SAFETY: RegOpenKeyExW succeeded and wrote the handle.
    Some(unsafe { key.assume_init() })
}

/// Blocks until a value of `key` changes; `false` when the notification fails.
#[allow(unsafe_code, reason = "RegNotifyChangeKeyValue has no safe wrapper")]
fn wait_for_change(key: HKEY) -> bool {
    // SAFETY: `key` is the open handle from `open`, used by this thread only; no event handle
    // (null) and FALSE for asynchronous make the call wait on this thread until a value of the
    // key itself (FALSE: not its subtree) is written.
    let status = unsafe {
        RegNotifyChangeKeyValue(key, 0, REG_NOTIFY_CHANGE_LAST_SET, std::ptr::null_mut(), 0)
    };
    status == ERROR_SUCCESS
}

/// Closes `key`.
#[allow(unsafe_code, reason = "RegCloseKey has no safe wrapper")]
fn close(key: HKEY) {
    // SAFETY: `key` is the handle `open` returned, closed once.
    unsafe { RegCloseKey(key) };
}
