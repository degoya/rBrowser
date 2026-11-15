//! The Win32 half of `super` (`AttachConsole`, the standard handles, `FreeConsole`). Handles are
//! kept as plain numbers (0: none) so the state can live in statics.

use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole, GetConsoleWindow, GetStdHandle,
    STD_ERROR_HANDLE, STD_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
};

use super::{Slot, slots};

/// Input, output, errors.
const IDS: [STD_HANDLE; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];

/// `attach` ran in a program without a console of its own: `release` has work to do.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// `attach` joined the parent's console: `release` leaves it.
static ATTACHED: AtomicBool = AtomicBool::new(false);

pub fn attach() {
    if has_console() || ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }
    let before = IDS.map(std_handle);
    let attached = attach_parent();
    ATTACHED.store(attached, Ordering::SeqCst);
    // Set every slot again: AttachConsole may fill empty ones with the console, input included.
    let mut screen = 0;
    for ((id, handle), slot) in IDS
        .into_iter()
        .zip(before)
        .zip(slots(before.map(|h| h != 0), attached))
    {
        let value = match slot {
            Slot::Keep => handle,
            Slot::Empty => 0,
            Slot::Console => {
                if screen == 0 {
                    screen = open_screen();
                }
                screen
            }
        };
        set_std_handle(id, value);
    }
}

pub fn release() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut closed = Vec::with_capacity(IDS.len());
    for id in IDS {
        let handle = std_handle(id);
        // Emptied first: a write racing this one goes nowhere instead of to a closed handle.
        set_std_handle(id, 0);
        if handle != 0 && !closed.contains(&handle) {
            close(handle);
            closed.push(handle);
        }
    }
    if ATTACHED.swap(false, Ordering::SeqCst) {
        free_console();
    }
}

#[allow(
    unsafe_code,
    reason = "GetConsoleWindow has no safe wrapper; it takes no arguments"
)]
fn has_console() -> bool {
    // SAFETY: no arguments; the window handle is only compared with null.
    !unsafe { GetConsoleWindow() }.is_null()
}

#[allow(
    unsafe_code,
    reason = "AttachConsole has no safe wrapper; a process without a parent console only fails"
)]
fn attach_parent() -> bool {
    // SAFETY: a constant argument; failure (no parent console) is a return value.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

#[allow(
    unsafe_code,
    reason = "FreeConsole has no safe wrapper; it takes no arguments"
)]
fn free_console() {
    // SAFETY: no arguments; called once, after the handles of the console were closed.
    if unsafe { FreeConsole() } == 0 {
        tracing::debug!("the console could not be left before the window opened");
    }
}

/// The standard handle `id` as a number; 0 when there is none.
#[allow(
    unsafe_code,
    reason = "GetStdHandle has no safe wrapper; a constant id only reads the process's table"
)]
fn std_handle(id: STD_HANDLE) -> usize {
    // SAFETY: `id` is one of the three STD_* constants; the handle is not used here.
    let handle = unsafe { GetStdHandle(id) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        0
    } else {
        handle as usize
    }
}

#[allow(
    unsafe_code,
    reason = "SetStdHandle has no safe wrapper; it only stores the value in the process's table"
)]
fn set_std_handle(id: STD_HANDLE, handle: usize) {
    // SAFETY: `id` is one of the three STD_* constants; `handle` is 0 or a handle this process
    // holds (given at start or opened by `open_screen`).
    unsafe { SetStdHandle(id, handle as HANDLE) };
}

#[allow(
    unsafe_code,
    reason = "CloseHandle has no safe wrapper; each handle is closed once"
)]
fn close(handle: usize) {
    // SAFETY: a standard handle of this process, emptied in the table before and closed once.
    unsafe { CloseHandle(handle as HANDLE) };
}

/// The attached console's screen, `CONOUT$`; 0 when it cannot be opened.
#[allow(
    unsafe_code,
    reason = "CreateFileW has no safe wrapper; the name is a NUL-terminated constant"
)]
fn open_screen() -> usize {
    let name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
    // SAFETY: `name` is NUL-terminated and outlives the call; no security attributes, no
    // template; the result is checked.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        0
    } else {
        handle as usize
    }
}
