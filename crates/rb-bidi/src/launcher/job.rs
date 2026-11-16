//! Firefox ends with rBrowser. On Windows every Firefox the launcher starts goes into one job
//! object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, whose handle the app holds until it ends:
//! however rBrowser ends - quit, crash, the task manager - Windows closes the handle and ends
//! Firefox with every process Firefox started after it joined. Elsewhere there is no such object
//! here; `kill_on_drop` and the orderly stop are all there is.

use tokio::process::Child;

/// Puts `child` into the app's Firefox job. A failure is logged and Firefox runs on without it.
pub fn adopt(child: &Child) {
    #[cfg(windows)]
    windows::adopt(child);
    #[cfg(not(windows))]
    let _unused = child;
}

#[cfg(windows)]
mod windows {
    use std::ptr::{null, without_provenance_mut};
    use std::sync::OnceLock;

    use tokio::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// The job's handle as a number: `HANDLE` is a raw pointer, neither `Send` nor `Sync`, and a
    /// handle is no address anyway. Never closed: the app's end closes it, which is the point.
    struct Job(usize);

    impl Job {
        fn handle(&self) -> HANDLE {
            without_provenance_mut(self.0)
        }
    }

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    #[allow(unsafe_code, reason = "AssignProcessToJobObject has no safe wrapper")]
    pub fn adopt(child: &Child) {
        let Some(job) = JOB.get_or_init(create).as_ref() else {
            return;
        };
        let Some(process) = child.raw_handle() else {
            return;
        };
        // SAFETY: `job` is the open job handle `create` made and never closes; `process` is the
        // handle of the running child, owned by `child`, which outlives the call.
        let assigned = unsafe { AssignProcessToJobObject(job.handle(), process) };
        if assigned == 0 {
            tracing::warn!(
                error = %std::io::Error::last_os_error(),
                "Firefox is not tied to rBrowser's life"
            );
        }
    }

    #[allow(unsafe_code, reason = "the job object API has no safe wrapper")]
    fn create() -> Option<Job> {
        // SAFETY: null attributes and a null name make an unnamed job with default security.
        let job = unsafe { CreateJobObjectW(null(), null()) };
        if job.is_null() {
            tracing::warn!(error = %std::io::Error::last_os_error(), "no job object for Firefox");
            return None;
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            .expect("the limit structure is a few hundred bytes");
        // SAFETY: `job` is the handle just made; `limits` is a live, initialised structure of
        // the class named and `size` is its own size.
        let set = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size,
            )
        };
        if set == 0 {
            tracing::warn!(error = %std::io::Error::last_os_error(), "the Firefox job has no limit");
            // SAFETY: `job` is open and closed exactly once, here.
            unsafe { CloseHandle(job) };
            return None;
        }
        Some(Job(job.addr()))
    }
}
