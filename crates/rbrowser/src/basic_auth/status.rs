//! What Settings > Project > Environments and the shell's bar show (RB-094-29): per environment
//! whether a credential is stored, and its user - never the password. The answer to
//! `getBasicAuthStatus`, and pushed to every shell page after `environment.auth` stored or
//! removed one, so both show it at once. MCP has no tool for it (`environment.auth` is
//! `omitted(secrets)`).

use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore};

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
        store: super::store::kind(),
    }
}

/// Tells every shell page the status after a credential was stored or removed.
pub fn push(core: &Core) {
    core.send(&message(core));
}

/// Answers `getBasicAuthStatus`, and the sign-in dialog's `authPromptReply` (RB-099-04, the
/// one Basic auth entry of `crate::features`); `None` for every other message.
pub fn ipc(core: &Core, request: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    match request {
        ShellToCore::GetBasicAuthStatus => Some(Ok(message(core))),
        ShellToCore::AuthPromptReply(reply) => {
            Some(super::ask::asking().reply(core, service(), reply))
        }
        _ => None,
    }
}

/// Whether `message` reads or writes the credential store: `getBasicAuthStatus` and the sign-in
/// dialog's `authPromptReply` (a save). Keychain or Secret Service may wait for an unlock prompt,
/// so they never run on the CEF UI thread the query comes in on (review w35).
#[must_use]
pub fn is_off_thread(message: &ShellToCore) -> bool {
    matches!(
        message,
        ShellToCore::GetBasicAuthStatus | ShellToCore::AuthPromptReply(_)
    )
}

/// Answers `message` (see [`is_off_thread`]) on a blocking thread of the core's runtime.
pub fn dispatch_off_thread(
    core: &Core,
    message: ShellToCore,
    reply: impl FnOnce(Result<CoreToShell, Failure>) + Send + 'static,
) {
    let Some(core) = core.arc() else {
        reply(Err((
            IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        reply(ipc(&core, &message).unwrap_or_else(|| {
            Err((IpcErrorCode::BadRequest, "no Basic auth request".to_owned()))
        }));
    });
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
