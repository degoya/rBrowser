//! The job object of [`super::ProcessJob`] on Windows. Win32 has no safe wrapper for the job
//! object API, so this file carries the unsafe blocks, each with its SAFETY note.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};

use tokio::process::{Child, Command};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_PROCESS_ID_LIST,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

use super::{ListStep, list_step};
/// The exit code of the processes [`Job::terminate`] ends.
const TERMINATED: u32 = 1;

/// An open job object; closing its handle ends every process in it.
pub struct Job(OwnedHandle);

/// `size_of::<T>()` as the `u32` the API takes.
fn size_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("a job object structure is a few hundred bytes")
}

impl Job {
    fn handle(&self) -> HANDLE {
        self.0.as_raw_handle()
    }

    #[allow(unsafe_code, reason = "the job object API has no safe wrapper")]
    pub fn adopt(child: &Child) -> io::Result<Self> {
        let process = child
            .raw_handle()
            .ok_or_else(|| io::Error::other("the process has ended"))?;
        // SAFETY: null attributes and a null name make an unnamed job with default security.
        let raw = unsafe { CreateJobObjectW(null(), null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is the open handle just made and owned by nothing else; the
        // `OwnedHandle` closes it exactly once.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(raw) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: an open job handle; `limits` is a live, initialised structure of the class
        // named, and the size passed is its own.
        let set = unsafe {
            SetInformationJobObject(
                job.handle(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_u32::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>(),
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: an open job handle; `process` is the handle of the running child, owned by
        // `child`, which outlives the call.
        if unsafe { AssignProcessToJobObject(job.handle(), process) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    #[allow(unsafe_code, reason = "QueryInformationJobObject has no safe wrapper")]
    pub fn active(&self) -> Option<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: an open job handle; `info` is a live structure of the class named, with its
        // own size; no return length is asked for.
        let ok = unsafe {
            QueryInformationJobObject(
                self.handle(),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast(),
                size_u32::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>(),
                null_mut(),
            )
        } != 0;
        ok.then_some(info.ActiveProcesses)
    }

    #[allow(unsafe_code, reason = "QueryInformationJobObject has no safe wrapper")]
    pub fn pids(&self) -> Vec<u32> {
        let header = std::mem::offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList)
            / size_of::<usize>();
        // A buffer of words: aligned for the structure, its header in the first `header` words.
        let mut words = vec![0_usize; header + 64];
        let mut attempt = 0;
        loop {
            let Ok(bytes) = u32::try_from(words.len() * size_of::<usize>()) else {
                return Vec::new();
            };
            // SAFETY: an open job handle; `words` is a live, writable buffer of `bytes` bytes,
            // aligned for the structure and larger than it.
            let ok = unsafe {
                QueryInformationJobObject(
                    self.handle(),
                    JobObjectBasicProcessIdList,
                    words.as_mut_ptr().cast(),
                    bytes,
                    null_mut(),
                )
            } != 0;
            let more = !ok
                && io::Error::last_os_error().raw_os_error() == i32::try_from(ERROR_MORE_DATA).ok();
            // SAFETY: the buffer holds at least one whole structure (header plus one id),
            // written by the call or still zeroed, and is aligned for it.
            let list = unsafe {
                words
                    .as_ptr()
                    .cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()
                    .read()
            };
            match list_step(
                ok,
                more,
                list.NumberOfAssignedProcesses,
                list.NumberOfProcessIdsInList,
                &words[header..],
                words.len(),
                header,
                attempt,
            ) {
                ListStep::Done(ids) => return ids,
                ListStep::Grow(size) => words = vec![0; size.max(header + 1)],
            }
            attempt += 1;
        }
    }

    #[allow(unsafe_code, reason = "TerminateJobObject has no safe wrapper")]
    pub fn terminate(&self) -> io::Result<()> {
        // SAFETY: an open job handle.
        if unsafe { TerminateJobObject(self.handle(), TERMINATED) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// Starts `command` with its first thread suspended (and without a console window).
pub fn spawn_suspended(command: &mut Command) -> io::Result<Child> {
    command.creation_flags(crate::CREATE_NO_WINDOW | CREATE_SUSPENDED);
    command.spawn()
}

/// Lets a child started by [`spawn_suspended`] run: resumes every thread of its process (a
/// fresh one has exactly one).
#[allow(
    unsafe_code,
    reason = "the Toolhelp and thread APIs have no safe wrapper"
)]
pub fn resume(child: &Child) -> io::Result<()> {
    let pid = child
        .id()
        .ok_or_else(|| io::Error::other("the process has ended"))?;
    // SAFETY: no pointer arguments; the handle is checked and owned by `OwnedHandle` below.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is the open snapshot just made, owned by nothing else, closed once.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = THREADENTRY32 {
        dwSize: size_u32::<THREADENTRY32>(),
        ..THREADENTRY32::default()
    };
    let mut resumed = 0_u32;
    // SAFETY: an open snapshot handle and an entry with its size set, both outliving the call.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: no pointer arguments; a null handle is checked.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                // SAFETY: `thread` was opened with THREAD_SUSPEND_RESUME just above.
                if unsafe { ResumeThread(thread) } != u32::MAX {
                    resumed += 1;
                }
                // SAFETY: `thread` is open and closed exactly once, here.
                unsafe { CloseHandle(thread) };
            }
        }
        // SAFETY: as for Thread32First.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(io::Error::other(
            "the suspended process could not be resumed",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "job_windows_tests.rs"]
mod tests;
