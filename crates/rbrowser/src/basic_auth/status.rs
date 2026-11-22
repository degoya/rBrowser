//! What Settings > Project > Environments and the shell's bar show (RB-094-29): per environment
//! whether a credential is stored, and its user - never the password. The answer to
//! `getBasicAuthStatus`, and pushed to every shell page after `environment.auth` stored or
//! removed one, so both show it at once. MCP has no tool for it (`environment.auth` is
//! `omitted(secrets)`).

use rb_ipc::{CoreToShell, ShellToCore};

use super::service::service;
use crate::control::Core;
use crate::ipc::Failure;

/// The status of every environment of `core`'s project.
#[must_use]
pub fn message(core: &Core) -> CoreToShell {
    let host = core.host();
    let project = host
        .project_root()
        .map(|root| crate::instance::file::key(&root));
    CoreToShell::BasicAuthStatus {
        environments: service().statuses(project.as_deref(), &host.config()),
    }
}

/// Tells every shell page the status after a credential was stored or removed.
pub fn push(core: &Core) {
    core.send(&message(core));
}

/// Answers `getBasicAuthStatus`; `None` for every other message.
pub fn ipc(core: &Core, request: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    matches!(request, ShellToCore::GetBasicAuthStatus).then(|| Ok(message(core)))
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
