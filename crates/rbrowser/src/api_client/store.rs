//! The collections on disk (RB-096-01): one request per `.http` file below `.rbrowser/api/`,
//! folders as the collection's folders, `http-client.env.json` beside them. Every id is checked
//! (`rb_api_model::paths`) and no symbolic link is followed - not the folder, not a file - so a
//! cloned project cannot lead a read or a write anywhere else. Files over [`MAX_FILE_BYTES`] are
//! not read; a file that does not parse is listed with its reason, never dropped silently.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rb_api_model::env::ENV_FILE;
use rb_api_model::paths::{self, API_DIR, EXTENSION, MAX_DEPTH};
use rb_api_model::{ApiRequest, EnvFile, parse_http, to_http};
use rb_plugin_types::{Failure, FailureKind};
use rb_util::win_name;
use serde::Serialize;

use super::error;

/// The largest request or environment file read.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// The most requests listed.
pub const MAX_REQUESTS: usize = 1000;

/// One request of the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    /// Why the file does not read, when it does not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// The collections of the project at `root`.
pub struct Store {
    root: PathBuf,
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

pub(super) fn io_failure(name: &str, error: &io::Error) -> Failure {
    super::error(FailureKind::Io, name).with_param("reason", error.kind().to_string())
}

impl Store {
    /// The collections below `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// `.rbrowser/api/`.
    #[must_use]
    pub fn dir(&self) -> PathBuf {
        self.root.join(API_DIR)
    }

    /// The file of `id`, when the id reads and no step on the way is a link.
    pub(super) fn path_of(&self, id: &str) -> Result<PathBuf, Failure> {
        if !paths::is_request_id(id) {
            return Err(
                error(FailureKind::BadData, "bad_id").with_param("id", rb_util::text::cut(id, 80))
            );
        }
        self.inside(id)
    }

    /// `relative` below the folder, refused when the folder, a folder on the way or the target
    /// is a link.
    pub(super) fn inside(&self, relative: &str) -> Result<PathBuf, Failure> {
        let mut path = self.root.clone();
        let steps = API_DIR.split('/').chain(relative.split('/'));
        for step in steps.filter(|step| !step.is_empty()) {
            path.push(step);
            if is_link(&path) {
                return Err(error(FailureKind::AccessDenied, "link"));
            }
        }
        Ok(path)
    }

    /// Every request, sorted by id.
    ///
    /// # Errors
    ///
    /// The folder is a link, or does not read.
    pub fn list(&self) -> Result<Vec<Listed>, Failure> {
        let dir = self.inside("")?;
        let mut out = Vec::new();
        if dir.is_dir() {
            self.walk(&dir, "", 0, &mut out)?;
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    fn walk(
        &self,
        dir: &Path,
        prefix: &str,
        depth: usize,
        out: &mut Vec<Listed>,
    ) -> Result<(), Failure> {
        let entries = fs::read_dir(dir).map_err(|e| io_failure("unreadable", &e))?;
        for entry in entries.flatten() {
            if out.len() >= MAX_REQUESTS {
                return Ok(());
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = paths::join(prefix, &name);
            if kind.is_dir() && depth < MAX_DEPTH && paths::is_folder(&id) {
                self.walk(&entry.path(), &id, depth + 1, out)?;
            } else if kind.is_file() && paths::is_request_id(&id) {
                out.push(match self.read(&id) {
                    Ok((request, _)) => Listed {
                        id,
                        name: request.name,
                        method: request.method,
                        url: request.url,
                        problem: None,
                    },
                    Err(failure) => Listed {
                        name: name.trim_end_matches(EXTENSION).to_owned(),
                        id,
                        method: String::new(),
                        url: String::new(),
                        problem: Some(problem_of(&failure)),
                    },
                });
            }
        }
        Ok(())
    }

    /// The saved request `id` and its file's text.
    ///
    /// # Errors
    ///
    /// A bad id, a link, no such file, too large, not UTF-8, not a request (`unreadable` with the
    /// line and the reason).
    pub fn read(&self, id: &str) -> Result<(ApiRequest, String), Failure> {
        let path = self.path_of(id)?;
        let text = read_text(&path)?
            .ok_or_else(|| error(FailureKind::NotFound, "not_found").with_param("id", id))?;
        let request = parse_http(&text).map_err(|parse| {
            error(FailureKind::BadData, "unreadable")
                .with_param("line", parse.line)
                .with_param("reason", parse.reason)
        })?;
        Ok((request, text))
    }

    /// Stores `request` as `id`, or as a new file in `folder` named after its title; the id.
    ///
    /// # Errors
    ///
    /// A bad id or folder, a link, a request that does not hold ([`ApiRequest::check`]), a
    /// write that failed.
    pub fn save(
        &self,
        id: Option<&str>,
        folder: Option<&str>,
        request: &ApiRequest,
    ) -> Result<String, Failure> {
        request.check().map_err(|invalid| {
            error(FailureKind::BadData, "invalid_request").with_param("reason", invalid.reason())
        })?;
        let id = match id {
            Some(id) => id.to_owned(),
            None => {
                let folder = folder.unwrap_or_default().trim_matches('/');
                if !paths::is_folder(folder) {
                    return Err(error(FailureKind::BadData, "bad_id")
                        .with_param("id", rb_util::text::cut(folder, 80)));
                }
                let wanted = paths::join(folder, &paths::file_name(&request.name));
                paths::free_id(&wanted, |candidate| match self.path_of(candidate) {
                    Ok(path) => path.exists(),
                    Err(_) => true,
                })
            }
        };
        let path = self.path_of(&id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_failure("not_written", &e))?;
        }
        // The folders just made are checked again: none may have become a link meanwhile.
        let path = self.path_of(&id)?;
        rb_util::fs::write_atomic(&path, to_http(request).as_bytes())
            .map_err(|e| io_failure("not_written", &e))?;
        Ok(id)
    }

    /// Deletes the saved request `id`.
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

    /// `http-client.env.json`; empty when there is none.
    ///
    /// # Errors
    ///
    /// A link, a file that does not read (`env_unreadable` with the reason).
    pub fn env_file(&self) -> Result<EnvFile, Failure> {
        let path = self.inside(ENV_FILE)?;
        match read_text(&path)? {
            None => Ok(EnvFile::default()),
            Some(text) => EnvFile::parse(&text).map_err(|reason| {
                error(FailureKind::BadData, "env_unreadable")
                    .with_param("reason", rb_util::text::cut(&reason, 200))
            }),
        }
    }

    /// `http-client.env.json` as JSON (`null` when there is none), for an import to add to.
    ///
    /// # Errors
    ///
    /// A link, a file that does not read or is no JSON.
    pub fn env_json(&self) -> Result<serde_json::Value, Failure> {
        let path = self.inside(ENV_FILE)?;
        match read_text(&path)? {
            None => Ok(serde_json::Value::Null),
            Some(text) => serde_json::from_str(&text).map_err(|e| {
                error(FailureKind::BadData, "env_unreadable")
                    .with_param("reason", rb_util::text::cut(&e.to_string(), 200))
            }),
        }
    }

    /// Writes `value` as `http-client.env.json`, once it reads as environments.
    ///
    /// # Errors
    ///
    /// Not of the form ([`EnvFile::from_json`]), a link, a write that failed.
    pub fn write_env_json(&self, value: &serde_json::Value) -> Result<(), Failure> {
        EnvFile::from_json(value).map_err(|reason| {
            error(FailureKind::BadData, "env_unreadable")
                .with_param("reason", rb_util::text::cut(&reason, 200))
        })?;
        let dir = self.inside("")?;
        fs::create_dir_all(&dir).map_err(|e| io_failure("not_written", &e))?;
        let path = self.inside(ENV_FILE)?;
        let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
        text.push('\n');
        rb_util::fs::write_atomic(&path, text.as_bytes()).map_err(|e| io_failure("not_written", &e))
    }

    /// The file `relative` to request `id`'s folder (a body's `< ./file`), held below the project
    /// root, out of `.git/` and of `.rbrowser/` but for `.rbrowser/api/`, and through no link.
    ///
    /// # Errors
    ///
    /// A path that climbs out or holds a link, a file that does not read or is over `limit`.
    pub fn body_file(
        &self,
        id: Option<&str>,
        relative: &str,
        limit: u64,
    ) -> Result<Vec<u8>, Failure> {
        let base = id
            .and_then(|id| id.rsplit_once('/').map(|(folder, _)| folder.to_owned()))
            .unwrap_or_default();
        let mut parts: Vec<String> = API_DIR.split('/').map(str::to_owned).collect();
        parts.extend(
            base.split('/')
                .filter(|part| !part.is_empty())
                .map(str::to_owned),
        );
        for step in relative.replace('\\', "/").split('/') {
            match step {
                "" | "." => {}
                ".." => {
                    // Never above the project root.
                    if parts.pop().is_none() {
                        return Err(error(FailureKind::AccessDenied, "outside"));
                    }
                }
                // A stream (`:`), an 8.3 short name (`GIT~1`), a name of dots and spaces only
                // (`...`, which Windows may read as another): never a body file.
                other
                    if other.contains(':')
                        || win_name::is_short_name(other)
                        || win_name::folded(other).is_empty() =>
                {
                    return Err(error(FailureKind::AccessDenied, "outside"));
                }
                other => parts.push(other.to_owned()),
            }
        }
        // Not the repository, not rBrowser's own files (config.local.json holds secrets) - only
        // the collections' folder of `.rbrowser/`, in no spelling Windows reads as another
        // (`.RBROWSER`, `.rbrowser.`, `.git `).
        let first = parts.first().map(|first| win_name::folded(first));
        let exact_api = parts.first().is_some_and(|first| first == ".rbrowser")
            && parts.get(1).is_some_and(|second| second == "api");
        if parts.is_empty()
            || first.as_deref() == Some(".git")
            || (first.as_deref() == Some(".rbrowser") && !exact_api)
        {
            return Err(error(FailureKind::AccessDenied, "outside"));
        }
        let mut path = self.root.clone();
        for part in &parts {
            path.push(part);
            if is_link(&path) {
                return Err(error(FailureKind::AccessDenied, "link"));
            }
        }
        let meta = fs::metadata(&path).map_err(|e| io_failure("file_unreadable", &e))?;
        if !meta.is_file() || meta.len() > limit {
            return Err(error(FailureKind::Limit, "file_too_large").with_param("limit", limit));
        }
        fs::read(&path).map_err(|e| io_failure("file_unreadable", &e))
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

/// The text of the file at `path`; `None` when there is none.
fn read_text(path: &Path) -> Result<Option<String>, Failure> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_failure("unreadable", &e)),
    };
    if !meta.is_file() {
        return Ok(None);
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(error(FailureKind::Limit, "file_too_large").with_param("limit", MAX_FILE_BYTES));
    }
    let bytes = fs::read(path).map_err(|e| io_failure("unreadable", &e))?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| error(FailureKind::BadData, "unreadable").with_param("reason", "not UTF-8"))
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
