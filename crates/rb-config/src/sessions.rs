//! The rules of named sessions (RB-020-07) a schema cannot say. A session id becomes the name of
//! a folder under the instance's browser profile (`profiles\<project>\sessions\<id>`), so it is
//! held to a plain, lowercase ASCII form that cannot leave that folder, name a Windows device or
//! collide with another id on a case-insensitive file system.
//!
//! `sessionColors` (`{ "<session>": "#rrggbb" }`) is [`ProjectConfig::session_colors`]; an absent
//! colour is chosen by the app.

use std::collections::HashSet;

use rb_util::json_pointer::escape;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validate::DEFAULT_SESSION;
use crate::{ConfigIssue, ProjectConfig};

/// `sessions` in the user settings: what the person decides about every session's data
/// (batch-2 security review of RB-020-07).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionSettings {
    /// Session cookies (logins without an expiry) are written to disk and survive a restart, in
    /// every session. Off, they end with the app as in a browser. Read when rBrowser starts.
    pub persist_session_cookies: bool,
    /// `session.state_export` writes the cookies and storage of every domain in the session's
    /// jar. Off, only those of the project's environments and start URL (RB-094-08). A login
    /// state file is a secret; only a person widens it.
    pub state_export_all_domains: bool,
}

/// The longest session id.
pub const SESSION_ID_MAX_LEN: usize = 32;
/// The config key of the session colours.
pub const SESSION_COLORS_KEY: &str = "sessionColors";

/// Windows device names a folder must not have (with or without an extension).
const DEVICE_NAMES: &[&str] = &["con", "prn", "aux", "nul"];

/// Whether `id` is a valid session id: 1 to 32 characters of `a-z`, `0-9`, `-` and `_`,
/// starting with a letter or digit, and no Windows device name (`con`, `com1`, ...).
pub fn is_valid_session_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    if bytes.len() > SESSION_ID_MAX_LEN || !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    if !bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_')
    {
        return false;
    }
    !is_device_name(id)
}

fn is_device_name(id: &str) -> bool {
    if DEVICE_NAMES.contains(&id) {
        return true;
    }
    ["com", "lpt"].iter().any(|prefix| {
        id.strip_prefix(prefix)
            .is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit())
    })
}

/// `#rrggbb` as `0xrrggbb`; `None` for anything else.
pub fn parse_hex_color(text: &str) -> Option<u32> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// Every problem of `sessions` and `sessionColors`: an invalid or repeated id, a colour that is
/// not `#rrggbb`, a colour for a session that is not listed (`default` has no colour).
pub fn check_sessions(config: &ProjectConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for (index, id) in config.sessions.iter().enumerate() {
        let path = format!("/sessions/{index}");
        if !is_valid_session_id(id) {
            issues.push(issue(
                path,
                "config.error.invalid_session_id",
                format!(
                    "session `{}` must be 1 to {SESSION_ID_MAX_LEN} characters of a-z, 0-9, - and _",
                    id.escape_debug()
                ),
            ));
        } else if !seen.insert(id.as_str()) {
            issues.push(issue(
                path,
                "config.error.duplicate_id",
                format!("session `{id}` is listed more than once"),
            ));
        }
    }
    // A `BTreeMap`: the issues come in session order.
    for (session, color) in &config.session_colors {
        let path = format!("/{SESSION_COLORS_KEY}/{}", escape(session));
        if session != DEFAULT_SESSION && !config.sessions.contains(session) {
            issues.push(issue(
                path,
                "config.error.unknown_session",
                format!(
                    "session `{}` is not listed in `sessions`",
                    session.escape_debug()
                ),
            ));
        } else if session == DEFAULT_SESSION || parse_hex_color(color).is_none() {
            issues.push(issue(
                path,
                "config.error.session_color",
                "a session colour is `#rrggbb`; `default` has none".to_owned(),
            ));
        }
    }
    issues
}

/// The colour `config` gives `session`, as `0xrrggbb`; `None` when it gives none.
pub fn session_color(config: &ProjectConfig, session: &str) -> Option<u32> {
    config
        .session_colors
        .get(session)
        .and_then(|color| parse_hex_color(color))
}

fn issue(path: String, key: &str, message: String) -> ConfigIssue {
    ConfigIssue {
        path,
        key: key.to_owned(),
        message,
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
