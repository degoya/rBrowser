//! `session.*` and `browser.*`: the session, the event subscription, the version check, the
//! outer window and quitting.

use rb_engine::external::WindowRect;
use serde_json::{Value, json};

use crate::error::BidiError;
use crate::protocol::Command;

/// The oldest Firefox major version rBrowser drives: per-context preload scripts, the
/// `emulation.*` overrides and `browser.setClientWindowState` (ADR 0001, RB-000-06 ran 159).
pub const MIN_FIREFOX_MAJOR: u32 = 140;

/// A new session. Certificates are checked as in any Firefox; prompts stay for the user.
pub fn session_new() -> Command {
    Command::call(
        "session.new",
        json!({ "capabilities": { "alwaysMatch": {
            "acceptInsecureCerts": false,
            "unhandledPromptBehavior": { "default": "ignore" },
        }}}),
    )
}

/// Subscribes to `events` in every context.
pub fn subscribe(events: &[&str]) -> Command {
    Command::call("session.subscribe", json!({ "events": events }))
}

/// Moves and sizes the window `client_window` (logical px of the desktop).
pub fn set_client_window_state(client_window: &str, rect: WindowRect) -> Command {
    Command::call(
        "browser.setClientWindowState",
        json!({
            "clientWindow": client_window,
            "state": "normal",
            "x": rect.x,
            "y": rect.y,
            "width": rect.width,
            "height": rect.height,
        }),
    )
}

/// Quits Firefox.
pub fn close() -> Command {
    Command::call("browser.close", json!({}))
}

/// `browserVersion` of a `session.new` answer.
pub fn browser_version(result: &Value) -> Option<String> {
    result
        .get("capabilities")?
        .get("browserVersion")?
        .as_str()
        .map(str::to_owned)
}

/// [`BidiError::TooOld`] when `version`'s major number is below `min`. A version without a
/// leading number passes: Firefox will say what it cannot do.
pub fn check_version(version: &str, min: u32) -> Result<(), BidiError> {
    let digits: String = version.chars().take_while(char::is_ascii_digit).collect();
    match digits.parse::<u32>() {
        Ok(major) if major < min => Err(BidiError::TooOld {
            found: version.to_owned(),
            min,
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "browser_tests.rs"]
mod tests;
