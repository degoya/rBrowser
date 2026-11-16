//! The message for what the user must see before any window exists - CEF did not start
//! (`crate::start_failure`) - on macOS (RB-090-15; the Windows twin is `../message_box.rs`, a
//! native message box). Not a dialog on macOS yet: the text goes to stderr; the caller has
//! logged it already, so the log and the diagnostics ZIP have it too.

/// Writes `title` and `text` to stderr.
pub fn error(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}
