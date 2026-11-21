//! The last screenshots saved to the project's screenshot folder (RB-095-01): what MCP's
//! `rbrowser://screenshots/latest` lists and the files its URIs read (`crate::mcp::latest`). In
//! memory only, for the project they were taken in, without image data; a capture into a folder
//! of its own (a bug report's) and one answered inline (`save: false`) are not recorded.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rb_util::lock;

use super::ScreenshotAnswer;

/// The project root and the answer of the last capture that saved a file.
static LATEST: Mutex<Option<(PathBuf, ScreenshotAnswer)>> = Mutex::new(None);

/// Keeps `answer` as the last screenshots of `root` when it saved a file.
pub fn remember(root: &Path, answer: &ScreenshotAnswer) {
    if answer.paths().is_empty() {
        return;
    }
    let mut kept = answer.clone();
    for shot in kept.shots.iter_mut().chain(kept.overview.as_mut()) {
        shot.png_base64 = None;
    }
    *lock(&LATEST) = Some((root.to_path_buf(), kept));
}

/// The last screenshots saved in the project `root`.
#[must_use]
pub fn recall(root: &Path) -> Option<ScreenshotAnswer> {
    lock(&LATEST)
        .as_ref()
        .filter(|(at, _)| at == root)
        .map(|(_, answer)| answer.clone())
}

#[cfg(test)]
#[path = "latest_tests.rs"]
mod tests;
