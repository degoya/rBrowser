//! The process list on Windows (RB-020-09): a Toolhelp snapshot for the ids and parents, then
//! for each process of our tree its start time (`GetProcessTimes`; Windows keeps a dead parent's
//! id in its children, so a process older than its "parent" is somebody else's) and its private
//! bytes (`PROCESS_MEMORY_COUNTERS_EX::PrivateUsage`). Our own children open with
//! `PROCESS_QUERY_LIMITED_INFORMATION`; a process that does not open counts without either. The
//! processes of rBrowser's job objects are read the same way (RB-080-67), and every process
//! carries its image name. Win32 has no safe wrapper for these calls, so this file carries the
//! unsafe blocks, each with its SAFETY note.

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::process::{Details, image_name};

/// A handle, closed on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    #[allow(unsafe_code, reason = "CloseHandle has no safe wrapper")]
    fn drop(&mut self) {
        // SAFETY: a valid handle from CreateToolhelp32Snapshot or OpenProcess, closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// The raw image name of `PROCESSENTRY32W`.
pub type RawName = [u16; 260];

/// Every process with its parent and its raw image name; nothing else is read here
/// ([`super::process::entries_from`] reads ours, [`read`]).
#[allow(
    unsafe_code,
    reason = "CreateToolhelp32Snapshot and Process32*W have no safe wrapper"
)]
pub fn list() -> Option<Vec<(u32, u32, RawName)>> {
    // SAFETY: no pointer arguments; the handle is checked and closed by `Handle`.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return None;
    }
    let snapshot = Handle(snapshot);
    // SAFETY: PROCESSENTRY32W is plain data; all zeros is a valid value before dwSize is set.
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).ok()?;
    let mut list = Vec::new();
    // SAFETY: a valid snapshot handle and an entry with its size set, both outliving the call.
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while more {
        list.push((
            entry.th32ProcessID,
            entry.th32ParentProcessID,
            entry.szExeFile,
        ));
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    Some(list)
}

/// The start time, the private bytes and the image name of one of our processes (only ours
/// are read: opening other users' processes would fail anyway).
pub fn read(pid: u32, raw: &RawName) -> Details {
    let (started, private_bytes) = open_and_read(pid);
    Details {
        private_bytes,
        started,
        name: image_name(raw),
    }
}

/// The start time and the private bytes of `pid`; `None` for what does not open or answer.
#[allow(unsafe_code, reason = "OpenProcess has no safe wrapper")]
fn open_and_read(pid: u32) -> (Option<u64>, Option<u64>) {
    // SAFETY: no pointer arguments; a null handle is checked, a valid one closed by `Handle`.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return (None, None);
    }
    let process = Handle(process);
    (started(&process), private_bytes(&process))
}

/// The creation time of `process`, in 100 ns ticks since 1601.
#[allow(unsafe_code, reason = "GetProcessTimes has no safe wrapper")]
fn started(process: &Handle) -> Option<u64> {
    let zero = FILETIME::default();
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: a valid process handle opened with PROCESS_QUERY_LIMITED_INFORMATION, which
    // GetProcessTimes accepts; four FILETIMEs that outlive the call.
    let ok =
        unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) }
            != 0;
    ok.then_some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

/// The private bytes of `process`; `None` when it does not answer.
#[allow(unsafe_code, reason = "K32GetProcessMemoryInfo has no safe wrapper")]
fn private_bytes(process: &Handle) -> Option<u64> {
    // SAFETY: plain data; all zeros is a valid value before `cb` is set.
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>()).ok()?;
    counters.cb = size;
    // SAFETY: a valid process handle; the EX struct is passed as the base struct with its own
    // size in `cb`, as the API documents, and outlives the call.
    let ok = unsafe {
        K32GetProcessMemoryInfo(
            process.0,
            std::ptr::from_mut(&mut counters).cast::<PROCESS_MEMORY_COUNTERS>(),
            size,
        )
    } != 0;
    ok.then_some(counters.PrivateUsage as u64)
}
