//! The size limit of a plugin panel's messages (RB-090-44): `plugins.panelMessageMaxBytes`,
//! 4 MiB by default (64 KiB to 16 MiB), apart from the page scripts' 64 KiB
//! ([`rb_plugin_host::MAX_MESSAGE_BYTES`]), which carry data from untrusted pages. It holds a
//! panel call's arguments ([`super::panels::checked_call`]) and its answer
//! ([`super::runtime::PluginRuntime::panel_call_tool`]); the shell holds the same value in
//! `ui/src/panels/bridge.ts`, in both directions, and gets it with a panel's settings.
//!
//! Memory: the shell lets at most 16 calls of one plugin's panels wait (`MAX_PENDING` of
//! `bridge.ts`), and the component answers them one after the other, each at most
//! `rb_plugin_api::MAX_JSON_CHARS` (4 MiB) - so one plugin's panels hold at most 16 answers of
//! the limit in flight (64 MiB at the default, 256 MiB at 16 MiB); the page requests the core
//! counts (`MAX_PENDING`, 32) stay at 64 KiB each.

use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

/// The setting the limit comes from.
pub const SETTING: &str = "plugins.panelMessageMaxBytes";
/// The `reason` of a panel message over the limit: its text names [`SETTING`]
/// (`plugins.reason.panel_message_limit`).
pub const REASON: &str = "panel_message_limit";

/// `plugins.error.too_large` for a panel message over `limit` bytes, naming the setting.
#[must_use]
pub fn too_large(limit: usize) -> Failure {
    Failure::coded(FailureKind::Limit, codes::TOO_LARGE)
        .with_param("limit", limit)
        .with_param("reason", REASON)
        .with_param("setting", SETTING)
}

/// `answer` when its JSON text (UTF-8 bytes) fits `limit`.
///
/// # Errors
///
/// [`too_large`] when it does not.
pub fn bounded(answer: Value, limit: usize) -> Result<Value, Failure> {
    if answer.to_string().len() > limit {
        Err(too_large(limit))
    } else {
        Ok(answer)
    }
}

#[cfg(test)]
#[path = "panel_limit_tests.rs"]
mod tests;
