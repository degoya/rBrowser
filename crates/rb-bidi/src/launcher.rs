//! The installed Firefox (ADR 0001, Decision 7): found by the setting `engines.firefox.path`,
//! then the registry (`App Paths\firefox.exe`, the uninstall entries), then the standard folders
//! of the four editions ([`installed`], RB-080-32); started with its own profile, `-no-remote
//! -new-instance` (beside the user's own Firefox, never in it) and `--remote-debugging-port 0`,
//! so Firefox picks a free port on 127.0.0.1 and prints the endpoint on stderr. No geckodriver.
//! The process gets no console window and dies with its handle, and on Windows with the app
//! itself (`job`).

pub mod installed;
mod job;
#[cfg(windows)]
mod registry;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rb_process::NoConsoleWindow;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

pub use installed::{Candidate, Installed, Kind};

use crate::error::BidiError;

/// The stderr line that names the endpoint.
pub const LISTENING: &str = "WebDriver BiDi listening on ";
/// How long Firefox may take to print it (a cold start on a slow disk).
pub const START_TIMEOUT: Duration = Duration::from_secs(30);

/// Why no Firefox was found.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LocateError {
    /// `engines.firefox.path` names a file that does not exist.
    #[error("the Firefox set in engines.firefox.path does not exist: {}", .0.display())]
    Configured(PathBuf),
    /// Nowhere.
    #[error("Firefox was not found")]
    NotFound,
}

/// Where Firefox is looked for after the setting, in order: the registry's entries, then the
/// standard install folders of the platform ([`installed::candidates`]).
pub fn candidates(
    windows: bool,
    app_path: Option<Candidate>,
    uninstall: Vec<Candidate>,
    env: impl Fn(&str) -> Option<OsString>,
) -> Vec<PathBuf> {
    installed::candidates(windows, app_path, uninstall, env)
        .into_iter()
        .map(|candidate| candidate.path)
        .collect()
}

/// The setting when it is set (it must exist), else the first candidate that exists.
pub fn locate(
    configured: Option<&Path>,
    candidates: Vec<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> Result<PathBuf, LocateError> {
    if let Some(path) = configured.filter(|path| !path.as_os_str().is_empty()) {
        return if exists(path) {
            Ok(path.to_path_buf())
        } else {
            Err(LocateError::Configured(path.to_path_buf()))
        };
    }
    candidates
        .into_iter()
        .find(|path| exists(path))
        .ok_or(LocateError::NotFound)
}

/// Finds the installed Firefox on this machine: the setting, else the first of
/// [`find_installed`]'s search order that exists - the one source of which is "the first".
pub fn find_firefox(configured: Option<&Path>) -> Result<PathBuf, LocateError> {
    let (app_path, uninstall) = registry_candidates();
    let candidates = candidates(cfg!(windows), app_path, uninstall, |name| {
        std::env::var_os(name)
    });
    locate(configured, candidates, Path::is_file)
}

/// Every Firefox installed on this machine, in search order, with edition and version.
pub fn find_installed() -> Vec<Installed> {
    let (app_path, uninstall) = registry_candidates();
    let candidates = installed::candidates(cfg!(windows), app_path, uninstall, |name| {
        std::env::var_os(name)
    });
    installed::installed(candidates, cfg!(windows), Path::is_file, |path| {
        std::fs::read_to_string(path).ok()
    })
}

#[cfg(windows)]
fn registry_candidates() -> (Option<Candidate>, Vec<Candidate>) {
    installed::from_registry(
        registry::app_path(installed::EXE_WINDOWS),
        registry::uninstall_locations(),
    )
}

#[cfg(not(windows))]
fn registry_candidates() -> (Option<Candidate>, Vec<Candidate>) {
    (None, Vec::new())
}

/// The session endpoint in a stderr line of Firefox, `ws://127.0.0.1:<port>/session`.
pub fn endpoint_in(line: &str) -> Option<String> {
    let start = line.find(LISTENING)? + LISTENING.len();
    let url = line[start..].split_whitespace().next()?;
    let url = url.trim_end_matches('/');
    url.starts_with("ws://").then(|| format!("{url}/session"))
}

/// The command line: the profile, beside any other Firefox, a free port, a blank first page.
pub fn arguments(profile: &Path) -> Vec<OsString> {
    let mut args = vec![OsString::from("-profile"), profile.as_os_str().to_owned()];
    args.extend(
        [
            "-no-remote",
            "-new-instance",
            "-wait-for-browser",
            "--remote-debugging-port",
            "0",
            "about:blank",
        ]
        .map(OsString::from),
    );
    args
}

/// A running Firefox: killed when dropped.
#[derive(Debug)]
pub struct FirefoxProcess {
    child: Child,
    endpoint: String,
}

impl FirefoxProcess {
    /// Its session endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Its process id while it runs: the owner of the Firefox windows (RB-080-39).
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// After its connection dropped: `Some(true)` when Firefox ended by itself with exit status
    /// 0 within `grace` (it quits with its last window, which is no crash - RB-080-38),
    /// `Some(false)` when it ended otherwise, `None` when it still ran and was killed.
    pub async fn ended_cleanly(mut self, grace: Duration) -> Option<bool> {
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(Ok(status)) => Some(status.success()),
            Ok(Err(_)) => Some(false),
            Err(_) => {
                if let Err(error) = self.child.kill().await {
                    tracing::debug!(%error, "Firefox could not be killed");
                }
                None
            }
        }
    }

    /// Waits up to `grace` for Firefox to end by itself (after `browser.close`), then kills it.
    pub async fn stop(mut self, grace: Duration) {
        let ended = tokio::time::timeout(grace, self.child.wait()).await;
        if ended.is_err()
            && let Err(error) = self.child.kill().await
        {
            tracing::debug!(%error, "Firefox could not be killed");
        }
    }
}

/// Starts `executable` with `profile` and waits up to `timeout` for its endpoint.
pub async fn launch(
    executable: &Path,
    profile: &Path,
    timeout: Duration,
) -> Result<FirefoxProcess, BidiError> {
    let mut command = Command::new(executable);
    command
        .args(arguments(profile))
        // No crash report leaves the machine.
        .env("MOZ_CRASHREPORTER_DISABLE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_console_window();
    let mut child = command.spawn().map_err(|error| {
        BidiError::Launch(format!(
            "Firefox could not be started from {}: {}",
            executable.display(),
            error.kind()
        ))
    })?;
    job::adopt(&child);
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| BidiError::Launch("Firefox has no stderr".to_owned()))?;
    let mut lines = BufReader::new(stderr).lines();
    let found = tokio::time::timeout(timeout, async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(endpoint) = endpoint_in(&line) {
                return Some(endpoint);
            }
        }
        None
    })
    .await;
    let endpoint = match found {
        Ok(Some(endpoint)) => endpoint,
        Ok(None) => {
            return Err(BidiError::Launch(
                "Firefox ended before it opened its remote endpoint (is this profile in use?)"
                    .to_owned(),
            ));
        }
        Err(_) => {
            return Err(BidiError::Launch(format!(
                "Firefox did not open its remote endpoint within {} s",
                timeout.as_secs()
            )));
        }
    };
    // The rest of stderr is drained unread: a full pipe would stall Firefox, and its lines can
    // carry page output.
    tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
    Ok(FirefoxProcess { child, endpoint })
}

#[cfg(test)]
#[path = "launcher_tests.rs"]
mod tests;
