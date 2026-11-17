//! The Videos panel's playback in the log (RB-110-12): the panel reports a video that does not
//! play, or plays only from memory, to the status bar (`reportStatus`); these reports also go to
//! the log the diagnostics ZIP carries, with the player's `MediaError` code and Chromium's text
//! (`DEMUXER_ERROR_COULD_NOT_OPEN: ...`, `PIPELINE_ERROR_NETWORK`). The route's own line per
//! request (`cef_app/video_scheme.rs`) says what was asked and answered; together they name the
//! cause. A report names a file, never a path.

use rb_ipc::Notification;

/// The panel's reports that are logged.
pub const LOGGED_KEYS: [&str; 4] = [
    "videos.error.play",
    "videos.error.mp4",
    "videos.error.too_large",
    "videos.fallback",
];
/// The most characters of a value the log keeps.
pub const LOGGED_CHARS: usize = 300;

/// A logged report's values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayReport {
    pub key: String,
    pub name: String,
    pub code: String,
    pub detail: String,
}

/// `value` for the log: control characters as `?`, cut to [`LOGGED_CHARS`], `-` when empty.
fn clean(value: Option<&String>) -> String {
    let text: String = value
        .map(String::as_str)
        .unwrap_or_default()
        .chars()
        .take(LOGGED_CHARS)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect();
    if text.is_empty() {
        "-".to_owned()
    } else {
        text
    }
}

/// What the log keeps of `report`, if it is one of the panel's playback reports.
#[must_use]
pub fn play_report(report: &Notification) -> Option<PlayReport> {
    if !LOGGED_KEYS.contains(&report.message_key.as_str()) {
        return None;
    }
    Some(PlayReport {
        key: report.message_key.clone(),
        name: clean(report.args.get("name")),
        code: clean(report.args.get("code")),
        detail: clean(report.args.get("detail")),
    })
}

/// Logs `report` at `info` when it is one of the panel's playback reports.
pub fn log(report: &Notification) {
    if let Some(play) = play_report(report) {
        tracing::info!(
            key = %play.key,
            file = %play.name,
            code = %play.code,
            detail = %play.detail,
            "the Videos panel reported a video"
        );
    }
}

#[cfg(test)]
#[path = "diagnose_tests.rs"]
mod tests;
