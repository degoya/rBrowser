//! CEF's own log (RB-010-10, RB-099-07): its severity for our level and its size, held to the
//! core log's rotation before CEF starts and while the app runs (`rb_diag::cef_log`).

use std::path::Path;

use cef::LogSeverity;

/// Trims CEF's log at `path` to the size of `rotation` before CEF opens it, and starts the guard
/// that does so while the app runs (`rb_diag::cef_log`). A failure leaves the log as it is.
pub(super) fn keep_small(path: &Path, rotation: rb_diag::rotate::SharedRotation) {
    let Some(dir) = path.parent() else {
        return;
    };
    if let Err(error) =
        rb_diag::cef_log::trim(dir, rb_diag::paths::CEF_LOG_FILE_NAME, rotation.get())
    {
        tracing::warn!(%error, "CEF's log could not be trimmed");
    }
    if let Err(error) =
        rb_diag::cef_log::spawn_guard(dir.to_path_buf(), rotation, rb_diag::cef_log::CHECK_EVERY)
    {
        tracing::warn!(%error, "the guard of CEF's log did not start");
    }
}

/// CEF's own log severity for our level: quieter, errors only up to `info` (rb-diag).
pub(super) fn severity(level: rb_diag::LogLevel) -> LogSeverity {
    match rb_diag::CefLogSeverity::from(level) {
        rb_diag::CefLogSeverity::Verbose => LogSeverity::VERBOSE,
        rb_diag::CefLogSeverity::Info => LogSeverity::INFO,
        rb_diag::CefLogSeverity::Warning => LogSeverity::WARNING,
        rb_diag::CefLogSeverity::Error => LogSeverity::ERROR,
    }
}
