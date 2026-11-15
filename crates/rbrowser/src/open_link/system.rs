//! The system's default browser: on Windows `ShellExecuteW` with the verb `open` and the checked
//! address as the file - no command line and no shell string, so nothing in the address is ever
//! parsed as a program or an argument. Elsewhere (phase 2) there is none yet.

/// Opens `url` (checked by [`super::address::checked`]) in the default browser; blocks until the
/// shell has taken it.
///
/// # Errors
///
/// The shell refused it (no browser registered, the call failed), or the platform has none.
pub fn open(url: &url::Url) -> Result<(), String> {
    platform::open(url.as_str())
}

#[cfg(windows)]
#[path = "system_windows.rs"]
mod platform;

#[cfg(not(windows))]
mod platform {
    pub(super) fn open(_url: &str) -> Result<(), String> {
        Err("no system browser on this platform yet".to_owned())
    }
}
