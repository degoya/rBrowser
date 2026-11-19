//! A notice's arguments as a person reads them (RB-090-85): an error passed into a text by its
//! `Display` (`{error}`) often starts with its stable code for MCP and the log
//! (`sitemap.error.busy: a sitemap capture is already running`); the status bar, its history and
//! MCP's `status_messages` show the sentence without the code in front. The code stays in MCP's
//! own answers and in the log.
//!
//! A code is `<area>.error.<name>` followed by `": "`: segments of ASCII letters, digits, `_`
//! and `-`, the first starting with a lower-case letter. Nothing else is touched - a host
//! (`example.com: ...`), a path or a file name keeps its text.

use rb_ipc::Notification;

/// Drops the error code in front of every argument of `notification`.
pub fn strip_codes(notification: &mut Notification) {
    for value in notification.args.values_mut() {
        if let Some(rest) = without_code(value) {
            *value = rest.to_owned();
        }
    }
}

/// The text after a leading `<area>.error.<name>: `, `None` without one (or with nothing
/// after it).
fn without_code(value: &str) -> Option<&str> {
    let (head, rest) = value.split_once(": ")?;
    let segments: Vec<&str> = head.split('.').collect();
    let well_formed = segments.iter().all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    });
    let starts_lower = head.chars().next().is_some_and(|c| c.is_ascii_lowercase());
    let is_code = segments.len() >= 3 && segments[1] == "error" && well_formed && starts_lower;
    (is_code && !rest.trim().is_empty()).then_some(rest)
}

#[cfg(test)]
#[path = "readable_tests.rs"]
mod tests;
