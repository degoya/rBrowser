//! The result of a run, `.rbrowser/out/flows/<runId>/result.json`: the same document the shell,
//! `flow_run` and `rbrowser flow run --json` show. The views' fields are the sidecar's
//! (`sidecar/src/modules/flows/result.ts`); the views rBrowser skipped are added here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::FlowError;

/// The folder of every run, below the project root.
pub const OUT_DIR: &str = ".rbrowser/out/flows";
/// The result file inside a run's folder.
pub const RESULT_FILE: &str = "result.json";
/// The format version of [`RunResult`].
pub const RESULT_VERSION: u32 = 1;

/// The status of one view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewStatus {
    /// The flow ran through.
    Passed,
    /// An error or a failed `expect`.
    Failed,
    /// Not run (Firefox, no page); `reason` says why.
    Skipped,
    /// Longer than the timeout.
    TimedOut,
}

/// One named step of a view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    /// `step(name, ...)`.
    pub name: String,
    /// `passed` or `failed`.
    pub status: String,
    /// How long it took.
    pub duration_ms: u64,
}

/// Where and why a view failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    /// The error's message.
    pub message: String,
    /// Its stack.
    #[serde(default)]
    pub stack: String,
    /// The step that was running.
    #[serde(default)]
    pub step: Option<String>,
    /// The line in the flow file, 1-based.
    #[serde(default)]
    pub line: Option<u32>,
    /// The column.
    #[serde(default)]
    pub column: Option<u32>,
    /// The flow file as the error names it (the Linux path for a WSL project).
    #[serde(default)]
    pub file: String,
}

/// One view of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewResult {
    /// The view's id.
    pub view_id: u32,
    /// Its label.
    pub label: String,
    /// Its breakpoint.
    #[serde(default)]
    pub breakpoint: Option<String>,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// What happened.
    pub status: ViewStatus,
    /// How long it ran.
    #[serde(default)]
    pub duration_ms: u64,
    /// Its named steps.
    #[serde(default)]
    pub steps: Vec<StepResult>,
    /// Why it failed.
    #[serde(default)]
    pub error: Option<ErrorInfo>,
    /// The screenshot's file name inside the run's folder.
    #[serde(default)]
    pub screenshot: Option<String>,
    /// Why it was skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One run of one flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// [`RESULT_VERSION`].
    pub version: u32,
    /// The run's id, also its folder's name.
    pub run_id: String,
    /// The flow's name.
    pub flow: String,
    /// The flow's file, relative to the project root.
    pub file: String,
    /// When it started, RFC 3339 UTC.
    pub started_at: String,
    /// How long the whole run took.
    pub duration_ms: u64,
    /// `passed` when no view failed or timed out.
    pub passed: bool,
    /// Every view asked for, in grid order.
    pub views: Vec<ViewResult>,
    /// Why the run reached no view (no such flow, the trust gate, no endpoint); its views are
    /// then empty and `passed` is `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RunResult {
    /// Whether any view failed or timed out.
    pub fn failed(views: &[ViewResult]) -> bool {
        views
            .iter()
            .any(|view| matches!(view.status, ViewStatus::Failed | ViewStatus::TimedOut))
    }

    /// The counts by status: passed, failed (timed out included), skipped.
    pub fn counts(&self) -> (usize, usize, usize) {
        let count = |wanted: &[ViewStatus]| {
            self.views
                .iter()
                .filter(|view| wanted.contains(&view.status))
                .count()
        };
        (
            count(&[ViewStatus::Passed]),
            count(&[ViewStatus::Failed, ViewStatus::TimedOut]),
            count(&[ViewStatus::Skipped]),
        )
    }
}

/// A run id from the start time (RFC 3339) and random hex: `20261111-101500-a1b2c3`.
pub fn run_id(started_at: &str, random: &str) -> String {
    let digits: String = started_at
        .chars()
        .take(19)
        .filter(char::is_ascii_digit)
        .collect();
    let (date, time) = digits.split_at(digits.len().min(8));
    format!("{date}-{time}-{random}")
}

/// Whether `id` can name a run's folder: letters, digits and `-`, 1 to 64.
pub fn valid_run_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !id.starts_with('-')
}

/// The folder of the run `run_id` of the project at `root`.
pub fn run_dir(root: &Path, run_id: &str) -> PathBuf {
    root.join(OUT_DIR).join(run_id)
}

/// Writes `result` as `result.json` into `dir` through a temporary file, so a reader (the CLI
/// waiting for it) never sees half of it.
///
/// # Errors
///
/// [`FlowError::Io`] when the folder or the file cannot be written.
pub fn write(dir: &Path, result: &RunResult) -> Result<PathBuf, FlowError> {
    let io = |error: std::io::Error| FlowError::Io(format!("{}: {error}", dir.display()));
    std::fs::create_dir_all(dir).map_err(io)?;
    let text =
        serde_json::to_string_pretty(result).map_err(|error| FlowError::Io(error.to_string()))?;
    let path = dir.join(RESULT_FILE);
    let partial = dir.join(format!("{RESULT_FILE}.partial"));
    std::fs::write(&partial, format!("{text}\n")).map_err(io)?;
    std::fs::rename(&partial, &path).map_err(io)?;
    Ok(path)
}

/// Reads the `result.json` of `dir`; `None` while it is not there.
pub fn read(dir: &Path) -> Option<RunResult> {
    let text = std::fs::read_to_string(dir.join(RESULT_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
#[path = "result_tests.rs"]
mod tests;
