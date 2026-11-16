//! The runs on disk: `.rbrowser/out/checks/<run-id>/findings.json`, the last `checks.keepRuns`
//! kept. Every folder is walked without following a link (`context/files.rs`), so a checked-in
//! `.rbrowser/out` that links elsewhere refuses the write.

use std::path::{Path, PathBuf};

use rb_plugin_types::{Failure, FailureKind, FindingsFile, codes, paths};

use crate::context::files;

/// The folder below `.rbrowser/out/` (a plugin id no plugin may take).
pub const CHECKS_DIR: &str = "checks";

/// The file name of a run.
const FINDINGS_FILE: &str = "findings.json";

/// The digits of a run id: unix milliseconds, zero-padded so the ids sort by time.
const RUN_ID_DIGITS: usize = 13;

/// The id of a run started at `unix_millis`: `run-0001731254400000`.
#[must_use]
pub fn run_id(unix_millis: u128) -> String {
    format!("run-{unix_millis:0RUN_ID_DIGITS$}")
}

/// Whether `id` is one [`run_id`] makes: `run-` and 13 to 19 digits.
#[must_use]
pub fn valid_run_id(id: &str) -> bool {
    id.strip_prefix("run-").is_some_and(|digits| {
        (RUN_ID_DIGITS..=19).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
    })
}

/// Where the run `run_id` of the project at `root` is.
#[must_use]
pub fn findings_path(root: &Path, run_id: &str) -> PathBuf {
    root.join(paths::CONFIG_DIR)
        .join(paths::OUT_DIR)
        .join(CHECKS_DIR)
        .join(run_id)
        .join(FINDINGS_FILE)
}

fn io(error: &std::io::Error) -> Failure {
    tracing::debug!(%error, "a check run could not be stored");
    Failure::new(FailureKind::Io)
}

/// Writes `file` under the project at `root` and removes all but the newest `keep_runs` runs.
///
/// # Errors
///
/// The run id is malformed, a folder on the way is a link, or the disk refused.
pub async fn write_run(root: &Path, file: &FindingsFile, keep_runs: usize) -> Result<(), Failure> {
    if !valid_run_id(&file.run_id) {
        return Err(Failure::coded(FailureKind::BadData, codes::INVALID)
            .with_param("run", file.run_id.chars().take(40).collect::<String>()));
    }
    let text = serde_json::to_vec_pretty(file)
        .map_err(|_| Failure::coded(FailureKind::Other, codes::HOST_FAILED))?;
    let shown = format!("{CHECKS_DIR}/{}/{FINDINGS_FILE}", file.run_id);
    let segments = [
        paths::CONFIG_DIR,
        paths::OUT_DIR,
        CHECKS_DIR,
        file.run_id.as_str(),
        FINDINGS_FILE,
    ];
    files::write(root, &segments, &shown, &text).await?;
    prune(root, keep_runs).await
}

async fn prune(root: &Path, keep_runs: usize) -> Result<(), Failure> {
    let folder = [paths::CONFIG_DIR, paths::OUT_DIR, CHECKS_DIR];
    let mut runs: Vec<String> = files::list(root, &folder, CHECKS_DIR)
        .await?
        .into_iter()
        .filter(|(name, is_dir)| *is_dir && valid_run_id(name))
        .map(|(name, _)| name)
        .collect();
    runs.sort();
    let excess = runs.len().saturating_sub(keep_runs.max(1));
    for run in runs.into_iter().take(excess) {
        let path = folder
            .iter()
            .fold(root.to_path_buf(), |path, segment| path.join(segment))
            .join(run);
        tokio::fs::remove_dir_all(&path)
            .await
            .map_err(|error| io(&error))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
