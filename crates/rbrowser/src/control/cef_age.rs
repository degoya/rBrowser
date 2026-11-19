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
/// The CEF build this binary was made with.
pub fn embedded_build() -> CefBuild {
    CefBuild {
        version: BUILD_VERSION.to_owned(),
        date: RELEASE_DATE.to_owned(),
    }
}

/// The warning for `build` on day `today` (days since the epoch) under the user's
/// `updates.cefAgeWarning`; `None` when the build is young enough, the notice is off, or updates
/// are checked by themselves (RB-090-14: the next release brings the new CEF).
pub fn notification(build: &CefBuild, today: i64, user: &UserSettings) -> Option<CoreToShell> {
    let notice = cef_age_notice(
        build,
        today,
        user.updates.cef_age_warning,
        crate::updates::checks_by_itself(crate::updates::SELF_UPDATE_BUILT, user),
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
