//! Where the package keeps the Node runtime and the sidecar, relative to the running executable
//! (RB-097-01): the Windows ZIP has `node\` and `sidecar\` beside `rbrowser.exe`; the macOS app
//! bundle has the executable in `rBrowser.app/Contents/MacOS` and both in
//! `rBrowser.app/Contents/Resources` (`scripts/package-macos.sh`). The search is a pure function
//! of the executable's path, so both layouts are tested on every platform.

use std::path::{Path, PathBuf};

/// How a package lays out the runtime and the sidecar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// `node/` and `sidecar/` beside the executable: the Windows ZIP, and every other build.
    Beside,
    /// The macOS app bundle: the executable in `Contents/MacOS`, `node/` and `sidecar/` in
    /// `Contents/Resources`.
    AppBundle,
}

impl Layout {
    /// The layout of this platform's package: the app bundle on macOS, else beside.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::AppBundle
        } else {
            Self::Beside
        }
    }
}

/// The folder that holds `node/` and `sidecar/` for the executable at `exe`, `None` when it has
/// no folder. In the [`Layout::AppBundle`] layout an executable in `<app>/Contents/MacOS` finds
/// them in `<app>/Contents/Resources`; one anywhere else - a development build - beside itself.
pub fn package_dir(exe: &Path, layout: Layout) -> Option<PathBuf> {
    let folder = exe.parent().filter(|dir| !dir.as_os_str().is_empty())?;
    if layout == Layout::AppBundle && folder.file_name().is_some_and(|name| name == "MacOS") {
        let contents = folder.parent()?;
        if contents.file_name().is_some_and(|name| name == "Contents") {
            return Some(contents.join("Resources"));
        }
    }
    Some(folder.to_path_buf())
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
