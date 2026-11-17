//! Self-update in the app (RB-090-02): prepared and inactive until the move to GitHub
//! (RB-110-05).
//!
//! Two switches, both off in phase 1: the Cargo feature `self-update` of this crate (without it
//! `rb-update` is not even linked, so the binary holds no update code and no update address) and
//! the protected setting `updates.enabled`. Without the feature the settings window shows
//! General > Updates with every update key locked and the hint [`INACTIVE_KEY`], and a write to
//! one of [`LOCKED_KEYS`] is refused ([`is_locked`]); `updates.cefAgeWarning` (RB-010-10) stays
//! active. With the feature the start proves a switched update and, only with
//! `updates.enabled` on and `updates.check` at `onStart`, checks the channel (`active.rs`);
//! offering and installing are RB-110-05's.
//!
//! No command, no shortcut and no MCP tool yet: the update commands arrive with RB-110-05 and
//! are `omitted("protected: updating the app is a human decision")` in the MCP coverage until
//! then.

use rb_config::UserSettings;

use crate::control::Core;

#[cfg(feature = "self-update")]
mod active;

/// Whether this build carries self-update: the Cargo feature `self-update`.
pub const SELF_UPDATE_BUILT: bool = cfg!(feature = "self-update");

/// The keys the window locks and a write refuses while the feature is not built.
pub const LOCKED_KEYS: &[&str] = &[
    "updates.enabled",
    "updates.channel",
    "updates.check",
    "updates.autoInstall",
];

/// The hint under a locked key, and the reason a write to one is refused.
pub const INACTIVE_KEY: &str = "settings.updates.inactive";

/// The MCP coverage entry of the update commands (RB-110-05), until they get tools.
pub const MCP_OMITTED_REASON: &str = "protected: updating the app is a human decision";

/// Whether `key` is locked in this build.
#[must_use]
pub fn is_locked(key: &str) -> bool {
    locked_in(SELF_UPDATE_BUILT, key)
}

/// [`is_locked`] for a build with or without the feature.
#[must_use]
pub fn locked_in(built: bool, key: &str) -> bool {
    !built && LOCKED_KEYS.contains(&key)
}

/// The gate `rb-update` checks before it reaches the network: open only in a build with the
/// feature and with `updates.enabled` on.
#[must_use]
pub fn gate_open(built: bool, settings: &UserSettings) -> bool {
    built && settings.updates.enabled
}

/// The core is up: with the feature and `updates.enabled` on, what `active.rs` does; otherwise
/// nothing at all - no request, no file, nothing spawned.
pub fn on_start(core: &Core) {
    #[cfg(feature = "self-update")]
    if gate_open(SELF_UPDATE_BUILT, &core.user_settings()) {
        active::on_start(core);
    }
    #[cfg(not(feature = "self-update"))]
    let _ = core;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
