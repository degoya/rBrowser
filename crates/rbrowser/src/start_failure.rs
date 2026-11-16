//! Why CEF did not start: `CefInitialize` returned false and `cef_get_exit_code` names the
//! reason (`cef_resultcode_t`). The app logs it and tells the user in a message box instead of
//! ending silently; the process exits with CEF's code.

use std::path::Path;

/// `CEF_RESULT_CODE_PROFILE_IN_USE`.
const PROFILE_IN_USE: i32 = 21;
/// `CEF_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED`: another process holds the profile.
const PROCESS_NOTIFIED: i32 = 24;
/// `CEF_RESULT_CODE_INVALID_SANDBOX_STATE`.
const INVALID_SANDBOX_STATE: i32 = 31;
/// `CEF_RESULT_CODE_NORMAL_EXIT_AUTO_DE_ELEVATED`: Chromium refuses to run as administrator.
const AUTO_DE_ELEVATED: i32 = 38;

/// A failed CEF start with CEF's exit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartFailure {
    code: i32,
}

impl StartFailure {
    /// The failure with CEF's exit code `code` (`cef_get_exit_code`).
    #[must_use]
    pub fn new(code: i32) -> Self {
        Self { code }
    }

    /// The i18n key of the reason.
    #[must_use]
    pub fn reason_key(self) -> &'static str {
        match self.code {
            AUTO_DE_ELEVATED => "startup.cefFailed.elevated",
            PROFILE_IN_USE | PROCESS_NOTIFIED => "startup.cefFailed.profileInUse",
            INVALID_SANDBOX_STATE => "startup.cefFailed.sandbox",
            _ => "startup.cefFailed.other",
        }
    }

    /// The process's exit code: CEF's, or 1 when it gave none.
    #[must_use]
    pub fn exit_code(self) -> i32 {
        if self.code > 0 { self.code } else { 1 }
    }

    /// The text for the log and the message box: the reason, the code and where the log is.
    #[must_use]
    pub fn message(self, profile: &Path, logs: Option<&Path>) -> String {
        let profile = profile.display().to_string();
        let code = self.code.to_string();
        let reason = rb_i18n::t_with(self.reason_key(), &[("profile", &profile), ("code", &code)]);
        let detail = match logs {
            Some(logs) => rb_i18n::t_with(
                "startup.cefFailed.detail",
                &[("code", &code), ("log", &logs.display().to_string())],
            ),
            None => rb_i18n::t_with("startup.cefFailed.detailNoLog", &[("code", &code)]),
        };
        format!("{reason}\n\n{detail}")
    }
}

/// The title of the message box.
#[must_use]
pub fn title() -> String {
    rb_i18n::t("startup.cefFailed.title")
}

#[cfg(test)]
#[path = "start_failure_tests.rs"]
mod tests;
