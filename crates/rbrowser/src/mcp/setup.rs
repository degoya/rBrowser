//! The MCP setup's requests (RB-091-09): the setup wizard's step "Connect an AI agent" and
//! Settings > Automation > MCP ask for the plan, write what the person ticked and check the
//! connection (`mcpSetup`, from the pages window only - `settings_window::guard`; MCP has no
//! tool for any of it, and `mcp.enabled` stays the person's switch). Each request runs on the
//! blocking pool, as the files may lie in WSL, and answers with a `mcpSetup` event.
//!
//! - A project file is written only into the open project or the one the wizard set up.
//! - A user file only with `user: true` - the person ticked its confirmation - and with a copy of
//!   the old file beside it.
//! - A file that cannot be merged is never written; nothing in it but the `rbrowser` entry is
//!   changed, and no answer, log line or file carries a token: every way is stdio through
//!   `rbrowser mcp`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rb_ipc::{
    CoreToShell, IpcErrorCode, McpClient, McpFileStatus, McpSetupAnswer, McpSetupRequest,
    McpWrittenFile, ShellToCore, parse_request,
};

use super::clients::files::{self, UserFiles};
use super::clients::{
    self, Format, NO_PROJECT_FILE_KEY, NO_PROJECT_KEY, PlanInput, project_file, user_file_format,
};
use crate::control::Core;
use crate::ipc::Failure;

/// The page named a folder that is neither the open project nor the wizard's.
pub const ROOT_REFUSED_KEY: &str = "mcp.setup.error.root";
/// The file cannot be read or merged; `{path}`.
pub const UNREADABLE_KEY: &str = "mcp.setup.error.unreadable";
/// The file could not be written; `{path}`, `{error}`.
pub const WRITE_FAILED_KEY: &str = "mcp.setup.error.write";
/// Nothing to write for this client.
pub const NOTHING_KEY: &str = "mcp.setup.error.nothing";

/// A failure: its i18n key and values.
pub type SetupFailure = (&'static str, BTreeMap<String, String>);

/// This machine as the clients start rBrowser.
#[derive(Clone, Debug)]
pub struct Machine {
    /// The program.
    pub exe: String,
    /// The program from WSL; `None` when WSL cannot reach it.
    pub wsl_exe: Option<String>,
    /// The clients' user files.
    pub user_files: UserFiles,
}

impl Machine {
    /// The running program and the person's files.
    #[must_use]
    pub fn here() -> Self {
        let exe = std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        // WSL exists only beside Windows.
        let wsl_exe = if cfg!(windows) {
            super::wsl_path(&exe)
        } else {
            None
        };
        Self {
            exe,
            wsl_exe,
            user_files: UserFiles::here(),
        }
    }
}

/// Whether `request` is one of the MCP setup's: the pages window's only.
#[must_use]
pub fn is_request(request: &str) -> bool {
    matches!(parse_request(request), Ok(ShellToCore::McpSetup { .. }))
}

/// Answers `mcpSetup` with `ack` and sends its result as an event; `None` for any other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::McpSetup { request } = message else {
        return None;
    };
    let Some(core) = core.arc() else {
        return Some(Err((
            IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
    };
    let request = request.clone();
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        let answer = answer(&core, &request);
        core.send(&CoreToShell::McpSetup { answer });
    });
    Some(Ok(CoreToShell::Ack))
}

/// The answer to `request`, on this thread.
pub fn answer(core: &Core, request: &McpSetupRequest) -> McpSetupAnswer {
    let request_id = request.request_id();
    let failed = |(key, args): SetupFailure| McpSetupAnswer::Failed {
        request_id,
        message_key: key.to_owned(),
        args,
    };
    match request {
        McpSetupRequest::Plan { root, .. } => match root_for(core, root.as_deref()) {
            Ok(root) => plan(core, request_id, root.as_deref()),
            Err(failure) => failed(failure),
        },
        McpSetupRequest::Write {
            client,
            root,
            project,
            user,
            ..
        } => {
            let written = root_for(core, root.as_deref()).and_then(|root| {
                write(
                    *client,
                    root.as_deref(),
                    &Machine::here(),
                    *project,
                    *user,
                    now_secs(),
                )
            });
            match written {
                Ok(files) => McpSetupAnswer::Written { request_id, files },
                Err(failure) => failed(failure),
            }
        }
        McpSetupRequest::Check { .. } => {
            let (ago, calls) = rb_mcp::activity::SERVER.since_last(SystemTime::now());
            let port = super::running_port();
            McpSetupAnswer::Check {
                request_id,
                enabled: core.user_settings().mcp.enabled,
                running: port.is_some(),
                port,
                last_call_secs: ago.map(|ago| ago.as_secs()),
                calls,
            }
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The folders the setup may write into: the open project, then the one the wizard set up.
fn root_for(core: &Core, asked: Option<&str>) -> Result<Option<PathBuf>, SetupFailure> {
    let mut allowed: Vec<PathBuf> = core.host().project_root().into_iter().collect();
    allowed.extend(crate::first_run::written_root());
    pick_root(asked, &allowed)
}

/// The folder for `asked` among `allowed` (the first without one).
///
/// # Errors
///
/// [`ROOT_REFUSED_KEY`]: `asked` is none of them.
pub fn pick_root(
    asked: Option<&str>,
    allowed: &[PathBuf],
) -> Result<Option<PathBuf>, SetupFailure> {
    match asked {
        None => Ok(allowed.first().cloned()),
        Some(asked) => allowed
            .iter()
            .find(|root| rb_mcp::stdio::same_root(&root.to_string_lossy(), asked))
            .cloned()
            .map(Some)
            .ok_or((ROOT_REFUSED_KEY, BTreeMap::new())),
    }
}

/// The program as the project's client starts it: by its WSL path for a project in WSL.
#[must_use]
pub fn project_command<'a>(machine: &'a Machine, root: Option<&Path>) -> &'a str {
    let in_wsl = root.is_some_and(|root| rb_wsl::path::is_wsl_unc(&root.to_string_lossy()));
    match (&machine.wsl_exe, in_wsl) {
        (Some(wsl), true) => wsl,
        _ => &machine.exe,
    }
}

fn plan(core: &Core, request_id: u32, root: Option<&Path>) -> McpSetupAnswer {
    let machine = Machine::here();
    let input = PlanInput {
        project_command: project_command(&machine, root),
        exe: &machine.exe,
        wsl_exe: machine.wsl_exe.as_deref(),
        root,
        user_files: &machine.user_files,
    };
    McpSetupAnswer::Plan {
        request_id,
        enabled: core.user_settings().mcp.enabled,
        root: root.map(|root| root.to_string_lossy().into_owned()),
        clients: McpClient::ALL
            .iter()
            .map(|client| clients::plan(*client, &input, &files::read))
            .collect(),
    }
}

/// Writes `client`'s project file into `root` and, confirmed by the person (`user`), its user
/// file with a backup made at `now` - the project file first; a failure stops there.
///
/// # Errors
///
/// What is missing ([`NOTHING_KEY`], [`NO_PROJECT_FILE_KEY`], [`NO_PROJECT_KEY`]), a file that
/// cannot be merged ([`UNREADABLE_KEY`]) or written ([`WRITE_FAILED_KEY`]).
pub fn write(
    client: McpClient,
    root: Option<&Path>,
    machine: &Machine,
    project: bool,
    user: bool,
    now: u64,
) -> Result<Vec<McpWrittenFile>, SetupFailure> {
    let nothing = || (NOTHING_KEY, BTreeMap::new());
    if !project && !user {
        return Err(nothing());
    }
    let mut written = Vec::new();
    if project {
        let (relative, format) =
            project_file(client).ok_or((NO_PROJECT_FILE_KEY, BTreeMap::new()))?;
        let root = root.ok_or((NO_PROJECT_KEY, BTreeMap::new()))?;
        let command = project_command(machine, Some(root));
        written.push(write_one(
            &root.join(relative),
            relative,
            format,
            command,
            None,
        )?);
    }
    if user {
        let format = user_file_format(client).ok_or_else(nothing)?;
        let path = machine.user_files.of(client).ok_or_else(nothing)?;
        let shown = path.to_string_lossy();
        written.push(write_one(path, &shown, format, &machine.exe, Some(now))?);
    }
    Ok(written)
}

fn write_one(
    path: &Path,
    shown: &str,
    format: Format,
    command: &str,
    backup_at: Option<u64>,
) -> Result<McpWrittenFile, SetupFailure> {
    let named = |key: &'static str, error: Option<String>| {
        let mut args = BTreeMap::from([("path".to_owned(), shown.to_owned())]);
        if let Some(error) = error {
            args.insert("error".to_owned(), error);
        }
        (key, args)
    };
    let existing = files::read(path).map_err(|_| named(UNREADABLE_KEY, None))?;
    let merged = format.merge(existing.as_deref(), command);
    let status = merged.status;
    let backup = match (status, merged.text) {
        (McpFileStatus::Unchanged, _) => None,
        (McpFileStatus::Unreadable, _) | (_, None) => return Err(named(UNREADABLE_KEY, None)),
        (_, Some(text)) => files::write(path, &text, backup_at)
            .map_err(|error| named(WRITE_FAILED_KEY, Some(error.kind().to_string())))?,
    };
    if status != McpFileStatus::Unchanged {
        tracing::info!(file = %shown, ?status, "the MCP setup wrote a client's file");
    }
    Ok(McpWrittenFile {
        path: shown.to_owned(),
        status,
        backup: backup.map(|backup| backup.to_string_lossy().into_owned()),
    })
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
