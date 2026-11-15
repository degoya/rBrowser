//! Sessions (RB-020-07): `default` is the one cookie and storage context every Chromium view
//! shares; each named session of the project config (`sessions`, `breakpoints[].session`) has a
//! CEF request context of its own, with its cache, cookies and storage in
//! `<profile>\sessions\<id>` under the instance's `root_cache_path`, so they survive a restart
//! and never mix with another session's.
//!
//! | Part | Where |
//! | --- | --- |
//! | Known sessions, the view's session, the label badge and its colour | [`model`] |
//! | The cache folder of a session | [`paths`] |
//! | Moving views into a session, clearing a session's data | `crate::host` (`host/sessions.rs`) |
//! | What "clear session data" clears | [`clear`] |
//! | The leader-per-session rule for synced submits | [`leader`] |
//! | The SyncHub's submit policy over that rule (`rb_sync::SubmitPolicy`, RB-020-02) | [`policy`] |
//! | Commands `session.assign` (`Leader E S`), `session.clear` (`Leader E K`) | [`defs`] |
//! | MCP tools `session_assign`, `session_clear` | [`mcp`] |
//! | The request contexts, the cookie deletion, the badge view (Windows) | `cef` |
//!
//! Sessions come from the project config, which may be untrusted: they only separate cookies
//! and storage inside the instance's own profile and grant nothing, so they are not gated by
//! workspace trust; their ids are held to plain folder names (`rb_config::is_valid_session_id`)
//! and checked again before a folder is derived.

pub mod clear;
pub mod defs;
pub mod leader;
pub mod mcp;
pub mod model;
pub mod paths;
pub mod policy;

#[cfg(any(windows, target_os = "macos"))]
pub mod cef;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use rb_commands::CommandError;
use rb_engine::{EngineError, Target, ViewHost};
use rb_ipc::CoreToShell;
use serde_json::Value;

use crate::control::Core;

pub use clear::{STORAGE_TYPES, origins_to_clear};
pub use defs::{ASSIGN, CLEAR, register};
pub use leader::{SubmitRule, submit_views};
pub use mcp::{AssignAnswer, AssignArgs, ClearAnswer, ClearArgs, Moved};
pub use model::{
    PALETTE, SessionBadge, SessionError, argb, auto_color, badge, css_color, effective_session,
    known_sessions, require_known,
};
pub use paths::{DEFAULT_CACHE_DIR, SESSIONS_DIR, cache_path};

/// The instance's browser profile (CEF's `root_cache_path`), set once before CEF starts.
static PROFILE: OnceLock<PathBuf> = OnceLock::new();
/// `sessions.persistSessionCookies` as read at the start; off by default.
static PERSIST_COOKIES: AtomicBool = AtomicBool::new(false);

/// Records the instance's browser profile; the first call wins.
pub fn set_profile(profile: PathBuf) {
    if PROFILE.set(profile).is_err() {
        tracing::debug!("the browser profile was set already");
    }
}

/// Records `sessions.persistSessionCookies` before CEF starts: the global context and every
/// session's context are created with it.
pub fn set_persist_session_cookies(persist: bool) {
    PERSIST_COOKIES.store(persist, Ordering::Release);
}

/// Whether session cookies are written to disk and survive a restart (the user's choice; off,
/// they end with the app).
pub fn persist_session_cookies() -> bool {
    PERSIST_COOKIES.load(Ordering::Acquire)
}

/// The instance's browser profile, once set.
pub fn profile() -> Option<&'static Path> {
    PROFILE.get().map(PathBuf::as_path)
}

/// Runs `session.assign` or `session.clear` (registered by [`register`]). Without a `session`
/// argument (a shortcut) the shell opens its picker or its confirmation and runs the command
/// again with one.
pub fn run(core: &Core, id: &str, args: &Value) -> Result<Value, CommandError> {
    if args.get("session").is_none() {
        // The session bar sits in the toolbar strip: the shell takes the keyboard.
        core.ui().focus_shell();
        core.send(&CoreToShell::UiCommand { id: id.to_owned() });
        return Ok(Value::Null);
    }
    let host = core.host();
    let answer = match id {
        ASSIGN => {
            let args: AssignArgs = parse(args)?;
            let target = match args.target {
                Some(target) => target,
                None => Target::View(
                    host.active_view()
                        .or_else(|| host.views().first().map(|view| view.id))
                        .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))?,
                ),
            };
            let moved = host.assign_session(target, &args.session).map_err(failed)?;
            serde_json::to_value(AssignAnswer {
                session: args.session,
                moved,
            })
        }
        CLEAR => {
            let args: ClearArgs = parse(args)?;
            let plan = host.clear_session(&args.session).map_err(failed)?;
            serde_json::to_value(ClearAnswer {
                session: args.session,
                views: plan.views,
                origins: plan.origins,
            })
        }
        other => return Err(CommandError::UnknownCommand(other.to_owned())),
    };
    answer.map_err(|error| CommandError::Failed(error.to_string()))
}

/// The sessions as the shell lists them: `default` first, each named one with its colour.
pub fn session_infos(config: &rb_config::ProjectConfig) -> Vec<rb_ipc::SessionInfo> {
    known_sessions(config)
        .into_iter()
        .map(|id| rb_ipc::SessionInfo {
            color: badge(config, &id).map(|badge| css_color(badge.rgb)),
            id,
        })
        .collect()
}

/// The config was read again: views whose session was not chosen by hand follow their
/// breakpoint's `session`.
pub fn on_config_changed(core: &Core) {
    if let Err(error) = core.host().follow_config_sessions() {
        tracing::warn!(%error, "the views did not follow the config's sessions");
    }
}

fn parse<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, CommandError> {
    serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))
}

fn failed(error: EngineError) -> CommandError {
    match error {
        EngineError::Invalid(message) => CommandError::InvalidArgs(message),
        other => CommandError::Failed(other.to_string()),
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
