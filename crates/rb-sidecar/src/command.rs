//! The commands `sidecar.restart` (leader `R S`) and `engines.update` (leader `V U`), both
//! without an MCP tool: the sidecar restarts by itself, and a download of browser binaries is
//! the user's decision. The job file's `V N` for the restart is `view.navigate` and `V S` is
//! `view.set_size` (RB-010-16); the restart sits in the Reload group beside `wsl.restart_helper`.

use std::sync::Arc;

use rb_commands::{Binding, Chord, CommandDef, CommandError, CommandId, Group, Handler, Key};
use serde_json::{Value, json};

use crate::LOG_TARGET;
use crate::error::SidecarError;
use crate::playwright::{InstallProgress, InstallReport, install};
use crate::supervisor::{Launcher, Sidecar};

/// Restarts the sidecar by hand (after a crash loop, or to pick up a new `engines.nodePath`).
pub const RESTART_ID: &str = "sidecar.restart";
/// Downloads the Playwright browsers (WebKit).
pub const UPDATE_ID: &str = "engines.update";
/// Why [`RESTART_ID`] has no MCP tool.
pub const RESTART_OMITTED: &str = "internal process; it restarts by itself";
/// Why [`UPDATE_ID`] has no MCP tool.
pub const UPDATE_OMITTED: &str = "downloads browser binaries from the network; only by the user";

fn def(id: &str, group: Group, key: char, omitted: &str) -> Result<CommandDef, CommandError> {
    Ok(CommandDef {
        id: CommandId::new(id)?,
        title_key: format!("commands.{id}"),
        group,
        default_bindings: vec![Binding::Leader(Chord {
            keys: vec![Key::Char(group.leader_key()), Key::Char(key)],
        })],
        mcp: rb_commands::McpCoverage::Omitted(omitted.to_owned()),
    })
}

/// `sidecar.restart`: leader `R S`.
pub fn restart_def() -> Result<CommandDef, CommandError> {
    def(RESTART_ID, Group::Reload, 'S', RESTART_OMITTED)
}

/// `engines.update`: leader `V U`.
pub fn update_def() -> Result<CommandDef, CommandError> {
    def(UPDATE_ID, Group::View, 'U', UPDATE_OMITTED)
}

/// The handler of `sidecar.restart`: restarts on `runtime` and answers at once
/// `{ "restarting": true }`; a failed start is logged.
pub fn restart_handler<L: Launcher + 'static>(
    sidecar: Arc<Sidecar<L>>,
    runtime: tokio::runtime::Handle,
) -> Handler {
    Arc::new(move |_args: Value| {
        let sidecar = Arc::clone(&sidecar);
        runtime.spawn(async move {
            if let Err(error) = sidecar.restart().await {
                let error = rb_diag::redact(&error.to_string());
                tracing::warn!(target: LOG_TARGET, %error, "the sidecar did not restart");
            }
        });
        Ok(json!({ "restarting": true }))
    })
}

/// Where the shell learns how `engines.update` goes.
pub type ProgressSink = Arc<dyn Fn(InstallProgress) + Send + Sync>;
/// Where the shell learns how `engines.update` ended.
pub type DoneSink = Arc<dyn Fn(Result<InstallReport, SidecarError>) + Send + Sync>;

/// The handler of `engines.update`: installs WebKit on `runtime` and answers at once
/// `{ "started": true }`; progress and the end go to the sinks.
pub fn update_handler<L: Launcher + 'static>(
    sidecar: Arc<Sidecar<L>>,
    runtime: tokio::runtime::Handle,
    on_progress: ProgressSink,
    on_done: DoneSink,
) -> Handler {
    Arc::new(move |_args: Value| {
        let sidecar = Arc::clone(&sidecar);
        let on_progress = Arc::clone(&on_progress);
        let on_done = Arc::clone(&on_done);
        runtime.spawn(async move {
            let result = install(&sidecar, "webkit", |progress| on_progress(progress)).await;
            on_done(result);
        });
        Ok(json!({ "started": true }))
    })
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
