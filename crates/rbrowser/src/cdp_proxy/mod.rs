//! The CDP proxy for outside clients (RB-030-06, ADR 0001 Decision 4, K10): Playwright's
//! `connectOverCDP(url, { headers })`, Puppeteer and other CDP tools reach the grid's views
//! through `127.0.0.1:<port>`, never through CEF itself (which speaks CDP only over its pipe).
//!
//! | Part | Where |
//! | --- | --- |
//! | Token, routing, target filter, sessions, emulation and input rules (pure) | `rb-cdp` |
//! | The listener, `/json/*`, the WebSocket upgrade, the token's rotation | [`server`] |
//! | One client's WebSocket over its own flat session on the pipe | [`connection`] |
//! | The answer of the MCP tool `cdp_endpoint` | [`endpoint`] |
//! | The tunnel into WSL in NAT mode (`rbrowser-wsl cdp-proxy`) | [`tunnel`] |
//!
//! The proxy runs while the user setting `cdp.enabled` is on (off by default; protected, so
//! never switched over MCP) on `cdp.port` (a free port when empty); its token is new at every
//! start and registered with the log's mask, so it is never logged. The commands:
//! `cdp.copy_endpoint` (`Leader , E`) shows the endpoint URL with its token in the shell, and
//! `cdp.rotate_token` (`Leader , O`, rOtate) replaces the token and ends every open connection - a
//! person's decision, so it has no MCP tool.
//!
//! **Why clients cannot change a view's size:** rBrowser owns `Emulation.setDeviceMetricsOverride`
//! (each view is its breakpoint; ADR 0001 Decision 3), so the metric commands are answered and
//! not sent ([`rb_cdp::policy`]) - Playwright's `page.setViewportSize` has no effect here, nor
//! its media features (`Emulation.setEmulatedMedia`, RB-040-06). Other emulation a client sets
//! is undone when it leaves: the views it attached get their emulation re-applied.
//! `cdp.copy_snippet` (`Leader F C`, RB-040-06) shows a ready `connectOverCDP` script with the
//! token, the same as the `snippet` of `cdp_endpoint`.

pub mod connection;
pub mod endpoint;
pub mod server;
pub mod tunnel;

use std::sync::{Arc, Mutex, Weak};

use rb_cdp::ProxyToken;
use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
pub use server::{Hooks, ProxyServer};

/// `cdp.copy_endpoint`.
pub const COPY_ENDPOINT: &str = "cdp.copy_endpoint";
/// `cdp.rotate_token`.
pub const ROTATE_TOKEN: &str = "cdp.rotate_token";
/// `cdp.copy_snippet`: the ready Playwright script (RB-040-06).
pub const COPY_SNIPPET: &str = "cdp.copy_snippet";
/// The setting that switches the proxy.
pub const ENABLED_KEY: &str = "cdp.enabled";
/// The MCP tool that gives the endpoint.
pub const MCP_TOOL: &str = "cdp_endpoint";
/// Why `cdp.rotate_token` has no MCP tool.
pub const ROTATE_OMITTED: &str = "protected: token management via UI";

/// The i18n keys of the notices; `{port}`, `{url}` and `{error}` are placeholders.
pub mod notice {
    /// The proxy listens.
    pub const STARTED: &str = "cdp.notice.started";
    /// The proxy stopped.
    pub const STOPPED: &str = "cdp.notice.stopped";
    /// The proxy did not start; `{error}`.
    pub const FAILED: &str = "cdp.notice.failed";
    /// The endpoint with its token; `{url}`.
    pub const ENDPOINT: &str = "cdp.notice.endpoint";
    /// The token was replaced.
    pub const ROTATED: &str = "cdp.notice.rotated";
    /// A command needs the proxy, which is off.
    pub const OFF: &str = "cdp.notice.off";
    /// The Playwright script with its token; `{snippet}`.
    pub const SNIPPET: &str = "cdp.notice.snippet";
}

/// The proxy of this process and its tunnels.
struct State {
    /// The running proxy.
    server: Option<Arc<ProxyServer>>,
    /// The port asked for when it was started (`cdp.port`).
    port: Option<u16>,
}

static STATE: Mutex<State> = Mutex::new(State {
    server: None,
    port: None,
});

/// The command definitions.
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    [
        (
            COPY_ENDPOINT,
            Group::Settings,
            'E',
            McpCoverage::Covered(MCP_TOOL.to_owned()),
        ),
        (
            ROTATE_TOKEN,
            Group::Settings,
            'O',
            McpCoverage::Omitted(ROTATE_OMITTED.to_owned()),
        ),
        (
            COPY_SNIPPET,
            Group::Flows,
            'C',
            McpCoverage::Covered(MCP_TOOL.to_owned()),
        ),
    ]
    .into_iter()
    .map(|(id, group, key, mcp)| {
        Ok(CommandDef {
            id: CommandId::new(id)?,
            title_key: format!("commands.{id}"),
            group,
            default_bindings: vec![Binding::Leader(Chord {
                keys: vec![Key::Char(group.leader_key()), Key::Char(key)],
            })],
            mcp,
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
        let Some(server) = current() else {
            core.notify(notification(NotificationLevel::Warning, notice::OFF, &[]));
            return Err(CommandError::Failed(
                "the CDP proxy is off: switch it on in Settings > Automation > CDP proxy"
                    .to_owned(),
            ));
        };
        match id.as_str() {
            COPY_ENDPOINT => {
                let url = endpoint::url_with_token(server.port(), &server.token());
                core.notify(notification(
                    NotificationLevel::Info,
                    notice::ENDPOINT,
                    &[("url", url)],
                ));
                Ok(json!({ "port": server.port() }))
            }
            COPY_SNIPPET => {
                let snippet = endpoint::snippet(server.port(), &server.token());
                core.notify(notification(
                    NotificationLevel::Info,
                    notice::SNIPPET,
                    &[("snippet", snippet)],
                ));
                Ok(json!({ "port": server.port() }))
            }
            ROTATE_TOKEN => {
                rotate(&server).map_err(|error| CommandError::Failed(error.to_string()))?;
                core.notify(notification(NotificationLevel::Info, notice::ROTATED, &[]));
                Ok(json!({ "rotated": true }))
            }
            other => Err(CommandError::UnknownCommand(other.to_owned())),
        }
    })
}

/// A fresh token for `server`: registered with the log's mask first, then swapped in, which
/// ends every open connection.
fn rotate(server: &ProxyServer) -> std::io::Result<()> {
    let token = ProxyToken::generate()?;
    rb_diag::register_secret(token.expose());
    server.rotate(token);
    Ok(())
}

/// The running proxy, when `cdp.enabled` is on and it started.
pub fn current() -> Option<Arc<ProxyServer>> {
    lock(&STATE).server.clone()
}

/// The core is up: the proxy starts when the setting says so. Runs on CEF's UI thread, outside
/// the runtime: everything async goes through `core.runtime()`.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
}

/// The user settings changed: the proxy starts, stops or moves to the new port.
pub fn on_user_settings_changed(core: &Core) {
    let settings = core.user_settings().cdp;
    let mut state = lock(&STATE);
    if state.server.is_some() && (!settings.enabled || state.port != settings.port) {
        stop_locked(&mut state);
        core.notify(notification(NotificationLevel::Info, notice::STOPPED, &[]));
    }
    if settings.enabled && state.server.is_none() {
        start_locked(&mut state, core, settings.port);
    }
}

/// Stops the proxy and its tunnels (the app quits).
pub fn shutdown() {
    stop_locked(&mut lock(&STATE));
}

fn stop_locked(state: &mut State) {
    if let Some(server) = state.server.take() {
        server.stop();
    }
    state.port = None;
    tunnel::stop_all();
}

fn start_locked(state: &mut State, core: &Core, port: Option<u16>) {
    let started = ProxyToken::generate().and_then(|token| {
        rb_diag::register_secret(token.expose());
        ProxyServer::start(
            core.runtime(),
            core.host().cdp().clone(),
            port,
            token,
            hooks(core.host()),
        )
    });
    match started {
        Ok(server) => {
            let port_text = server.port().to_string();
            tracing::info!(port = server.port(), "the CDP proxy listens");
            state.server = Some(server);
            state.port = port;
            core.notify(notification(
                NotificationLevel::Info,
                notice::STARTED,
                &[("port", port_text)],
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "the CDP proxy did not start");
            core.notify(notification(
                NotificationLevel::Error,
                notice::FAILED,
                &[("error", error.to_string())],
            ));
        }
    }
}

/// The app's side of the proxy: a target's scale, the re-applied emulation, the hidden targets.
pub fn hooks(host: &Arc<Host>) -> Hooks {
    let for_scale = Arc::downgrade(host);
    let for_end = Arc::downgrade(host);
    Hooks {
        scale: Arc::new(move |target: &str| {
            let host = for_scale.upgrade()?;
            let view = view_of_target(&host, target)?;
            host.views()
                .into_iter()
                .find(|info| info.id == view)
                .map(|info| info.scale)
        }),
        on_end: Arc::new(move |targets: Vec<String>| {
            let Some(host) = for_end.upgrade() else {
                return;
            };
            for target in targets {
                if let Some(view) = view_of_target(&host, &target) {
                    host.reapply_emulation(view);
                }
            }
        }),
        filter: crate::devtools::hidden::filter(),
    }
}

/// The view whose page is the target `target`.
pub fn view_of_target(host: &Host, target: &str) -> Option<ViewId> {
    host.views()
        .into_iter()
        .map(|info| info.id)
        .find(|view| host.target_id(*view).as_deref() == Some(target))
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

#[cfg(test)]
#[path = "playwright_tests.rs"]
mod playwright_tests;
