//! The file of the last pages (RB-092-04): `last-pages.json` in the project's browser profile
//! (`%LOCALAPPDATA%\rBrowser\profiles\<project>`, `crate::project::profile_dir`) - never in the
//! project's `config.json` or `config.local.json`, and outside the folders the diagnostics ZIP
//! reads (the logs and the crash dumps). Written through a temporary file renamed over it, so
//! a crash never leaves half a file. The URLs it holds may carry tokens: no error and no log
//! line names them.

use std::path::{Path, PathBuf};

use super::model::{self, LastPages};

/// The file's name inside the profile.
pub const FILE_NAME: &str = "last-pages.json";

/// The file of the profile `profile`.
pub fn file(profile: &Path) -> PathBuf {
    profile.join(FILE_NAME)
}

/// The pages kept in `profile`; `None` without the file or when it is not readable as the
/// format (said in the log without its content).
pub fn read(profile: &Path) -> Option<LastPages> {
    let path = file(profile);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(kind = %error.kind(), "the last pages cannot be read");
            return None;
        }
    };
    let pages = model::parse(&text);
    if pages.is_none() {
        tracing::warn!("the file of the last pages is not readable; it is replaced");
    }
    pages
}

/// Writes `pages` into `profile`, replacing what was there. The error names the file and the
/// cause, never a URL.
pub fn write(profile: &Path, pages: &LastPages) -> Result<(), String> {
    rb_config::write_json(&file(profile), &model::to_value(pages))
        .map_err(|error| rb_diag::redact(&error.to_string()))
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
