//! How this installation came to be, and therefore whether it updates itself or something else
//! does ([`InstallKind::update_way`]). Only the portable archive (RB-090-01) installs an update
//! itself; a package manager (RB-090-13, RB-110-04) owns its files - a program that replaced
//! itself behind its back would leave its records stale - and is shown its own command; an MSI
//! (none is published yet) and anything unknown are offered the download. Detection, first match
//! wins:
//!
//! 1. **The marker file** [`INSTALL_KIND_FILE`] (`install-kind`) beside the program: an installer
//!    or a package writes its kind there (`scoop` from the bucket's `post_install`; `msi`,
//!    `winget`, `homebrew`, `deb`, `rpm`). UTF-8, the first line that is neither empty nor a `#`
//!    comment; at most 1 KiB is read. `portable` goes on to step 3; a kind this build does not
//!    know is [`InstallKind::Unknown`].
//! 2. **The folder** of a package manager that writes no marker ([`by_path`]): Scoop's
//!    `scoop\apps\rbrowser\`, winget's `WinGet\Packages\degoya.rBrowser_*`, Homebrew's
//!    `Cellar/rbrowser/` and `Caskroom/rbrowser/`.
//! 3. **`VERSION.txt` beside the program** whose first line names rBrowser, as
//!    `scripts/package-windows.sh` writes it, in a folder this process may write:
//!    [`InstallKind::Portable`].
//! 4. Otherwise [`InstallKind::Unknown`]: a build from source, the copy of
//!    `scripts/run-windows.sh`, or a folder like `C:\Program Files` that cannot be written.
//!
//! Adapted from rCommander's `crates/rc-update/src/install_kind.rs` (RB-090-02, RB-090-14).

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
    /// An MSI installer (none is published yet): offered the download.
    Msi,
    /// Scoop (RB-090-13): its bucket writes the marker.
    Scoop,
    /// winget (RB-090-13): its portable ZIP under `WinGet\Packages`.
    Winget,
    /// Homebrew's cask (macOS) or formula (Linux), RB-110-04.
    Homebrew,
    /// The apt repository's package (RB-110-04).
    Deb,
    /// The dnf repository's package (RB-110-04).
    Rpm,
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
            Self::Homebrew => "homebrew",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
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
            "homebrew" => Self::Homebrew,
            "deb" => Self::Deb,
            "rpm" => Self::Rpm,
            _ => return None,
        })
    }

    /// Whether this kind installs an update itself: the portable ZIP only.
    #[must_use]
    pub fn installs_itself(self) -> bool {
        self == Self::Portable
    }

    /// How an installation of this kind takes an offered update.
    #[must_use]
    pub fn update_way(self) -> UpdateWay {
        match self {
            Self::Portable => UpdateWay::SelfUpdate,
            Self::Scoop => UpdateWay::Command("scoop update rbrowser"),
            Self::Winget => UpdateWay::Command("winget upgrade degoya.rBrowser"),
            Self::Homebrew => UpdateWay::Command("brew upgrade rbrowser"),
            Self::Deb => {
                UpdateWay::Command("sudo apt update && sudo apt install --only-upgrade rbrowser")
            }
            Self::Rpm => UpdateWay::Command("sudo dnf upgrade rbrowser"),
            Self::Msi | Self::Unknown => UpdateWay::DownloadOnly,
        }
    }
}

/// How an offered update reaches this installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateWay {
    /// Download, verify, switch and restart (the portable archive).
    SelfUpdate,
    /// The package manager's command, shown to copy; nothing is downloaded.
    Command(&'static str),
    /// The release page, for a download by hand.
    DownloadOnly,
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
    } else if let Some(kind) = by_path(install_dir) {
        return kind;
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

/// The package manager whose own layout `install_dir` lies in, with rBrowser's package in it;
/// compared without case and with either separator. Each rule names the manager's folder and the
/// package's name below it - `scoop/apps/rbrowser/`, `winget/packages/degoya.rbrowser`,
/// `cellar/rbrowser/`, `caskroom/rbrowser/` - so a portable folder whose path merely holds one of
/// those words stays portable. They stand before the `VERSION.txt` check on purpose: a package
/// manager's folder carries the same files as the ZIP and is writable by the person.
#[must_use]
pub fn by_path(install_dir: &Path) -> Option<InstallKind> {
    let path = format!(
        "{}/",
        install_dir
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
    );
    if path.contains("/scoop/apps/rbrowser/") {
        Some(InstallKind::Scoop)
    } else if path.contains("/winget/packages/degoya.rbrowser") {
        Some(InstallKind::Winget)
    } else if path.contains("/cellar/rbrowser/") || path.contains("/caskroom/rbrowser/") {
        Some(InstallKind::Homebrew)
    } else {
        None
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
