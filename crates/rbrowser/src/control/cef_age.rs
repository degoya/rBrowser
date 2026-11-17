//! The notice about an old CEF build (RB-010-10): `build.rs` embeds the pinned build and its
//! release date from `scripts/lib/tool-versions.sh`; at startup the core turns
//! [`rb_diag::cef_age_notice`] into a notification that waits for the shell.

use rb_config::UserSettings;
use rb_diag::{CefBuild, cef_age_notice};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};

/// The release date of the pinned CEF build (`RB_CEF_RELEASE_DATE`, `YYYY-MM-DD`).
const RELEASE_DATE: &str = env!("RB_CEF_RELEASE_DATE");
/// The pinned CEF build's version, `156.0.26+g...+chromium-...` (from `RB_CEF_ARCHIVE`).
const BUILD_VERSION: &str = env!("RB_CEF_BUILD_VERSION");
/// The self-update is prepared but off in phase 1 (RB-090-02), so the notice is never silenced
/// by it.
const SELF_UPDATE_ACTIVE: bool = false;

/// The CEF build this binary was made with.
pub fn embedded_build() -> CefBuild {
    CefBuild {
        version: BUILD_VERSION.to_owned(),
        date: RELEASE_DATE.to_owned(),
    }
}

/// The warning for `build` on day `today` (days since the epoch) under the user's
/// `updates.cefAgeWarning`; `None` when the build is young enough or the notice is off.
pub fn notification(build: &CefBuild, today: i64, user: &UserSettings) -> Option<CoreToShell> {
    let notice = cef_age_notice(
        build,
        today,
        user.updates.cef_age_warning,
        SELF_UPDATE_ACTIVE,
    )?;
    Some(CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: notice.message_key,
        args: notice.args,
        settings_key: notice.settings_key,
    }))
}

#[cfg(test)]
#[path = "cef_age_tests.rs"]
mod tests;
