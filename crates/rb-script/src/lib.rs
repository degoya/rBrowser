//! The sandbox of MCP code mode (RB-098-01): `script_run` hands a short JavaScript to the script
//! engine, a WebAssembly component (`plugins/script-engine`, the Boa engine) run here in
//! wasmtime.
//!
//! - The component's one import is `rbrowser:script/host` ([`WIT`]): `call` - one MCP tool call,
//!   which the caller ([`Tools`], rb-mcp's script bridge) runs through the server's dispatcher
//!   and guards - and `clock-ms`. No WASI is linked, so a script has no file system, network,
//!   environment or process; a component importing anything else is refused before it runs.
//! - Every run gets a fresh store and instance: nothing survives between runs. Its memory is
//!   capped ([`Limits::memory_bytes`]), its time by an epoch deadline that also makes a busy
//!   loop yield every tick ([`ticker`]) and a tokio timeout around the whole run, tool calls
//!   included ([`Limits::timeout`]).
//! - [`ENGINE`] is the component as rBrowser ships it, checked in at
//!   `crates/rb-script/engine/rb-script-engine.wasm` and written by
//!   `scripts/build-script-engine.sh`.

mod sandbox;
pub mod ticker;

pub use sandbox::{Limits, RunError, Sandbox};

/// The contract of the engine component.
pub const WIT: &str = include_str!("../wit/script.wit");
/// The interface the component imports.
pub const HOST_INTERFACE: &str = "rbrowser:script/host@0.1.0";
/// The interface the component exports.
pub const ENGINE_INTERFACE: &str = "rbrowser:script/engine@0.1.0";
/// The engine component rBrowser ships (`scripts/build-script-engine.sh`).
pub const ENGINE: &[u8] = include_bytes!("../engine/rb-script-engine.wasm");

/// What a script's `rb.<tool>(...)` reaches: the MCP tools behind the caller's guards.
#[async_trait::async_trait]
pub trait Tools: Send + Sync + 'static {
    /// Runs the tool `tool` with `arguments` (a JSON object as text): its answer as JSON text,
    /// or the refusal as JSON text `{"code", "message"}`.
    async fn call(&self, tool: String, arguments: String) -> Result<String, String>;
}

#[cfg(test)]
mod testbed;
