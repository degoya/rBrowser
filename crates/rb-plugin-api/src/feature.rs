//! The native side of the `feature-plugin` world: `interface feature`.
//!
//! A feature plugin serves the commands and MCP tools its manifest declares; the host calls
//! `run-command` with the command's id (without the `plugin.<id>.` prefix) and `call-tool` with
//! the tool's name (without the `<mcp_prefix>_` prefix). Arguments and answers are JSON text.

use crate::{Failure, PluginHost};

/// The logic of a feature plugin. A store lives for one call.
pub trait Feature {
    /// Called once after the plugin is loaded; the default does nothing.
    ///
    /// # Errors
    ///
    /// The plugin cannot start; the host marks it unhealthy.
    fn activate(&mut self, host: &mut dyn PluginHost) -> Result<(), Failure> {
        let _ = host;
        Ok(())
    }

    /// Runs the command `command` with `args` (JSON); answers JSON.
    ///
    /// # Errors
    ///
    /// The command failed or is unknown (`plugins.error.unknown_contribution`).
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure>;

    /// Runs the MCP tool `tool` with `args` (JSON); answers JSON.
    ///
    /// # Errors
    ///
    /// The tool failed or is unknown (`plugins.error.unknown_contribution`).
    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure>;
}

#[cfg(test)]
#[path = "feature_tests.rs"]
mod tests;
