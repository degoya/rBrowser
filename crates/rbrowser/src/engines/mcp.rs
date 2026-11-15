//! MCP's `engine_restart` (RB-030-01 registers it) - the arguments, the answer and the function
//! behind it - and the engine fields `views_list` adds per view: `engine`, `external`,
//! `status`, `unsupported` and, for an external view, `externalWindow` (`open` or `closed`,
//! RB-080-38) and `embedded` (the window sits in its grid cell, RB-080-39). The command
//! `engine.restart` is `covered(engine_restart)`, and it is how an agent opens a closed window
//! again; `view.external.arrange` and `view.external.raise` are omitted (desktop window layout,
//! no use to an agent; with the embedding on, arrange shows the views as windows of their own).

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::external::ExternalStatus;
use rb_engine::{EmulationReport, ViewHost, ViewId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::commands::restart;
use super::for_host;
use crate::control::Core;
use crate::host::Host;

/// The tool's name.
pub const ENGINE_RESTART_TOOL: &str = "engine_restart";
/// The tool's description for the MCP tool list.
pub const ENGINE_RESTART_DESCRIPTION: &str = "Restart an external engine's views: `engine: \
     \"firefox\"` or `\"webkit\"`, and optionally `viewId` for one view. Each view opens again \
     with its last URL. Fails with a readable message when no view of that engine is open or the \
     engine is not available. Answers the restarted view ids.";

/// The engines `engine_restart` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExternalEngine {
    /// Firefox (RB-040-02).
    Firefox,
    /// WebKit (RB-040-04).
    Webkit,
}

impl From<ExternalEngine> for EngineKind {
    fn from(engine: ExternalEngine) -> Self {
        match engine {
            ExternalEngine::Firefox => EngineKind::Firefox,
            ExternalEngine::Webkit => EngineKind::Webkit,
        }
    }
}

/// The arguments of `engine_restart`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineRestartArgs {
    /// The engine.
    pub engine: ExternalEngine,
    /// One view; every view of the engine when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<ViewId>,
}

/// The answer of `engine_restart`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRestartAnswer {
    /// The views that start again.
    pub restarted: Vec<ViewId>,
}

/// Runs `engine_restart` with `args` (its JSON).
pub fn engine_restart(core: &Core, args: &Value) -> Result<EngineRestartAnswer, CommandError> {
    let args: EngineRestartArgs = serde_json::from_value(args.clone())
        .map_err(|error| CommandError::InvalidArgs(error.to_string()))?;
    let engines = for_host(core.host())
        .ok_or_else(|| CommandError::Failed("the external windows are not started".to_owned()))?;
    restart(&engines, Some(args.engine.into()), args.view_id)
        .map(|restarted| EngineRestartAnswer { restarted })
        .map_err(|error| CommandError::Failed(error.to_string()))
}

/// What `views_list` adds to a view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewEngineState {
    /// The engine.
    pub engine: EngineKind,
    /// Whether it lives in an external window.
    pub external: bool,
    /// `running`, or an external view's status (`starting`, `exited`, `restarting`, `failed`,
    /// `unavailable`).
    pub status: String,
    /// The emulation fields the engine does not apply as asked.
    pub unsupported: Vec<String>,
    /// An external view's window: `open` while it runs, `closed` otherwise (closed by the
    /// person, crashed, failed, not available); absent for a grid view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_window: Option<String>,
    /// An external view's window sits in its grid cell (`engines.external.embed`, RB-080-39);
    /// `false` while it is a window of its own; absent for a grid view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded: Option<bool>,
}

/// The engine fields of `view` for `views_list`; `None` for an unknown view.
pub fn view_engine_state(host: &Host, view: ViewId) -> Option<ViewEngineState> {
    let info = host.view(view)?;
    let external = for_host(host).and_then(|engines| engines.windows().view(view));
    Some(match external {
        Some(external) => ViewEngineState {
            engine: external.engine,
            external: true,
            status: status_text(&external.status),
            unsupported: external.unsupported,
            external_window: Some(
                if matches!(
                    external.status,
                    ExternalStatus::Running | ExternalStatus::Starting | ExternalStatus::Restarting
                ) {
                    "open"
                } else {
                    "closed"
                }
                .to_owned(),
            ),
            embedded: Some(external.embedded),
        },
        None => ViewEngineState {
            engine: info.engine,
            external: false,
            status: "running".to_owned(),
            unsupported: host
                .emulation(view)
                .map(|emulation| EmulationReport::for_engine(info.engine, &emulation).unsupported())
                .unwrap_or_default(),
            external_window: None,
            embedded: None,
        },
    })
}

fn status_text<T: Serialize>(status: &T) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
