//! MCP code mode in the app (RB-098-01): `rb-script`'s sandbox behind rb-mcp's
//! [`ScriptRunner`], and the command `mcp.run_script` (`Leader X J`), which in the shell only
//! opens the setting `mcp.scripts.enabled` - agents run scripts with `script_run`, a person
//! decides whether they may.
//!
//! The engine component compiles once per process, at the first script, off the async workers;
//! each run gets a fresh instance with [`MEMORY_BYTES`] of memory and the run's deadline, at
//! most [`MAX_CONCURRENT`] at once.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rb_commands::{
    Binding, Chord, CommandDef, CommandError, CommandId, CommandRegistry, Group, Handler, Key,
    McpCoverage,
};
use rb_mcp::scripts::{ENABLED_KEY, ScriptRequest, ScriptRunner, ScriptStop, ScriptTools, TOOL};
use rb_script::{Limits, RunError, Sandbox};
use serde_json::{Value, json};
use tokio::sync::{OnceCell, Semaphore};

use crate::control::Core;

/// `mcp.run_script`.
pub const RUN_SCRIPT: &str = "mcp.run_script";
/// Its keys after the leader.
const KEYS: [char; 2] = ['X', 'J'];
/// The memory of one run's engine instance.
pub const MEMORY_BYTES: usize = 256 << 20;
/// How many scripts run at once; more wait (within their time) so memory stays bounded.
pub const MAX_CONCURRENT: usize = 2;

/// The command's definition: MCP reaches the function as `script_run`.
///
/// # Errors
///
/// The id does not parse (a test holds it).
pub fn definitions() -> Result<Vec<CommandDef>, CommandError> {
    Ok(vec![CommandDef {
        id: CommandId::new(RUN_SCRIPT)?,
        title_key: format!("commands.{RUN_SCRIPT}"),
        group: Group::Plugins,
        default_bindings: vec![Binding::Leader(Chord {
            keys: KEYS.iter().map(|key| Key::Char(*key)).collect(),
        })],
        mcp: McpCoverage::Covered(TOOL.to_owned()),
    }])
}

/// Registers the command with `registry`; returns what the registry refused.
pub fn register(registry: &mut CommandRegistry, core: &Arc<Core>) -> Vec<CommandError> {
    let defs = match definitions() {
        Ok(defs) => defs,
        Err(error) => return vec![error],
    };
    defs.into_iter()
        .filter_map(|def| registry.register(def, handler(Arc::downgrade(core))).err())
        .collect()
}

/// Opens the settings at `mcp.scripts.enabled`.
fn handler(core: Weak<Core>) -> Handler {
    Arc::new(move |_args: Value| {
        let core = core
            .upgrade()
            .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
        core.run_command(
            crate::settings_window::OPEN_COMMAND,
            json!({ "key": ENABLED_KEY }),
        )?;
        Ok(Value::Null)
    })
}

/// The process's script sandbox.
#[must_use]
pub fn runner() -> Arc<dyn ScriptRunner> {
    static RUNNER: std::sync::OnceLock<Arc<WasmScripts>> = std::sync::OnceLock::new();
    let runner = RUNNER.get_or_init(|| Arc::new(WasmScripts::default()));
    Arc::clone(runner) as Arc<dyn ScriptRunner>
}

/// `rb-script`'s sandbox, made at the first run.
pub struct WasmScripts {
    sandbox: OnceCell<Result<Arc<Sandbox>, String>>,
    running: Semaphore,
}

impl Default for WasmScripts {
    fn default() -> Self {
        Self {
            sandbox: OnceCell::new(),
            running: Semaphore::new(MAX_CONCURRENT),
        }
    }
}

impl WasmScripts {
    async fn sandbox(&self) -> Result<Arc<Sandbox>, String> {
        self.sandbox
            .get_or_init(|| async {
                match tokio::task::spawn_blocking(Sandbox::new).await {
                    Ok(Ok(sandbox)) => Ok(Arc::new(sandbox)),
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "the script engine does not load");
                        Err(format!("{error:#}"))
                    }
                    Err(_) => Err("the script engine's load did not finish".to_owned()),
                }
            })
            .await
            .clone()
    }
}

#[async_trait]
impl ScriptRunner for WasmScripts {
    async fn run(
        &self,
        request: ScriptRequest,
        tools: Arc<dyn ScriptTools>,
    ) -> Result<String, ScriptStop> {
        let arrived = Instant::now();
        let sandbox = self.sandbox().await.map_err(ScriptStop::Engine)?;
        let _permit = self
            .running
            .acquire()
            .await
            .map_err(|_| ScriptStop::Engine("the script sandbox is closed".to_owned()))?;
        // One deadline from arrival: the wait for a turn and the first compile count, as
        // rb-mcp's own timer around this call started then too.
        let timeout = remaining(request.timeout, arrived.elapsed()).ok_or(ScriptStop::Timeout)?;
        let limits = Limits {
            timeout,
            memory_bytes: MEMORY_BYTES,
        };
        sandbox
            .run(
                &request.code,
                &request.args,
                &request.target,
                limits,
                Arc::new(Bridged(tools)),
            )
            .await
            .map_err(stop_of)
    }
}

/// What is left of `timeout` after `waited`; `None` once nothing is.
fn remaining(timeout: Duration, waited: Duration) -> Option<Duration> {
    timeout.checked_sub(waited).filter(|left| !left.is_zero())
}

/// rb-mcp's bridge as `rb-script`'s tools.
struct Bridged(Arc<dyn ScriptTools>);

#[async_trait]
impl rb_script::Tools for Bridged {
    async fn call(&self, tool: String, arguments: String) -> Result<String, String> {
        self.0.call(&tool, &arguments).await
    }
}

/// The sandbox's stop in rb-mcp's words.
fn stop_of(error: RunError) -> ScriptStop {
    match error {
        RunError::Script(failure) => ScriptStop::Script(failure),
        RunError::Timeout => ScriptStop::Timeout,
        RunError::OutOfMemory => ScriptStop::OutOfMemory,
        RunError::Engine(reason) => ScriptStop::Engine(reason),
    }
}

#[cfg(test)]
#[path = "scripts_tests.rs"]
mod tests;
