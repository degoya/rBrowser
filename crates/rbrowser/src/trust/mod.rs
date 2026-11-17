//! Workspace trust in the app (RB-010-09): the check when a project opens or its config
//! changes (`rb_config::trust`), the prompt and the decision (`trustPrompt`,
//! `trustDecision`), revoking, and [`crate::host::shell_state::ShellState::trusted`] that every
//! protected part reads. Only the shell's dialog grants trust; no command, setting or MCP tool
//! does: `trust.grant` opens the dialog, `trust.*` have no MCP tool, and no settings key holds
//! a decision (they live in `trust.json` beside the user settings).
//!
//! The project config loads through [`load_config`], so the trust-gated network keys apply
//! only to a trusted workspace; they are switches of the browser process and change at the
//! next start. The plugin loader asks [`ProjectTrust`].

pub(crate) mod grants;
mod state;
mod workspace;

use std::path::Path;
use std::sync::Arc;

use rb_commands::CommandError;
use rb_config::trust::{load_project_trusted, trust_store_path};
use rb_config::{ConfigError, LoadedConfig};
use rb_ipc::{CoreToShell, ShellToCore};
use serde_json::Value;

use crate::control::Core;
use crate::host::Host;
use crate::ipc::Failure;

pub use state::{RESTART_NOTICE_KEY, rfc3339, store_path};
pub use workspace::ProjectTrust;

/// Runs the command `id` when it is this module's (`trust.*`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let _ = args;
    let result = match id {
        "trust.show" => {
            open_dialog(core);
            Ok(Value::Null)
        }
        // Opens the dialog with a fresh prompt; only the dialog's button trusts.
        "trust.grant" => match state::current(core) {
            Some((root, state)) => {
                state::prompt_on_request(core, &root, &state);
                open_dialog(core);
                Ok(Value::Null)
            }
            None => Err(no_project()),
        },
        "trust.revoke" => match core.host().project_root() {
            Some(root) => state::revoke(core, &root)
                .map(|()| Value::Null)
                .map_err(|(_, message)| CommandError::Failed(message)),
            None => Err(no_project()),
        },
        _ => return None,
    };
    Some(result)
}

fn no_project() -> CommandError {
    CommandError::Failed("no project is open".to_owned())
}

fn open_dialog(core: &Core) {
    core.ui()
        .open_settings_window(crate::shell::page_url(crate::shell::ROUTE_TRUST));
}

/// Answers `message` when it is this module's (`trustDecision`, `getTrust`, `revokeTrust`);
/// `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let answer = match message {
        ShellToCore::TrustDecision {
            prompt_id,
            decision,
        } => state::decide(core, *prompt_id, *decision).map(|()| CoreToShell::Ack),
        ShellToCore::GetTrust => {
            // The dialog opened after the banner's prompt: ask again, so it has the prompt id;
            // a project with nothing protected is asked here too, so a person can trust it.
            if let Some((root, state)) = state::current(core) {
                state::prompt_on_request(core, &root, &state);
            }
            Ok(state::info(core))
        }
        ShellToCore::RevokeTrust { root } => {
            state::revoke(core, Path::new(root)).map(|()| state::info(core))
        }
        _ => return None,
    };
    Some(answer)
}

/// The project opened: check its trust, prompt when needed.
pub fn on_start(core: &Core) {
    if let Some((root, state)) = state::refresh(core) {
        if !state.trusted && !state.blocked.is_empty() {
            tracing::info!(
                parts = state.blocked.len(),
                "the workspace is not trusted; its protected parts are blocked"
            );
        }
        state::prompt_if_needed(core, &root, &state);
    }
}

/// [`on_start`] on a blocking thread of the core's runtime (RB-090-36), then `then` there: the
/// app's start runs on the CEF UI thread, and the check reads the trust store and hashes the
/// protected folders. The trust the config was loaded with is in effect until then
/// ([`start_trust`]); what depends on the checked trust (extensions, plugins) starts in `then`,
/// after the check, never beside it. Checks never run side by side (`state::refresh`).
pub fn on_start_then(core: &Core, then: impl FnOnce(&Arc<Core>) + Send + 'static) {
    let Some(core) = core.arc() else {
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        on_start(&core);
        then(&core);
    });
}

/// The config changed: check again; a changed protected part asks again.
pub fn on_config_changed(core: &Core) {
    if let Some((root, state)) = state::refresh(core) {
        state::prompt_if_needed(core, &root, &state);
    }
}

/// The project config of `root` as the app loads it: `rb_config::load_project` with the trust
/// gate decided by the store at `rb_config::trust::trust_store_path()` - a trusted workspace
/// keeps `network.ignoreCertificateErrors`, `network.proxy` and `network.hostResolverRules`,
/// any other loses them with a warning each. Also answers whether the workspace was trusted.
pub fn load_config(root: &Path) -> Result<(LoadedConfig, bool), ConfigError> {
    let store = state::load_store(trust_store_path().as_deref());
    let (loaded, trust) = load_project_trusted(root, &store)?;
    if !trust.trusted && !trust.blocked.is_empty() {
        tracing::info!(
            parts = trust.blocked.len(),
            "the workspace is not trusted; its network overrides are not applied"
        );
    }
    Ok((loaded, trust.trusted))
}

/// The trust the project config was loaded with at start-up ([`load_config`]), before the grid
/// is built: the views attach with the project's headers at once, and the first check
/// ([`on_start`]) changes nothing for a project trusted at start - no second emulation, no
/// restart notice for network keys that already apply.
pub fn start_trust(host: &Host, trusted: bool) {
    host.update_shell_state(|shell| shell.trusted = trusted);
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
