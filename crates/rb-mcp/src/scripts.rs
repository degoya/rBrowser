//! MCP code mode (RB-098-01): `script_run` runs a short JavaScript in the host's sandbox, its
//! `rb.<tool>(...)` calls the server's own tools, and only the script's return value comes back.
//!
//! The script gets no right the client has not: every inner call goes through the server's one
//! tool router and so through the guards of a direct call ([`bridge`]: `evaluate` behind
//! `mcp.allowEvaluate`, `flow_run` behind workspace trust, `config_set` and `settings_set`
//! refusing the protected keys), counts as a request in the activity record, and `script_run`
//! itself is refused inside (`script.recursion`). The sandbox is the host's ([`ScriptRunner`],
//! `crates/rb-script` in the app): no file system, network, environment or process, a memory
//! limit, a deadline. This module holds the contract, the bridge, the `rb.*` signatures of the
//! tool's description ([`signatures`]) and the answer ([`answer`]).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

pub mod answer;
pub mod bridge;
pub mod signatures;

/// The tool.
pub const TOOL: &str = "script_run";
/// The setting that switches code mode on (off by default).
pub const ENABLED_KEY: &str = "mcp.scripts.enabled";
/// The setting of a run's time.
pub const TIMEOUT_KEY: &str = "mcp.scripts.timeoutMs";
/// The setting of a run's tool calls.
pub const MAX_CALLS_KEY: &str = "mcp.scripts.maxCalls";
/// The setting of the answer's size.
pub const MAX_RESULT_KEY: &str = "mcp.scripts.maxResultBytes";

/// The codes of `script_run`.
pub mod codes {
    /// Code mode is off (`mcp.scripts.enabled`).
    pub const DISABLED: &str = "scripts_disabled";
    /// The script does not parse.
    pub const SYNTAX: &str = "script.syntax";
    /// The run took longer than `mcp.scripts.timeoutMs`.
    pub const TIMEOUT: &str = "script.timeout";
    /// The script made more tool calls than `mcp.scripts.maxCalls`.
    pub const CALL_LIMIT: &str = "script.call_limit";
    /// The script called `script_run`.
    pub const RECURSION: &str = "script.recursion";
    /// An uncaught error without a code of its own.
    pub const ERROR: &str = "script.error";
    /// The script ran out of stack (too deep a recursion).
    pub const STACK: &str = "script.stack";
    /// The run took more memory than the sandbox allows.
    pub const MEMORY: &str = "script.memory";
    /// This rBrowser has no script engine, or it failed.
    pub const UNAVAILABLE: &str = "script.unavailable";
}

/// The most bytes of `code`.
pub const MAX_CODE_BYTES: usize = 64 * 1024;
/// The most bytes of `args` as JSON.
pub const MAX_ARGS_BYTES: usize = 1024 * 1024;

/// `mcp.scripts.*`, read at every call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptOptions {
    /// `mcp.scripts.enabled`: `script_run` runs scripts (off by default).
    pub enabled: bool,
    /// `mcp.scripts.timeoutMs`: the whole run, its tool calls included.
    pub timeout: Duration,
    /// `mcp.scripts.maxCalls`: the most tool calls of one run.
    pub max_calls: u32,
    /// `mcp.scripts.maxResultBytes`: the most bytes of the answer; more is cut.
    pub max_result_bytes: usize,
}

impl Default for ScriptOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout: Duration::from_secs(30),
            max_calls: 200,
            max_result_bytes: 64 * 1024,
        }
    }
}

/// What `rb.<tool>(...)` reaches: the server's tools behind their guards ([`bridge::Bridge`]).
#[async_trait]
pub trait ScriptTools: Send + Sync + 'static {
    /// Runs `tool` with `arguments` (a JSON object as text): its answer as JSON text, or the
    /// refusal as JSON text `{"code", "message"}`.
    async fn call(&self, tool: &str, arguments: &str) -> Result<String, String>;
}

/// One run as the sandbox gets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptRequest {
    /// The body of `async function (args, target)`.
    pub code: String,
    /// `args` as JSON text.
    pub args: String,
    /// `target` as JSON text.
    pub target: String,
    /// The whole run.
    pub timeout: Duration,
}

/// Why a run gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptStop {
    /// The script failed; the engine's failure JSON (`{"code", "name", "message", ...}`).
    Script(String),
    /// The time ran out.
    Timeout,
    /// The memory limit was reached.
    OutOfMemory,
    /// The engine failed; one line.
    Engine(String),
}

/// The sandbox the host runs scripts in (`crates/rb-script` in the app).
#[async_trait]
pub trait ScriptRunner: Send + Sync + 'static {
    /// Runs `request` in a fresh instance; its tool calls go to `tools`. Ok: the engine's
    /// answer as JSON text `{"value", "logs"}`.
    async fn run(
        &self,
        request: ScriptRequest,
        tools: Arc<dyn ScriptTools>,
    ) -> Result<String, ScriptStop>;
}
