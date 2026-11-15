//! Whether the process behind a staging folder still runs (RB-080-30). The store under
//! `%LOCALAPPDATA%` is shared by every rBrowser instance, one per project, and every install
//! stages under `<id>-<pid>-<n>`: a folder whose pid is alive is another window's install under
//! way, one whose pid is gone a crashed install's leftover. Windows asks the process
//! ([`alive_windows`]), Linux `/proc`; elsewhere every pid counts as alive, so nothing live is
//! ever deleted (phase 2 decides).

#[cfg(windows)]
#[path = "alive_windows.rs"]
mod platform;

#[cfg(target_os = "linux")]
mod platform {
    pub fn alive(pid: u32) -> bool {
        std::path::Path::new("/proc").join(pid.to_string()).exists()
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    pub fn alive(_pid: u32) -> bool {
        true
    }
}

/// Whether a process with `pid` runs; this process's own pid included.
pub fn process_alive(pid: u32) -> bool {
    pid == std::process::id() || platform::alive(pid)
}

/// The pid in a staging folder's name `<id>-<pid>-<n>`; `None` for another id's or a name
/// that is not an attempt's.
pub fn staging_pid(name: &str, id: &str) -> Option<u32> {
    let rest = name.strip_prefix(id)?.strip_prefix('-')?;
    let (pid, attempt) = rest.split_once('-')?;
    attempt.parse::<u64>().ok()?;
    pid.parse().ok()
}

#[cfg(test)]
#[path = "alive_tests.rs"]
mod tests;
