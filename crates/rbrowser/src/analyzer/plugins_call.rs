//! A call into a loaded plugin (split from `plugins.rs`, RB-092-06): [`LoadedPlugins::call_tool`]
//! and its [`PluginError`], and [`block_on`], which runs a plugin's export on the core's runtime
//! from a blocking thread.

use serde_json::Value;
use tokio::runtime::Handle;

use super::LoadedPlugins;
use rb_plugin_host::{HostServices as _, PluginSource};

/// A plugin call that failed: the stable code (`plugins.error.*` or the plugin's own key) and
/// its parameters - no English text, no secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginError {
    pub code: String,
    pub params: Vec<(String, String)>,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.code)?;
        for (key, value) in &self.params {
            write!(formatter, " {key}={value}")?;
        }
        Ok(())
    }
}

/// The code `plugins.error.not_available` with the plugin's id.
const NOT_AVAILABLE: &str = "plugins.error.not_available";

impl LoadedPlugins {
    /// Calls `tool` of plugin `id` with `args`; the answer as JSON.
    ///
    /// # Errors
    ///
    /// The plugin is not loaded (its refusal, else `plugins.error.not_available`), it is a
    /// project plugin and the workspace is not trusted now (`plugins.error.untrusted_workspace`,
    /// review M2 of Charge 6), or the call failed.
    pub fn call_tool(&self, id: &str, tool: &str, args: &Value) -> Result<Value, PluginError> {
        let Some(entry) = self
            .report
            .loaded
            .iter()
            .find(|entry| entry.plugin.manifest().id == id)
        else {
            return Err(self.refusal(id).unwrap_or_else(|| PluginError {
                code: NOT_AVAILABLE.to_owned(),
                params: vec![("service".to_owned(), id.to_owned())],
            }));
        };
        if entry.source == PluginSource::Project
            && !crate::plugins::services::shared().workspace_trusted()
        {
            return Err(PluginError {
                code: rb_plugin_types::codes::UNTRUSTED_WORKSPACE.to_owned(),
                params: vec![("plugin".to_owned(), id.to_owned())],
            });
        }
        let answer = block_on(entry.plugin.call_tool(tool, &args.to_string()))
            .map_err(PluginError::from)?
            .map_err(|failure| PluginError {
                code: failure.code().unwrap_or(NOT_AVAILABLE).to_owned(),
                params: failure.params.into_iter().collect(),
            })?;
        serde_json::from_str(&answer).map_err(|_| PluginError {
            code: "plugins.error.bad_output".to_owned(),
            params: Vec::new(),
        })
    }

    /// The grants of plugin `id`'s manifest, when it is loaded: the `project-read` globs bound
    /// the files the static source hands it (RB-030-07).
    pub fn grants(&self, id: &str) -> Option<&rb_plugin_host::Grants> {
        self.report
            .loaded
            .iter()
            .find(|entry| entry.plugin.manifest().id == id)
            .map(|entry| &entry.plugin.manifest().grants)
    }

    /// Why plugin `id` did not load, when its package was refused.
    pub fn refusal(&self, id: &str) -> Option<PluginError> {
        self.report
            .refused
            .iter()
            .find(|refused| {
                refused
                    .path
                    .file_stem()
                    .is_some_and(|stem| stem.to_string_lossy().starts_with(id))
            })
            .map(|refused| PluginError {
                code: refused.failure.code().unwrap_or(NOT_AVAILABLE).to_owned(),
                params: refused
                    .failure
                    .params
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            })
    }
}

impl From<NoRuntime> for PluginError {
    fn from(error: NoRuntime) -> Self {
        Self {
            code: rb_plugin_types::codes::HOST_FAILED.to_owned(),
            params: vec![("detail".to_owned(), error.to_string())],
        }
    }
}

/// The calling thread has no tokio runtime: the CEF UI thread, or a thread started without
/// entering the core's runtime. Waiting there would hang the window (CDP answers on the UI
/// thread), so nothing runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NoRuntime;

impl std::fmt::Display for NoRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("no runtime on this thread: plugin calls run on a blocking thread of the core's runtime")
    }
}

impl std::error::Error for NoRuntime {}

/// Runs `future` to its end from a thread that is no async task - a blocking task of the core's
/// runtime, or a thread that entered its handle - on that runtime. Like
/// [`crate::reload::ReloadService::block_on`], a thread without a runtime gets [`NoRuntime`]
/// instead of a runtime of its own: from the UI thread that would be a deadlock until the
/// plugin's timeout.
///
/// # Errors
///
/// [`NoRuntime`]: no runtime is entered on this thread.
pub(crate) fn block_on<F: std::future::Future>(future: F) -> Result<F::Output, NoRuntime> {
    let handle = Handle::try_current().map_err(|_| NoRuntime)?;
    Ok(handle.block_on(future))
}
