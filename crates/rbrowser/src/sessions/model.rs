//! Which sessions exist, which one a view is in, and how a session is shown: the pure rules,
//! tested on every platform.

use rb_config::{DEFAULT_SESSION, ProjectConfig};

/// Why a session cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// Not a valid session id (`rb_config::is_valid_session_id`).
    #[error("`{0}` is not a valid session id")]
    InvalidId(String),
    /// Not `default` and not listed in `sessions`.
    #[error("session `{0}` is not listed in `sessions`")]
    Unknown(String),
}

/// The colours a session without one of its own gets: readable on light and dark strips.
pub const PALETTE: [u32; 8] = [
    0x00_8c_d2, // blue
    0xe5_48_4d, // red
    0x30_a4_6c, // green
    0xd9_7a_00, // amber
    0x8e_4e_c6, // purple
    0x00_9a_a0, // teal
    0xd6_40_9f, // pink
    0x6e_6e_6e, // grey
];

/// The sessions a view can be in: `default` first, then `sessions[]` in config order.
pub fn known_sessions(config: &ProjectConfig) -> Vec<String> {
    let mut known = vec![DEFAULT_SESSION.to_owned()];
    for session in &config.sessions {
        if !known.contains(session) {
            known.push(session.clone());
        }
    }
    known
}

/// `Ok` when `session` is `default` or listed in `sessions` (and a valid id).
pub fn require_known(config: &ProjectConfig, session: &str) -> Result<(), SessionError> {
    if !rb_config::is_valid_session_id(session) {
        return Err(SessionError::InvalidId(session.to_owned()));
    }
    if session == DEFAULT_SESSION || config.sessions.iter().any(|known| known == session) {
        Ok(())
    } else {
        Err(SessionError::Unknown(session.to_owned()))
    }
}

/// The session a view is in: the one chosen at runtime, else its breakpoint's, else `default`.
pub fn effective_session<'a>(chosen: Option<&'a str>, breakpoint: Option<&'a str>) -> &'a str {
    chosen.or(breakpoint).unwrap_or(DEFAULT_SESSION)
}

/// What a cell's label strip shows of a named session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionBadge {
    /// The session id.
    pub name: String,
    /// Its colour, `0xrrggbb`.
    pub rgb: u32,
}

impl SessionBadge {
    /// The strip's text: a dot (the icon) and the id.
    pub fn text(&self) -> String {
        format!("\u{25cf} {}", self.name)
    }
}

/// The badge of `session`; `None` for `default`, which shows none.
pub fn badge(config: &ProjectConfig, session: &str) -> Option<SessionBadge> {
    if session == DEFAULT_SESSION {
        return None;
    }
    Some(SessionBadge {
        name: session.to_owned(),
        rgb: rb_config::session_color(config, session).unwrap_or_else(|| auto_color(session)),
    })
}

/// A stable colour of [`PALETTE`] for `id` (FNV-1a over its bytes).
pub fn auto_color(id: &str) -> u32 {
    let hash = id.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    let index = usize::try_from(hash).unwrap_or(0) % PALETTE.len();
    PALETTE[index]
}

/// `0xrrggbb` as `#rrggbb`.
pub fn css_color(rgb: u32) -> String {
    format!("#{:06x}", rgb & 0x00ff_ffff)
}

/// `0xrrggbb` as opaque ARGB, as CEF's views take colours.
pub fn argb(rgb: u32) -> u32 {
    0xff00_0000 | (rgb & 0x00ff_ffff)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
