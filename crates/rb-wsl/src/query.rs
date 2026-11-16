//! Running `wsl.exe` under a time limit, and what the app asks it: the distributions and a
//! distribution's networking mode.
//!
//! Without WSL, or with WSL switched off, `wsl.exe` may print an installation prompt, fail, or
//! not answer at all; every call has a limit after which the process is killed. Every call sets
//! `WSL_UTF8=1` (UTF-8 output where WSL knows it; UTF-16LE is read as well) and starts without a
//! console window. The program is a parameter, so the tests run a stand-in on any platform.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use rb_process::NoConsoleWindow;
use tokio::process::Command;

use crate::WslError;
use crate::decode::decode;
use crate::distro::{Distro, apply_running, parse_quiet, parse_verbose};
use crate::netmode::{Detection, ModeCache, NetworkingMode, from_wslconfig, parse_mode, resolve};
use crate::path::{PathError, valid_distro};

/// How long one `wsl.exe` call may take.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
/// The most characters of an error line kept for a message.
const DETAIL_CHARS: usize = 200;

/// `wsl_exe` with `args`, `WSL_UTF8=1` and no console window.
#[must_use]
pub fn wsl_command(wsl_exe: &str, args: &[&str]) -> Command {
    let mut command = Command::new(wsl_exe);
    command.args(args).env("WSL_UTF8", "1").no_console_window();
    command
}

/// What `command` wrote to its standard output.
///
/// # Errors
///
/// [`WslError::NotInstalled`] when the program does not exist, [`WslError::Timeout`] when it
/// did not end within `limit` (it is killed), [`WslError::Failed`] when it could not start or
/// ended with a failure.
pub async fn capture(mut command: Command, limit: Duration) -> Result<Vec<u8>, WslError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_console_window();
    let output = match tokio::time::timeout(limit, command.output()).await {
        Err(_) => {
            return Err(WslError::Timeout(
                u64::try_from(limit.as_millis()).unwrap_or(u64::MAX),
            ));
        }
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(WslError::NotInstalled);
        }
        Ok(Err(error)) => {
            return Err(WslError::Failed {
                status: "not started".into(),
                detail: error.to_string(),
            });
        }
        Ok(Ok(output)) => output,
    };
    if output.status.success() {
        return Ok(output.stdout);
    }
    // wsl.exe writes some of its errors to standard output.
    let stderr = decode(&output.stderr);
    let stdout = decode(&output.stdout);
    Err(WslError::Failed {
        status: output.status.to_string(),
        detail: first_line(if stderr.trim().is_empty() {
            &stdout
        } else {
            &stderr
        }),
    })
}

/// The installed distributions with their states.
///
/// # Errors
///
/// [`WslError::NotInstalled`] when `wsl.exe` is missing or fails (WSL off, or no distribution
/// on versions that fail then), [`WslError::NoDistro`] for an empty list,
/// [`WslError::Timeout`].
pub async fn list_distros(wsl_exe: &str, limit: Duration) -> Result<Vec<Distro>, WslError> {
    let verbose = match capture(wsl_command(wsl_exe, &["--list", "--verbose"]), limit).await {
        Ok(bytes) => bytes,
        Err(WslError::Failed { status, detail }) => {
            tracing::info!(%status, %detail, "wsl.exe --list failed; WSL is off or has no distribution");
            return Err(WslError::NotInstalled);
        }
        Err(error) => return Err(error),
    };
    let mut distros = parse_verbose(&decode(&verbose));
    if distros.is_empty() {
        return Err(WslError::NoDistro);
    }
    let running = wsl_command(wsl_exe, &["--list", "--running", "--quiet"]);
    match capture(running, limit).await {
        Ok(bytes) => apply_running(&mut distros, &parse_quiet(&decode(&bytes))),
        // No distribution running makes some versions fail; the English states stay.
        Err(error) => tracing::debug!(%error, "wsl.exe --list --running"),
    }
    Ok(distros)
}

/// `wslinfo --networking-mode` in `distro` (starts it when stopped); `None` when `wslinfo` is
/// too old to know the option.
///
/// # Errors
///
/// [`WslError::Path`] for a name WSL cannot have, the errors of [`capture`].
pub async fn wslinfo_mode(
    wsl_exe: &str,
    distro: &str,
    limit: Duration,
) -> Result<Option<NetworkingMode>, WslError> {
    if !valid_distro(distro) {
        return Err(PathError::InvalidDistro(distro.to_owned()).into());
    }
    let args = ["-d", distro, "-e", "wslinfo", "--networking-mode"];
    let bytes = capture(wsl_command(wsl_exe, &args), limit).await?;
    Ok(parse_mode(&decode(&bytes)))
}

/// `distro`'s networking mode: the override when set, else the cached detection, else
/// `wslinfo`, `.wslconfig` (at `wslconfig`) and NAT, kept in `cache`. Never fails: a source that
/// fails is skipped and logged.
pub async fn detect_mode(
    wsl_exe: &str,
    distro: &str,
    override_mode: Option<NetworkingMode>,
    wslconfig: Option<&Path>,
    cache: &ModeCache,
    limit: Duration,
) -> Detection {
    if override_mode.is_some() {
        return resolve(override_mode, None, None);
    }
    if let Some(known) = cache.get(distro) {
        return known;
    }
    let wslinfo = match wslinfo_mode(wsl_exe, distro, limit).await {
        Ok(mode) => mode,
        Err(error) => {
            tracing::info!(%error, distro, "wslinfo --networking-mode; falling back to .wslconfig");
            None
        }
    };
    let config = match (wslinfo, wslconfig) {
        (None, Some(path)) => match tokio::fs::read(path).await {
            Ok(bytes) => from_wslconfig(&decode(&bytes)),
            Err(error) => {
                tracing::debug!(%error, "no .wslconfig");
                None
            }
        },
        _ => None,
    };
    let detection = resolve(None, wslinfo, config);
    cache.insert(distro, detection);
    detection
}

/// The first non-blank line of `text`, at most [`DETAIL_CHARS`] characters.
fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(DETAIL_CHARS)
        .collect()
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
