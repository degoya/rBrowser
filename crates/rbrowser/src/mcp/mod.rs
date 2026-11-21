//! The MCP server in the app (RB-030-01): `rb-mcp` over this core. The server runs while the
//! user setting `mcp.enabled` is on (off by default) - started with the project, started and
//! stopped when the setting changes, stopped with the app; its token is new at every start and
//! registered with the log's mask. [`backend::CoreBackend`] answers the tools through the one
//! command registry; `rbrowser mcp` is [`cli`].
//!
//! The commands: `mcp.toggle` (`Leader , M`) switches the setting - never over MCP, a person
//! decides - and `mcp.copy_config` (`Leader , C`) shows the line that adds rBrowser to Claude
//! Code (stdio through `rbrowser mcp`, so no token is in it). `mcp.setup` (`Leader , N`) opens
//! Settings > Automation > MCP at the client choice, where [`setup`] connects an AI client in
//! the project or for every project (RB-091-09; the setup wizard's step does the same).

pub mod autostart;
pub mod backend;
pub mod cli;
pub mod clients;
pub(crate) mod collect;
pub mod host_tools;
pub mod latest;
pub mod network_tools;
pub mod settings;
pub mod setup;
pub mod shared_logs;
pub mod shortcuts;
pub mod url_policy;
pub mod wsl;

use std::path::Path;
use std::sync::{Arc, Mutex, Weak};

use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_mcp::{HttpHandle, McpHttpConfig, McpToken};
use rb_settings::Scope;
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;
use crate::settings_window::Caller;

/// `mcp.toggle`.
pub const TOGGLE: &str = "mcp.toggle";
/// `mcp.copy_config`.
pub const COPY_CONFIG: &str = "mcp.copy_config";
/// `mcp.setup`.
pub const SETUP: &str = "mcp.setup";
/// The setting that switches the server.
pub const ENABLED_KEY: &str = "mcp.enabled";
/// The client the MCP setup shows first; `mcp.setup` opens the settings there (RB-091-09).
pub const CLIENT_KEY: &str = "mcp.client";
/// The i18n keys of the notices; `{port}` and `{snippet}` are placeholders.
pub mod notice {
    /// The server listens.
    pub const STARTED: &str = "mcp.notice.started";
    /// The server stopped.
    pub const STOPPED: &str = "mcp.notice.stopped";
    /// The server did not start; `{error}`.
    pub const FAILED: &str = "mcp.notice.failed";
    /// The line for Claude Code; `{snippet}`.
    pub const CONFIG: &str = "mcp.notice.config";
    /// The line for Claude Code on Windows and in WSL; `{snippet}`, `{wsl}` (RB-030-02).
    pub const CONFIG_WSL: &str = "mcp.notice.config_wsl";
}

/// The commands: id, keys after the leader, why there is no MCP tool.
const COMMANDS: [(&str, [char; 2], &str); 3] = [
    (TOGGLE, [',', 'M'], "protected: MCP cannot switch itself"),
    (
        COPY_CONFIG,
        [',', 'C'],
        "ui-only: a line for the person's agent configuration",
    ),
    (
        SETUP,
        [',', 'N'],
        "ui-only: a person connects an AI client and confirms every file it writes",
    ),
];

/// The server of this process.
struct Server {
    /// Counts starts and stops, so a start that finishes after a stop is dropped.
    generation: u64,
    /// The port asked for when it was started (`mcp.port`).
    port: Option<u16>,
    /// The running server; `None` while starting or stopped.
    handle: Option<HttpHandle>,
    /// A start is wanted (running or on its way).
    wanted: bool,
    /// The log buffers of the running server, shared with the plugins (RB-090-34).
    logs: Option<Arc<shared_logs::SharedLogs>>,
}

static SERVER: Mutex<Server> = Mutex::new(Server {
    generation: 0,
    port: None,
    handle: None,
    wanted: false,
    logs: None,
});

/// The port the running MCP server listens on; `None` while it does not run. The portal never
/// offers it as an asset origin (RB-090-72).
pub fn running_port() -> Option<u16> {
    lock(&SERVER)
        .handle
        .as_ref()
        .map(|handle| handle.addr().port())
}

/// The command definitions.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    COMMANDS
        .iter()
        .map(|(id, keys, omitted)| {
            Ok(CommandDef {
                id: CommandId::new(id)?,
                title_key: format!("commands.{id}"),
                group: Group::Settings,
                default_bindings: vec![Binding::Leader(Chord {
                    keys: keys.iter().map(|key| Key::Char(*key)).collect(),
                })],
                mcp: McpCoverage::Omitted((*omitted).to_owned()),
            })
        })
        .collect()
}

/// Registers the commands with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| {
            let handler = handler(def.id.as_str(), Arc::downgrade(core));
            registry.register(def, handler).err()
        })
        .collect()
}

fn handler(id: &str, core: Weak<Core>) -> Handler {
    let id = id.to_owned();
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        match id.as_str() {
            TOGGLE => toggle(&core),
            COPY_CONFIG => Ok(copy_config(&core)),
            SETUP => {
                core.ui()
                    .open_settings_window(crate::shell::page_url(&setup_route()));
                Ok(Value::Null)
            }
            other => Err(CommandError::UnknownCommand(other.to_owned())),
        }
    })
}

/// `mcp.toggle`: writes the opposite of `mcp.enabled` as the settings window would; the server
/// follows ([`on_user_settings_changed`]).
fn toggle(core: &Core) -> Result<Value, CommandError> {
    let enabled = !core.user_settings().mcp.enabled;
    match crate::settings_window::set_setting(
        core,
        ENABLED_KEY,
        &Value::Bool(enabled),
        Scope::User,
        Caller::Shell,
    ) {
        CoreToShell::SettingRejected { issues, .. } => {
            Err(CommandError::Failed(issues.first().map_or_else(
                || "settings.rejected.invalid".to_owned(),
                |issue| issue.message_key.clone(),
            )))
        }
        _ => Ok(json!({ "enabled": enabled })),
    }
}

/// `mcp.copy_config`: the line that adds rBrowser to Claude Code, shown as a notice - with the
/// line for Claude Code in WSL when the program sits on a drive (RB-030-02).
fn copy_config(core: &Core) -> Value {
    let exe = std::env::current_exe().unwrap_or_default();
    let root = core.host().project_root();
    let snippet = config_snippet(&exe, root.as_deref());
    let wsl = wsl_config_snippet(&exe, root.as_deref());
    let shown = match &wsl {
        Some(wsl) => notification(
            NotificationLevel::Info,
            notice::CONFIG_WSL,
            &[("snippet", snippet.clone()), ("wsl", wsl.clone())],
        ),
        None => notification(
            NotificationLevel::Info,
            notice::CONFIG,
            &[("snippet", snippet.clone())],
        ),
    };
    core.notify(shown);
    json!({ "snippet": snippet, "wslSnippet": wsl })
}

/// The pages window's route `mcp.setup` opens: the client choice in Settings > Automation > MCP.
#[must_use]
pub fn setup_route() -> String {
    format!("settings/{CLIENT_KEY}")
}

/// `claude mcp add rbrowser -- "<exe>" mcp [--project "<root>"]`: stdio through `rbrowser mcp`,
/// which finds the running instance itself - no port and no token in it.
pub fn config_snippet(exe: &Path, root: Option<&Path>) -> String {
    let mut line = format!("claude mcp add rbrowser -- \"{}\" mcp", exe.display());
    if let Some(root) = root {
        line.push_str(&format!(" --project \"{}\"", root.display()));
    }
    line
}

/// The same line for Claude Code inside WSL: the program at its `/mnt/<drive>/` path (run
/// through interop), the project at its Linux path; `None` when the program is on no drive.
pub fn wsl_config_snippet(exe: &Path, root: Option<&Path>) -> Option<String> {
    let exe = wsl_path(&exe.to_string_lossy())?;
    let mut line = format!("claude mcp add rbrowser -- \"{exe}\" mcp");
    if let Some(root) = root.and_then(|root| wsl_path(&root.to_string_lossy())) {
        line.push_str(&format!(" --project \"{root}\""));
    }
    Some(line)
}

/// The Linux path of a Windows path as WSL sees it: a share path of a distribution its path
/// inside, a drive path its default mount (`C:\a` is `/mnt/c/a`).
pub fn wsl_path(windows: &str) -> Option<String> {
    if rb_wsl::path::is_wsl_unc(windows) {
        return rb_wsl::path::parse_unc(windows)
            .ok()
            .map(|path| path.linux_path);
    }
    let text = windows.replace('/', "\\");
    let mut chars = text.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    let rest = text.strip_prefix(letter)?.strip_prefix(':')?;
    if !(rest.is_empty() || rest.starts_with('\\')) {
        return None;
    }
    Some(format!(
        "/mnt/{}{}",
        letter.to_ascii_lowercase(),
        rest.replace('\\', "/")
    ))
}

/// The core is up: the server starts when the setting says so.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
}

/// The user settings changed: the server starts, stops or moves to the new port.
pub fn on_user_settings_changed(core: &Core) {
    let settings = core.user_settings().mcp;
    let mut server = lock(&SERVER);
    if server.wanted && (!settings.enabled || server.port != settings.port) {
        stop_locked(&mut server);
        core.notify(notification(NotificationLevel::Info, notice::STOPPED, &[]));
    }
    if settings.enabled && !server.wanted {
        start_locked(&mut server, core, settings.port);
    }
    shared_logs::apply_settings(&settings);
}

/// Stops the server (the app quits); the instance file goes with it.
pub fn shutdown() {
    stop_locked(&mut lock(&SERVER));
}

fn stop_locked(server: &mut Server) {
    server.generation += 1;
    server.wanted = false;
    // Dropping the handle stops the server and removes the instance file; letting the buffers go
    // stops their collector unless the plugins still hold them.
    server.handle = None;
    server.logs = None;
}

fn start_locked(server: &mut Server, core: &Core, port: Option<u16>) {
    let Some(core_arc) = core.arc() else {
        return;
    };
    let token = match McpToken::generate() {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(%error, "no MCP token: the OS gives no randomness");
            return;
        }
    };
    rb_diag::register_secret(token.expose());
    server.generation += 1;
    server.wanted = true;
    server.port = port;
    let generation = server.generation;
    let root = core.host().project_root();
    let instance_file = rb_config::local_dir().map(|local| {
        let key = root.as_deref().map(crate::instance::file::key);
        rb_mcp::instance::file_path(&local, key.as_deref())
    });
    let config = McpHttpConfig {
        port: port.unwrap_or(0),
        token,
        instance_file,
        project: root
            .map(|root| root.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let logs = shared_logs::acquire(core);
    let store = Arc::clone(logs.store());
    server.logs = Some(logs);
    let backend: Arc<dyn rb_mcp::Backend> = Arc::new(backend::CoreBackend::new(&core_arc, store));
    let weak = Arc::downgrade(&core_arc);
    core.runtime().spawn(async move {
        let started = rb_mcp::serve_http(backend, config).await;
        let core = weak.upgrade();
        let mut server = lock(&SERVER);
        match started {
            Ok(handle) if server.generation == generation && server.wanted => {
                let port = handle.addr().port().to_string();
                server.handle = Some(handle);
                drop(server);
                if let Some(core) = core {
                    core.notify(notification(
                        NotificationLevel::Info,
                        notice::STARTED,
                        &[("port", port)],
                    ));
                }
            }
            // Stopped meanwhile: the handle drops here and takes its file with it.
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "the MCP server did not start");
                if server.generation == generation {
                    server.wanted = false;
                }
                drop(server);
                if let Some(core) = core {
                    core.notify(notification(
                        NotificationLevel::Error,
                        notice::FAILED,
                        &[("error", error.to_string())],
                    ));
                }
            }
        }
    });
}

fn notification(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(ENABLED_KEY.to_owned()),
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
