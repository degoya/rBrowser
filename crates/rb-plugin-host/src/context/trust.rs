//! Workspace trust for `project.read` and `project.list` (RB-070-04), and for every
//! `project-write` (RB-110-13), which writes nothing into an untrusted project: the folders workspace trust
//! hashes - `.rbrowser/{flows,snippets,plugins,mocks,scripts}/`, code a project checks in - are
//! read by no plugin while the open workspace is untrusted, a bundled one included, so a plugin
//! that runs such code (the `snippets` plugin) cannot be handed it before a person trusted the
//! project. The folders are `rb_config::trust::PROTECTED_DIRS`, the list the trust hash covers.

use rb_plugin_types::{Failure, FailureKind, codes, paths};

/// Whether the project-relative `segments` lie in (or are) a trust-protected folder.
#[must_use]
pub(crate) fn protected(segments: &[&str]) -> bool {
    // In any case: Windows opens `.RBROWSER/Snippets` as `.rbrowser/snippets`.
    segments
        .first()
        .is_some_and(|first| paths::is_config_dir(first))
        && segments.get(1).is_some_and(|dir| {
            rb_config::trust::PROTECTED_DIRS
                .iter()
                .any(|(_, protected)| paths::same_name(protected, dir))
        })
}

/// `plugins.error.untrusted_workspace` (naming `path`) for a protected path while `trusted` is
/// false.
///
/// # Errors
///
/// The refusal.
pub(crate) fn check(segments: &[&str], trusted: bool, path: &str) -> Result<(), Failure> {
    if trusted || !protected(segments) {
        return Ok(());
    }
    Err(untrusted(path))
}

/// `plugins.error.untrusted_workspace` (naming `path`) whatever the path, while `trusted` is
/// false: `project-write` writes nothing into an untrusted project (RB-110-13).
///
/// # Errors
///
/// The refusal.
pub(crate) fn require(trusted: bool, path: &str) -> Result<(), Failure> {
    if trusted {
        Ok(())
    } else {
        Err(untrusted(path))
    }
}

fn untrusted(path: &str) -> Failure {
    Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)
        .with_param("path", path.chars().take(200).collect::<String>())
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
