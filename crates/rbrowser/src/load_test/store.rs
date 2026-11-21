//! The scenarios on disk (RB-096-07): one JSON file per scenario in `.rbrowser/api/load/`,
//! beside the API client's requests and checked in like them. An id is a file name of that
//! folder (`checkout.json`: letters, digits, space, `-`, `_`, `.`), never a path; no symbolic
//! link is followed, so a cloned project cannot lead a read or a write anywhere else. A file
//! that does not read is listed with its reason, never dropped silently.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rb_plugin_types::{Failure, FailureKind};
use serde::Serialize;

use super::error;
use super::scenario::Scenario;

/// The folder, below the project.
pub const LOAD_DIR: &str = ".rbrowser/api/load";
/// The largest scenario file read.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;
/// The most scenarios listed.
pub const MAX_SCENARIOS: usize = 500;

/// One scenario of the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// The scenarios of one project.
pub struct Scenarios {
    root: PathBuf,
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

fn io_failure(name: &str, failure: &io::Error) -> Failure {
    error(FailureKind::Io, name).with_param("reason", failure.kind().to_string())
}

/// Whether `id` names a scenario file of the folder.
#[must_use]
pub fn is_id(id: &str) -> bool {
    let Some(stem) = id.strip_suffix(".json") else {
        return false;
    };
    !stem.is_empty()
        && id.len() <= 120
        && !stem.starts_with('.')
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
}

/// A file name for a scenario titled `name`.
fn id_for(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .take(80)
        .collect();
    let stem = stem.trim_matches('-');
    format!("{}.json", if stem.is_empty() { "scenario" } else { stem })
}

impl Scenarios {
    /// The scenarios of the project at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// The folder, refused when it or a folder on the way is a link; `file` below it.
    fn inside(&self, file: Option<&str>) -> Result<PathBuf, Failure> {
        let mut path = self.root.clone();
        for step in LOAD_DIR.split('/').chain(file) {
            path.push(step);
            if is_link(&path) {
                return Err(error(FailureKind::AccessDenied, "link"));
            }
        }
        Ok(path)
    }

    fn path_of(&self, id: &str) -> Result<PathBuf, Failure> {
        if !is_id(id) {
            return Err(
                error(FailureKind::BadData, "bad_id").with_param("id", rb_util::text::cut(id, 80))
            );
        }
        self.inside(Some(id))
    }

    /// Every scenario, sorted by id.
    ///
    /// # Errors
    ///
    /// The folder is a link or does not read.
    pub fn list(&self) -> Result<Vec<Listed>, Failure> {
        let dir = self.inside(None)?;
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&dir).map_err(|e| io_failure("unreadable", &e))?;
        let mut ids: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|id| is_id(id))
            .collect();
        ids.sort();
        ids.truncate(MAX_SCENARIOS);
        Ok(ids
            .into_iter()
            .map(|id| match self.read(&id) {
                Ok(scenario) => Listed {
                    name: scenario.name,
                    id,
                    problem: None,
                },
                Err(failure) => Listed {
                    name: String::new(),
                    id,
                    problem: Some(problem_of(&failure)),
                },
            })
            .collect())
    }

    /// The scenario `id`.
    ///
    /// # Errors
    ///
    /// A bad id, a link, no such file, too large, not UTF-8, not a scenario (`unreadable` with
    /// the reason).
    pub fn read(&self, id: &str) -> Result<Scenario, Failure> {
        let path = self.path_of(id)?;
        let meta = fs::metadata(&path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                error(FailureKind::NotFound, "not_found").with_param("id", id)
            } else {
                io_failure("unreadable", &e)
            }
        })?;
        if !meta.is_file() {
            return Err(error(FailureKind::NotFound, "not_found").with_param("id", id));
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(
                error(FailureKind::Limit, "file_too_large").with_param("limit", MAX_FILE_BYTES)
            );
        }
        let bytes = fs::read(&path).map_err(|e| io_failure("unreadable", &e))?;
        let text = String::from_utf8(bytes).map_err(|_| {
            error(FailureKind::BadData, "unreadable").with_param("reason", "not UTF-8")
        })?;
        Scenario::from_json(&text).map_err(|reason| {
            error(FailureKind::BadData, "unreadable")
                .with_param("reason", rb_util::text::cut(&reason, 200))
        })
    }

    /// Stores `scenario` as `id`, or as a new file named after its title; the id.
    ///
    /// # Errors
    ///
    /// A bad id, a link, a scenario that does not hold (`invalid_scenario` with the field), a
    /// write that failed.
    pub fn save(&self, id: Option<&str>, scenario: &Scenario) -> Result<String, Failure> {
        scenario.check().map_err(|field| {
            error(FailureKind::BadData, "invalid_scenario").with_param("field", field)
        })?;
        let id = id.map_or_else(|| id_for(&scenario.name), str::to_owned);
        let path = self.path_of(&id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_failure("not_written", &e))?;
        }
        rb_util::fs::write_atomic(&path, scenario.to_json().as_bytes())
            .map_err(|e| io_failure("not_written", &e))?;
        Ok(id)
    }

    /// Deletes the scenario `id`.
    ///
    /// # Errors
    ///
    /// A bad id, a link, no such file, a delete that failed.
    pub fn delete(&self, id: &str) -> Result<(), Failure> {
        let path = self.path_of(id)?;
        if !path.is_file() {
            return Err(error(FailureKind::NotFound, "not_found").with_param("id", id));
        }
        fs::remove_file(&path).map_err(|e| io_failure("not_written", &e))
    }
}

/// A failure as the list's problem text: its code and parameters.
fn problem_of(failure: &Failure) -> String {
    let mut text = failure.code().unwrap_or("unreadable").to_owned();
    for (key, value) in &failure.params {
        text.push_str(&format!(" {key}={value}"));
    }
    text
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
