//! Where a session keeps its cookies, cache and storage: one folder per named session under the
//! instance's browser profile (CEF's `root_cache_path`), never shared with another session.

use std::path::{Path, PathBuf};

use rb_config::DEFAULT_SESSION;

use super::model::SessionError;

/// The folder of the named sessions inside the profile.
pub const SESSIONS_DIR: &str = "sessions";
/// The folder of the global context (`default`) inside the profile; set by `cef_app` as CEF's
/// `cache_path`.
pub const DEFAULT_CACHE_DIR: &str = "default";

/// The cache folder of `session` under `profile`: `None` for `default` (the global context),
/// `profile/sessions/<id>` for a named one. An id that is not a plain folder name is refused
/// (it comes from the project config or an MCP caller).
pub fn cache_path(profile: &Path, session: &str) -> Result<Option<PathBuf>, SessionError> {
    if session == DEFAULT_SESSION {
        return Ok(None);
    }
    if !rb_config::is_valid_session_id(session) {
        return Err(SessionError::InvalidId(session.to_owned()));
    }
    let path = profile.join(SESSIONS_DIR).join(session);
    // The id is one plain segment, so this holds; kept as the last guard.
    if path.parent() != Some(profile.join(SESSIONS_DIR).as_path()) {
        return Err(SessionError::InvalidId(session.to_owned()));
    }
    Ok(Some(path))
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
