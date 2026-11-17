//! The installed distributions: names, the default, WSL versions and whether each one runs.
//!
//! The list comes from `wsl.exe --list --verbose`:
//!
//! ```text
//!   NAME            STATE           VERSION
//! * Ubuntu          Running         2
//!   Debian          Stopped         1
//! ```
//!
//! The header and the state follow the display language, and a translated state may hold a
//! space, so a row is read from its ends: the default marker and the name in front, the version
//! last. The state is read only in English; `wsl.exe --list --running --quiet`, which is not
//! translated, then settles it ([`apply_running`]). `wsl.exe` is always the one in
//! `%SystemRoot%\System32`, never a name looked up on `PATH`. Adapted from rCommander's
//! `rc-vfs::wsl`.

use serde::Serialize;

use crate::WslError;

/// The Windows folder when `SystemRoot` is missing or not a drive path.
const DEFAULT_SYSTEM_ROOT: &str = r"C:\Windows";

/// Whether a distribution runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistroState {
    /// Running now.
    Running,
    /// Installed and stopped; starting the helper starts it.
    Stopped,
    /// A state not read (translated, installing, converting).
    Unknown,
}

/// One installed distribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Distro {
    /// The name as WSL spells it, e.g. `Ubuntu-24.04`.
    pub name: String,
    /// Whether `wsl.exe` without `-d` starts this one.
    pub is_default: bool,
    /// Running or stopped.
    pub state: DistroState,
    /// WSL 1 or 2.
    pub version: Option<u8>,
}

/// The distributions of `wsl.exe --list --verbose` output, in its order; none for the messages
/// it prints instead of a list (WSL missing, switched off, no distribution).
#[must_use]
pub fn parse_verbose(text: &str) -> Vec<Distro> {
    let mut distros: Vec<Distro> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let (is_default, row) = match line.strip_prefix('*') {
            Some(row) => (true, row),
            None => (false, line),
        };
        let words: Vec<&str> = row.split_whitespace().collect();
        let [name, middle @ .., last] = words.as_slice() else {
            continue;
        };
        let Ok(version @ 1..=2) = last.parse::<u8>() else {
            continue;
        };
        if middle.is_empty() || !is_distro_name(name) || known(&distros, name) {
            continue;
        }
        let state = match middle {
            ["Running"] => DistroState::Running,
            ["Stopped"] => DistroState::Stopped,
            _ => DistroState::Unknown,
        };
        distros.push(Distro {
            name: (*name).to_owned(),
            is_default,
            state,
            version: Some(version),
        });
    }
    distros
}

/// The names of `wsl.exe --list --quiet` output (also `--running`), in its order, each once.
#[must_use]
pub fn parse_quiet(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in text.lines() {
        let name = line.trim();
        if is_distro_name(name) && !names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            names.push(name.to_owned());
        }
    }
    names
}

/// Sets each distribution's state from the names `wsl.exe --list --running --quiet` printed.
pub fn apply_running(distros: &mut [Distro], running: &[String]) {
    for distro in distros {
        distro.state = if running
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&distro.name))
        {
            DistroState::Running
        } else {
            DistroState::Stopped
        };
    }
}

/// The distribution to use: `wanted` when given (the `wsl.distro` setting or the project's
/// share), else WSL's default, else the first one.
///
/// # Errors
///
/// [`WslError::DistroNotFound`] when `wanted` is not installed, [`WslError::NoDistro`] when the
/// list is empty.
pub fn choose<'a>(distros: &'a [Distro], wanted: Option<&str>) -> Result<&'a Distro, WslError> {
    if let Some(wanted) = wanted {
        return distros
            .iter()
            .find(|distro| distro.name.eq_ignore_ascii_case(wanted))
            .ok_or_else(|| WslError::DistroNotFound(wanted.to_owned()));
    }
    distros
        .iter()
        .find(|distro| distro.is_default)
        .or_else(|| distros.first())
        .ok_or(WslError::NoDistro)
}

/// `wsl.exe` in the `System32` folder of `system_root` (the `SystemRoot` variable); the
/// default Windows folder when it is missing or not an absolute drive path.
#[must_use]
pub fn wsl_exe_in(system_root: Option<&str>) -> String {
    let root = system_root
        .map(|root| root.trim_end_matches('\\'))
        .filter(|root| {
            let bytes = root.as_bytes();
            bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && bytes[2] == b'\\'
                && !root.split('\\').any(|name| name == "..")
        })
        .unwrap_or(DEFAULT_SYSTEM_ROOT);
    format!(r"{root}\System32\wsl.exe")
}

/// `wsl.exe` of this machine, from the `SystemRoot` variable.
#[must_use]
pub fn wsl_exe() -> String {
    wsl_exe_in(std::env::var("SystemRoot").ok().as_deref())
}

/// What WSL accepts as a distribution name: ASCII letters, digits, `.`, `-` and `_`.
fn is_distro_name(name: &str) -> bool {
    crate::path::valid_distro(name)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn known(distros: &[Distro], name: &str) -> bool {
    distros
        .iter()
        .any(|distro| distro.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "distro_tests.rs"]
mod tests;
