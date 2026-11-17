//! Starting the helper from Windows: where it lies as the distribution sees it, its command line
//! and when a crashed helper is started again.
//!
//! The helper ships beside `rbrowser.exe`; the distribution reaches it through its drive mount
//! (`C:\Program Files\rBrowser\rbrowser-wsl` is `/mnt/c/Program Files/rBrowser/rbrowser-wsl`).
//! `wsl.helperPath` names another one, as a Linux or a Windows path. It runs as
//! `wsl.exe -d <distro> -e <path> <subcommand> ...`: `-e` starts it without a shell, so no name
//! is ever interpreted, and a stopped distribution is started by the call itself.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use tokio::process::Command;

use crate::path::{PathError, Translator, normalize_linux, valid_distro};
use crate::query::wsl_command;

/// The helper's file name in the package.
pub const HELPER_FILE: &str = "rbrowser-wsl";

/// The helper as the distribution sees it: `configured` (`wsl.helperPath`, a Linux path or a
/// Windows path) when set, else [`HELPER_FILE`] in `exe_dir`, the Windows folder of
/// `rbrowser.exe`.
///
/// # Errors
///
/// [`PathError`] when the path has no Linux counterpart (a network share, a relative path).
pub fn helper_linux_path(
    translator: &Translator,
    exe_dir: &str,
    configured: Option<&str>,
) -> Result<String, PathError> {
    match configured.map(str::trim).filter(|path| !path.is_empty()) {
        Some(path) if path.starts_with('/') => normalize_linux(path),
        Some(path) => translator.to_linux(path),
        None => {
            let dir = translator.to_linux(exe_dir)?;
            Ok(format!("{}/{HELPER_FILE}", dir.trim_end_matches('/')))
        }
    }
}

/// The arguments of `wsl.exe` that start the helper at `helper` in `distro` with `subcommand`.
///
/// # Errors
///
/// [`PathError::InvalidDistro`] for a name WSL cannot have, [`PathError::NotAbsolute`] for a
/// helper path that is not absolute.
pub fn helper_args(
    distro: &str,
    helper: &str,
    subcommand: &[String],
) -> Result<Vec<String>, PathError> {
    if !valid_distro(distro) {
        return Err(PathError::InvalidDistro(distro.to_owned()));
    }
    let helper = normalize_linux(helper)?;
    let mut args = vec!["-d".to_owned(), distro.to_owned(), "-e".to_owned(), helper];
    args.extend(subcommand.iter().cloned());
    Ok(args)
}

/// A factory of helper commands for [`crate::supervisor::Supervisor::start`]: `wsl_exe` with
/// `args` (from [`helper_args`]), without a console window.
pub fn helper_command_factory(wsl_exe: String, args: Vec<String>) -> impl FnMut() -> Command {
    move || {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        wsl_command(&wsl_exe, &refs)
    }
}

/// When a crashed helper is started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    /// The most crashes within [`Self::window`] that are restarted; one more gives up.
    pub max_crashes: u32,
    /// The time in which crashes count together.
    pub window: Duration,
    /// The wait before the first restart; it doubles with every further crash in the window.
    pub first_delay: Duration,
    /// The longest wait.
    pub max_delay: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_crashes: 5,
            window: Duration::from_secs(60),
            first_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
        }
    }
}

/// The crashes of the recent past, against a [`RestartPolicy`].
#[derive(Debug, Clone)]
pub struct RestartTracker {
    policy: RestartPolicy,
    crashes: VecDeque<Instant>,
}

impl RestartTracker {
    /// No crash yet.
    #[must_use]
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            crashes: VecDeque::new(),
        }
    }

    /// A crash at `now`: the wait before the restart, or `None` to give up.
    pub fn on_crash(&mut self, now: Instant) -> Option<Duration> {
        while self
            .crashes
            .front()
            .is_some_and(|first| now.saturating_duration_since(*first) > self.policy.window)
        {
            self.crashes.pop_front();
        }
        self.crashes.push_back(now);
        let count = u32::try_from(self.crashes.len()).unwrap_or(u32::MAX);
        if count > self.policy.max_crashes {
            return None;
        }
        let factor = 1u32.checked_shl(count - 1).unwrap_or(u32::MAX);
        Some(
            self.policy
                .first_delay
                .saturating_mul(factor)
                .min(self.policy.max_delay),
        )
    }

    /// The crashes within the window.
    #[must_use]
    pub fn recent(&self) -> u32 {
        u32::try_from(self.crashes.len()).unwrap_or(u32::MAX)
    }

    /// Forgets the crashes (a restart the user asked for).
    pub fn reset(&mut self) {
        self.crashes.clear();
    }
}

#[cfg(test)]
#[path = "helper_tests.rs"]
mod tests;
