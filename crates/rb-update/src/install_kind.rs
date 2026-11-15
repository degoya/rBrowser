//! How this installation came to be, and therefore whether it updates itself or something else
//! does. Only the portable ZIP (RB-090-01) installs an update itself; an MSI or a package
//! manager (RB-090-13, RB-110-04) owns its files, and a program that replaced itself behind its
//! back would leave its records stale. Detection, first match wins:
//!
//! 1. **The marker file** [`INSTALL_KIND_FILE`] (`install-kind`) beside the program: an installer
//!    or a package writes its kind there (`msi`, `scoop`, `winget`). UTF-8, the first line that is
//!    neither empty nor a `#` comment; at most 1 KiB is read. `portable` goes on to step 2; a kind
//!    this build does not know is [`InstallKind::Unknown`].
//! 2. **`VERSION.txt` beside the program** whose first line names rBrowser, as
//!    `scripts/package-windows.sh` writes it, in a folder this process may write:
//!    [`InstallKind::Portable`].
//! 3. Otherwise [`InstallKind::Unknown`]: a build from source, the copy of
//!    `scripts/run-windows.sh`, or a folder like `C:\Program Files` that cannot be written.
//!
//! Adapted from rCommander's `crates/rc-update/src/install_kind.rs` (RB-090-02): the MSI of
//! RB-090-13's plans instead of deb and rpm, no path rules yet.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The marker an installer or a package writes beside the program.
pub const INSTALL_KIND_FILE: &str = "install-kind";
/// What every package carries beside the program.
pub const VERSION_FILE: &str = "VERSION.txt";
/// The most of a marker file that is read.
const MAX_MARKER_BYTES: u64 = 1024;

/// How this installation was installed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    /// The ZIP, unpacked by hand: updates itself.
    Portable,
    /// An MSI installer (placeholder for RB-090-13): the installer updates it.
    Msi,
    /// Scoop (placeholder for RB-090-13).
    Scoop,
    /// winget (placeholder for RB-090-13).
    Winget,
    /// Anything else: offered the download only.
    Unknown,
}

impl InstallKind {
    /// The name in the marker file and the journal.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Portable => "portable",
            Self::Msi => "msi",
            Self::Scoop => "scoop",
            Self::Winget => "winget",
            Self::Unknown => "unknown",
        }
    }

    /// The kind `value` names; anything else is `None`, never a guess.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "portable" => Self::Portable,
            "msi" => Self::Msi,
            "scoop" => Self::Scoop,
            "winget" => Self::Winget,
            _ => return None,
        })
    }

    /// Whether this kind installs an update itself: the portable ZIP only.
    #[must_use]
    pub fn installs_itself(self) -> bool {
        self == Self::Portable
    }
}

/// How the program in `install_dir` was installed; see the module documentation.
#[must_use]
pub fn detect(install_dir: &Path) -> InstallKind {
    if let Some(marked) = marker(install_dir) {
        match InstallKind::parse(&marked) {
            Some(InstallKind::Portable) => {}
            Some(kind) => return kind,
            None => return InstallKind::Unknown,
        }
    }
    let packaged = fs::read_to_string(install_dir.join(VERSION_FILE)).is_ok_and(|text| {
        text.lines()
            .next()
            .is_some_and(|line| line.starts_with("rBrowser "))
    });
    if packaged && writable(install_dir) {
        InstallKind::Portable
    } else {
        InstallKind::Unknown
    }
}

/// The marker's kind line, if the file is there and readable.
fn marker(install_dir: &Path) -> Option<String> {
    let file = fs::File::open(install_dir.join(INSTALL_KIND_FILE)).ok()?;
    let mut text = String::new();
    file.take(MAX_MARKER_BYTES).read_to_string(&mut text).ok()?;
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
}

/// Whether this process may create a file in `folder`.
#[must_use]
pub fn writable(folder: &Path) -> bool {
    let probe = folder.join(".rbrowser-write-probe");
    let written = fs::write(&probe, b"probe").is_ok();
    let _ = fs::remove_file(&probe);
    written
}

#[cfg(test)]
#[path = "install_kind_tests.rs"]
mod tests;
