//! A refusal with a stable code: why a manifest, a package or a contribution is not taken.
//!
//! Adapted from rCommander's `Refusal` and `refusal_code` (`crates/rc-plugin-host/src/
//! verifier.rs`): the code travels inside an `anyhow` chain, so every check can say why in
//! English for the log and the caller still finds the one code the shell translates.

use rb_plugin_types::{Failure, FailureKind, codes};

/// A refusal nobody can override, with its code (an i18n key of [`rb_plugin_types::codes`]).
#[derive(Debug, thiserror::Error)]
#[error("{reason}")]
pub struct Refusal {
    pub code: &'static str,
    /// English, for the log and the developer; never shown as the translated text.
    pub reason: String,
}

/// `reason` refused under `code`, as an `anyhow` error.
pub(crate) fn refuse(code: &'static str, reason: impl Into<String>) -> anyhow::Error {
    Refusal {
        code,
        reason: reason.into(),
    }
    .into()
}

/// The code of the [`Refusal`] in `error`'s chain; `plugins.error.invalid` for any other failure
/// (an unreadable archive, a manifest that does not parse).
#[must_use]
pub fn refusal_code(error: &anyhow::Error) -> &'static str {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<Refusal>())
        .map_or(codes::INVALID, |refusal| refusal.code)
}

/// `error` as the failure a caller gets: its code, the English chain as `reason`.
#[must_use]
pub fn refusal_failure(error: &anyhow::Error) -> Failure {
    let kind = match refusal_code(error) {
        codes::UNTRUSTED_WORKSPACE | codes::KEY_UNTRUSTED | codes::DENIED => {
            FailureKind::AccessDenied
        }
        codes::SIGNATURE_INVALID | codes::KEY_MISMATCH | codes::UNSIGNED => {
            FailureKind::AccessDenied
        }
        _ => FailureKind::BadData,
    };
    Failure::coded(kind, refusal_code(error)).with_param(
        "reason",
        crate::failure::readable(&format!("{error:#}"), 500),
    )
}
