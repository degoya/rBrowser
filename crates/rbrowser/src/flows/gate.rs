//! The trust gate (K9, threat model T1): `.rbrowser/flows` is a protected folder, so a flow runs
//! only in a workspace a person trusted in the shell, in exactly the form trusted. The check is
//! made from the files and the store on every run - never from a cached flag - and once more
//! after the flow's text was read, so a file changed in between does not run on an older
//! decision.

use std::path::Path;

use rb_config::trust::{ProtectedKind, load_trust_store, trust_state};

use super::FlowError;

/// The hash of the protected parts when the project at `root` is trusted for flows by the store
/// at `store`.
///
/// # Errors
///
/// [`FlowError::Untrusted`] without a store, a decision for this form of the project, or when
/// the project or the store cannot be read.
pub fn trusted_hash(root: &Path, store: Option<&Path>) -> Result<String, FlowError> {
    let store = store
        .and_then(|path| load_trust_store(path).ok())
        .ok_or(FlowError::Untrusted)?;
    match trust_state(root, &store) {
        Ok(state) if state.allows(ProtectedKind::Flows) => Ok(state.hash),
        _ => Err(FlowError::Untrusted),
    }
}

/// Checks the trust of `root` again after a flow was read: the protected parts must still have
/// the hash `before`.
///
/// # Errors
///
/// [`FlowError::Untrusted`] when they changed or trust is gone.
pub fn still_trusted(root: &Path, store: Option<&Path>, before: &str) -> Result<(), FlowError> {
    if trusted_hash(root, store)? == before {
        Ok(())
    } else {
        Err(FlowError::Untrusted)
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
