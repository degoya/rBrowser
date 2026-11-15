//! Where the app finds its own files (RB-090-15). On Windows they lie beside the executable;
//! in the macOS app bundle the executable is `rBrowser.app/Contents/MacOS/rbrowser` and the
//! files (the shell's `ui/`, the bundled `plugins/`) are in `Contents/Resources`, the CEF
//! framework in `Contents/Frameworks`, the helper apps beside it. Pure path logic, tested on
//! every platform.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The executable of the CEF framework, below the app's (or a helper's) frameworks folder.
const FRAMEWORK: &str = "Chromium Embedded Framework.framework/Chromium Embedded Framework";

/// The folder that holds the app's files for the executable `exe`: the bundle's
/// `Contents/Resources` when `exe` lies in `<name>.app/Contents/MacOS`, else the executable's
/// own folder. `None` only for a path without a parent.
pub fn resource_dir(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    let contents = dir
        .parent()
        .filter(|_| dir.file_name() == Some("MacOS".as_ref()))
        .filter(|contents| contents.file_name() == Some("Contents".as_ref()))
        .filter(|contents| {
            contents
                .parent()
                .and_then(Path::extension)
                .is_some_and(|extension| extension == "app")
        });
    Some(contents.map_or_else(|| dir.to_path_buf(), |contents| contents.join("Resources")))
}

/// The CEF framework's executable as the `cef` crate's macOS loader looks for it: from the
/// app, `Contents/Frameworks`; from a helper (`Contents/Frameworks/<helper>.app/Contents/MacOS`),
/// three folders up. Not resolved: the caller checks that it exists before the loader (which
/// panics without it) runs.
pub fn framework(exe: &Path, helper: bool) -> Option<PathBuf> {
    let dir = exe.parent()?;
    let up = if helper { "../../.." } else { "../Frameworks" };
    Some(dir.join(up).join(FRAMEWORK))
}

/// The process arguments without the `-psn_<n>_<n>` process serial number an older Finder
/// adds to an app it starts, which the command line parser would refuse.
pub fn without_process_serial(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    args.into_iter()
        .filter(|arg| !arg.to_string_lossy().starts_with("-psn_"))
        .collect()
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
