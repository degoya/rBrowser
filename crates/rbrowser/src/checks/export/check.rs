//! The instance's side of `rbrowser check` (RB-092-05): the request the command line hands over
//! (`Request::Check`), run as every check run is (shown in the panel, written under
//! `.rbrowser/out/checks/`), exported to the command line's `--out` or the export folder, and
//! answered small: the file, the counts, the threshold in effect and the plugins that failed -
//! the command line decides its exit code from them (`crate::cli::check`).

use std::path::PathBuf;
use std::time::Duration;

use rb_plugin_host::ChecksRunArgs;
use rb_plugin_types::FindingsSummary;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::command::{self, Destination};
use super::{ExportFormat, FindingsFailOn};
use crate::control::Core;

/// The most plugins `--plugins` names.
pub const MAX_PLUGINS: usize = 64;
/// How long the command line waits for a run and its export (a Lighthouse run over eight views
/// takes minutes); the run's own per-plugin limits end it sooner.
pub const CHECK_BUDGET: Duration = Duration::from_secs(30 * 60);

/// What `rbrowser check` asks of the running instance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckRequest {
    /// Only these check plugins (ids); every enabled one when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<String>,
    /// The file format; `checks.export.defaultFormat` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<ExportFormat>,
    /// An absolute file (the command line's `--out`); the export folder when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
    /// The threshold; `checks.export.failOn` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_on: Option<FindingsFailOn>,
}

impl CheckRequest {
    /// Whether the request stays in bounds: at most [`MAX_PLUGINS`] plugin ids as the check
    /// runner takes them, `out` a plain absolute file.
    ///
    /// # Errors
    ///
    /// Why not, for the person at the command line.
    pub fn check(&self) -> Result<(), String> {
        if self.plugins.len() > MAX_PLUGINS {
            return Err(format!("at most {MAX_PLUGINS} plugins can be named"));
        }
        self.run_args()?;
        if let Some(out) = &self.out {
            let path = crate::instance::protocol::check_out_dir(out)
                .map_err(|reason| reason.replace("output folder", "output file"))?;
            if path.file_name().is_none() || out.ends_with(['/', '\\']) {
                return Err(format!("{out}: --out names a file, not a folder"));
            }
        }
        Ok(())
    }

    /// The check run's arguments: every view, the plugins named.
    ///
    /// # Errors
    ///
    /// A plugin id the check runner refuses.
    pub fn run_args(&self) -> Result<ChecksRunArgs, String> {
        let plugins = (!self.plugins.is_empty()).then_some(&self.plugins);
        ChecksRunArgs::parse(&json!({ "plugins": plugins }))
            .map_err(|_| "a plugin id is not valid (letters, digits, -, _ and .)".to_owned())
    }
}

/// What the instance answers `rbrowser check`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAnswer {
    /// The file written.
    pub path: String,
    /// The run's id.
    pub run_id: String,
    /// The format written.
    pub format: ExportFormat,
    /// The threshold in effect.
    pub fail_on: FindingsFailOn,
    /// The counts per severity.
    pub summary: FindingsSummary,
    /// The plugins that failed in a view (each once).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_plugins: Vec<String>,
    /// The run was cancelled (in the window) before it was through.
    #[serde(default)]
    pub cancelled: bool,
}

/// Runs `request` in `core`'s window, exports the run and answers the [`CheckAnswer`].
///
/// # Errors
///
/// The request is out of bounds, another run is in progress, the run or the export failed.
pub fn run(core: &Core, request: &CheckRequest) -> Result<Value, String> {
    request.check()?;
    let args = request.run_args()?;
    let outcome = crate::checks::run_blocking(core, args).map_err(|error| error.to_string())?;
    let file = &outcome.file;
    let messages = crate::checks::run::messages(file, rb_i18n::plugin_texts);
    let format = command::format_of(core, request.format);
    let destination = match &request.out {
        Some(out) => Destination::Own(PathBuf::from(out)),
        None => Destination::Checked(None),
    };
    let path = command::export(core, file, &messages, format, destination)
        .map_err(|error| error.to_string())?;
    let project = core.host().config().checks.findings.export;
    let user = core.user_settings().checks.export;
    let fail_on = request
        .fail_on
        .or(project.over(&user).fail_on)
        .unwrap_or_default();
    let mut failed_plugins: Vec<String> = Vec::new();
    for failure in &file.failures {
        if !failed_plugins.contains(&failure.plugin) {
            failed_plugins.push(failure.plugin.clone());
        }
    }
    let answer = CheckAnswer {
        path: path.display().to_string(),
        run_id: file.run_id.clone(),
        format,
        fail_on,
        summary: file.summary.clone(),
        failed_plugins,
        cancelled: outcome.cancelled,
    };
    serde_json::to_value(answer).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
