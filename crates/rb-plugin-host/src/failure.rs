//! The failures the host reports for a plugin and what a stopped call becomes.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/failure.rs` (itself from
//! rDownloader's `component/convert.rs`): the same reading of a trap - fuel, the epoch, an
//! `unreachable`, a refused memory growth - into a stable code, now the i18n keys of
//! [`rb_plugin_types::codes`].

use rb_plugin_types::{Failure, FailureKind, codes};

/// Longest reason kept from a trap, in characters.
const MAX_REASON_CHARS: usize = 300;

/// A failure with a host code.
pub(crate) fn coded(kind: FailureKind, code: &str) -> Failure {
    Failure::coded(kind, code)
}

/// The host could not set a call up; `detail` goes to the log, not to the caller.
pub(crate) fn host_failed(detail: impl std::fmt::Display) -> Failure {
    tracing::warn!(%detail, "a plugin call could not be set up");
    coded(FailureKind::Other, codes::HOST_FAILED)
}

/// The plugin's answer broke a rule; `what` names which.
pub(crate) fn bad_output(what: &str) -> Failure {
    coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("what", what)
}

/// A size bound was hit; `limit` names it.
pub(crate) fn too_large(limit: usize) -> Failure {
    coded(FailureKind::Limit, codes::TOO_LARGE).with_param("limit", limit)
}

/// What a call that did not return becomes: `error` is Wasmtime's, `None` the caller's wall
/// clock.
pub(crate) fn stopped(error: Option<&wasmtime::Error>) -> Failure {
    let Some(error) = error else {
        return coded(FailureKind::Limit, codes::TIMED_OUT);
    };
    let (kind, code, reason) = describe(error);
    coded(kind, code).with_param("reason", reason)
}

/// A stable code and one line: the trap reason matters to the person, the backtrace does not.
fn describe(error: &wasmtime::Error) -> (FailureKind, &'static str, String) {
    if let Some(trap) = error.downcast_ref::<wasmtime::Trap>() {
        return match trap {
            wasmtime::Trap::OutOfFuel => (
                FailureKind::Limit,
                codes::OUT_OF_FUEL,
                "compute budget (fuel) exhausted".to_owned(),
            ),
            wasmtime::Trap::Interrupt => (
                FailureKind::Limit,
                codes::TIMED_OUT,
                "time limit exceeded".to_owned(),
            ),
            wasmtime::Trap::UnreachableCodeReached => (
                FailureKind::Other,
                codes::CRASHED,
                "crashed (panic or unreachable)".to_owned(),
            ),
            other => (
                FailureKind::Other,
                codes::CRASHED,
                readable(&other.to_string(), MAX_REASON_CHARS),
            ),
        };
    }
    let reason = readable(&error.root_cause().to_string(), MAX_REASON_CHARS);
    // A refused memory growth (`StoreLimits::trap_on_grow_failure`) is an error of the limiter,
    // not a `Trap`, so what it says is the only way to tell it apart (rDownloader's reading).
    if reason.to_ascii_lowercase().contains("memory") {
        (FailureKind::Limit, codes::OUT_OF_MEMORY, reason)
    } else {
        (FailureKind::Other, codes::CRASHED, reason)
    }
}

/// `text` clipped to `limit` characters, with control, zero-width and direction characters
/// spelled out (`\n`, `\u{202e}`): a log line shows what the plugin sent, and a plugin cannot
/// forge a line of rBrowser's or hide text in it.
#[must_use]
pub fn readable(text: &str, limit: usize) -> String {
    let mut shown = String::with_capacity(text.len().min(limit + 16));
    for (kept, c) in text.chars().enumerate() {
        if kept == limit {
            shown.push('\u{2026}');
            break;
        }
        if matches!(c, '\r' | '\n' | '\t') {
            shown.extend(c.escape_default());
        } else if invisible(c) {
            shown.extend(c.escape_unicode());
        } else {
            shown.push(c);
        }
    }
    shown
}

/// Control, zero-width and bidirectional formatting characters.
fn invisible(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
                | '\u{061c}'
        )
}

#[cfg(test)]
#[path = "failure_tests.rs"]
mod tests;
