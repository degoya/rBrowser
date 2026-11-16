//! Finding the project root: the first directory, from a start path upwards, that holds
//! `.rbrowser/config.json`.
//!
//! Pure `std::path` and `std::fs`, so a `\\wsl.localhost\<distro>\...` path (9P) works like a
//! local one: `std::path::absolute` keeps the UNC prefix (no `\\?\` form as `canonicalize`
//! would produce) and the walk stops at the share root.

use std::path::{Path, PathBuf};

use crate::{CONFIG_DIR, CONFIG_FILE};

/// The project root at or above `start` (a directory or a file inside the project).
pub fn discover(start: &Path) -> Option<PathBuf> {
    let start = std::path::absolute(start).ok()?;
    start
        .ancestors()
        .find(|dir| dir.join(CONFIG_DIR).join(CONFIG_FILE).is_file())
        .map(Path::to_path_buf)
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod tests;
