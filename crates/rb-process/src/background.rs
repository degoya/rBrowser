//! Programs that work in the background beside the app (RB-090-52): a video encoder must never
//! take the CPU from the browser views. On Windows such a program starts in the
//! `BELOW_NORMAL_PRIORITY_CLASS` (set with its creation flags in [`crate::low_priority_command`],
//! the one file that sets creation flags); on Unix it starts through `nice -n 10` when the
//! system has `nice`, else at the normal priority. The program keeps its own process id either
//! way (`nice` replaces itself with it), so stopping it stops the program.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `BELOW_NORMAL_PRIORITY_CLASS`: scheduled after every normal program, before idle ones.
pub const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
/// How much lower `nice` starts a background program on Unix (0 normal, 19 lowest).
pub const NICE_INCREMENT: u8 = 10;
/// Where `nice` is looked for on Unix; never a path a project or the environment names.
const NICE_PATHS: [&str; 2] = ["/usr/bin/nice", "/bin/nice"];

/// The first of `candidates` that is a regular file.
pub(crate) fn first_file(candidates: &[&Path]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.to_path_buf())
}

/// The system's `nice`, looked up once; `None` on Windows and where there is none.
pub fn nice_program() -> Option<&'static Path> {
    static NICE: OnceLock<Option<PathBuf>> = OnceLock::new();
    NICE.get_or_init(|| {
        if cfg!(windows) {
            return None;
        }
        let candidates: Vec<&Path> = NICE_PATHS.iter().map(Path::new).collect();
        first_file(&candidates)
    })
    .as_deref()
}

/// The program to start and the arguments before `program`'s own: `nice -n 10 <program>` with a
/// `nice`, `program` itself without one.
#[must_use]
pub fn low_priority_argv(program: &Path, nice: Option<&Path>) -> (PathBuf, Vec<OsString>) {
    match nice {
        Some(nice) => (
            nice.to_path_buf(),
            vec![
                OsString::from("-n"),
                OsString::from(NICE_INCREMENT.to_string()),
                program.as_os_str().to_owned(),
            ],
        ),
        None => (program.to_path_buf(), Vec::new()),
    }
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
