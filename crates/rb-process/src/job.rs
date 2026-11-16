//! A process tree that ends with its owner (RB-080-67). On Windows a child that exits leaves its
//! own children behind - they keep running, are in no process tree any more and are reached
//! neither by `taskkill /T` nor by rBrowser's process count. A [`ProcessJob`] is a job object
//! with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; [`spawn_in_job`] starts the child suspended, puts
//! it into its job and only then lets it run, so nothing it starts runs outside (review 9); and
//! everything it starts joins too, at any depth. Dropping the job, or rBrowser's end in any
//! way (quit, crash, the task manager), ends every process still in it. [`adopted_pids`] lists
//! the processes of every live job, so the process count sees an orphan of ours too.
//!
//! A child that could not join its job runs without one and is counted ([`untied_spawns`]), so
//! the app can say so where a person sees it.
//!
//! Elsewhere there is no such object: [`ProcessJob::adopt`] answers `Unsupported`, and the
//! caller's own stop is all there is.

use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use tokio::process::{Child, Command};

#[cfg(windows)]
#[path = "job_windows.rs"]
mod platform;

#[cfg(not(windows))]
mod platform {
    //! No job objects: nothing is ever made.

    use std::io;

    pub struct Job;

    impl Job {
        pub fn adopt(_child: &tokio::process::Child) -> io::Result<Self> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "job objects exist on Windows only",
            ))
        }

        pub fn active(&self) -> Option<u32> {
            None
        }

        pub fn pids(&self) -> Vec<u32> {
            Vec::new()
        }

        pub fn terminate(&self) -> io::Result<()> {
            Ok(())
        }
    }

    pub fn spawn_suspended(
        command: &mut tokio::process::Command,
    ) -> io::Result<tokio::process::Child> {
        command.spawn()
    }

    pub fn resume(_child: &tokio::process::Child) -> io::Result<()> {
        Ok(())
    }
}

/// How many children ran without their job because it could not be made or joined.
static UNTIED: AtomicU32 = AtomicU32::new(0);

/// Starts `command` inside a job of its own: on Windows suspended, put into the job, then let
/// run. The job is an error when it could not be made or joined (`Unsupported` outside
/// Windows); the child then runs without it, and on Windows [`untied_spawns`] counts it. The
/// creation flags are set here: [`crate::CREATE_NO_WINDOW`] stays.
pub fn spawn_in_job(command: &mut Command) -> io::Result<(Child, io::Result<ProcessJob>)> {
    let mut child = platform::spawn_suspended(command)?;
    let job = ProcessJob::adopt(&child);
    if let Err(error) = platform::resume(&child) {
        let _ = child.start_kill();
        return Err(error);
    }
    if job
        .as_ref()
        .is_err_and(|error| error.kind() != io::ErrorKind::Unsupported)
    {
        UNTIED.fetch_add(1, Ordering::Relaxed);
    }
    Ok((child, job))
}

/// How many children of this run of the app run without the job they should be in.
pub fn untied_spawns() -> u32 {
    UNTIED.load(Ordering::Relaxed)
}

/// A child's process tree, ended when this is dropped (on Windows; see the module).
pub struct ProcessJob {
    job: Arc<platform::Job>,
}

impl std::fmt::Debug for ProcessJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessJob")
            .field("active", &self.active_processes())
            .finish()
    }
}

impl ProcessJob {
    /// A job of its own for `child`, which joins it at once; `Unsupported` outside Windows.
    /// What `child` started before this call stays outside: call it right after the spawn.
    pub fn adopt(child: &Child) -> io::Result<Self> {
        let job = Arc::new(platform::Job::adopt(child)?);
        registry().add(&job);
        Ok(Self { job })
    }

    /// How many processes of the tree run now; `None` when that cannot be read.
    pub fn active_processes(&self) -> Option<u32> {
        self.job.active()
    }

    /// The ids of the tree's running processes.
    pub fn pids(&self) -> Vec<u32> {
        self.job.pids()
    }

    /// Ends every process of the tree now.
    pub fn terminate(&self) -> io::Result<()> {
        self.job.terminate()
    }
}

/// The processes of every live [`ProcessJob`] (empty outside Windows).
pub fn adopted_pids() -> Vec<u32> {
    registry()
        .live()
        .iter()
        .flat_map(|job| job.pids())
        .collect()
}

fn registry() -> &'static Registry<platform::Job> {
    static REGISTRY: Registry<platform::Job> = Registry {
        entries: Mutex::new(Vec::new()),
    };
    &REGISTRY
}

/// Weak references to the live jobs: a job ends with its last owner, never with the registry.
pub(crate) struct Registry<T> {
    entries: Mutex<Vec<Weak<T>>>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

impl<T> Registry<T> {
    pub(crate) fn add(&self, entry: &Arc<T>) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Arc::downgrade(entry));
    }

    /// The entries still owned somewhere; the dropped ones are forgotten.
    pub(crate) fn live(&self) -> Vec<Arc<T>> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|entry| entry.strong_count() > 0);
        entries.iter().filter_map(Weak::upgrade).collect()
    }
}

/// The most attempts to read a job's process ids.
pub(crate) const MAX_ATTEMPTS: u32 = 4;
/// The most process ids read from one job.
pub(crate) const MAX_IDS: usize = 16 * 1024;

/// What to do after one read of a job's process ids.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ListStep {
    /// The ids.
    Done(Vec<u32>),
    /// Read again with a buffer of this many words.
    Grow(usize),
}

/// After a read of `words` words (`header` of them the list's header) that succeeded (`ok`) or
/// did not fit (`more_data`): the job counts `assigned` processes, the list holds `in_list` of
/// `ids`. A list that did not fit is read again - larger, or at the same size when the job
/// shrank meanwhile - up to [`MAX_ATTEMPTS`] times and [`MAX_IDS`]; then what fit (review 2).
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "read by the Windows job and the tests")
)]
#[allow(
    clippy::too_many_arguments,
    reason = "the fields of one Win32 answer, kept apart to test the decision alone"
)]
pub(crate) fn list_step(
    ok: bool,
    more_data: bool,
    assigned: u32,
    in_list: u32,
    ids: &[usize],
    words: usize,
    header: usize,
    attempt: u32,
) -> ListStep {
    if ok || !more_data || attempt >= MAX_ATTEMPTS || words >= MAX_IDS {
        return ListStep::Done(ids_from_list(ids, in_list));
    }
    let wanted = usize::try_from(assigned)
        .unwrap_or(MAX_IDS)
        .saturating_add(header);
    if wanted <= words {
        return ListStep::Grow(words);
    }
    ListStep::Grow(wanted.saturating_add(16).min(MAX_IDS))
}

/// The process ids of a `JOBOBJECT_BASIC_PROCESS_ID_LIST`: `count` of the `ids` after its
/// header, at most as many as the buffer holds; a value wider than 32 bits is no process id.
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "read by the Windows job and the tests")
)]
pub(crate) fn ids_from_list(ids: &[usize], count: u32) -> Vec<u32> {
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    ids.iter()
        .take(count)
        .filter_map(|&id| u32::try_from(id).ok())
        .collect()
}

#[cfg(test)]
#[path = "job_tests.rs"]
mod tests;
