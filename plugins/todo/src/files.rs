//! The project's todo files: found by walking the project's folders (`project.list`, the root
//! first, one folder per call - the host lists no deeper), skipping the folders that never hold
//! the project's todos without listing them (RB-110-16), read whole, written back only over the bytes they were read with
//! (`project-write.write` with `expected`), so a file another program saved in between is a
//! conflict, never overwritten.

use std::collections::VecDeque;
use std::rc::Rc;

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes, paths};

use crate::config::Settings;
use crate::ignore::{self, Ignore};
use crate::{bad_args, coded};

/// The ending every todo file has; the manifest grants no other.
pub const EXTENSION: &str = ".todo";
/// The most todo files listed.
pub const MAX_FILES: usize = 200;
/// The most folders searched.
pub const MAX_FOLDERS: usize = 2_000;
/// The deepest folder searched below the root.
pub const MAX_DEPTH: usize = 16;
/// The largest todo file read, in bytes.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
/// The longest file path an argument may name, in characters.
const MAX_PATH_CHARS: usize = 1024;

/// The todo files found, sorted, and whether a bound stopped the search.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
    pub paths: Vec<String>,
    pub truncated: bool,
}

fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

/// Reads the `.gitignore` in `folder`; `None` when it does not read (not UTF-8, too large,
/// refused) or holds no rule - the search goes on without it.
fn read_ignore(host: &mut dyn PluginHost, folder: &str) -> Option<Rc<Ignore>> {
    let bytes = host.project_read(&join(folder, ignore::FILE)).ok()?;
    if bytes.len() > ignore::MAX_BYTES {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    Ignore::parse(folder, &text).map(Rc::new)
}

/// Whether the search goes into folder `name` at `path`: not a folder of `todo.exclude`, not
/// `.git/` or `.rbrowser/`, and - unless a glob of `todo.files` names it - not hidden (a name
/// starting with `.`) and not left out by a `.gitignore` (RB-110-16).
fn searched(settings: &Settings, ignores: &[Rc<Ignore>], name: &str, path: &str) -> bool {
    if settings.excluded(name) || paths::is_write_protected(&[name]) {
        return false;
    }
    settings.names_folder(path) || !(name.starts_with('.') || ignore::ignored(ignores, path))
}

/// The todo files of the project the settings name. Never searched: `.git/`, `.rbrowser/`, a
/// folder of `todo.exclude`, a hidden folder below the root and one a `.gitignore` leaves out
/// ([`searched`]) - the folders are skipped by name, never listed. The log says how many folders
/// were listed and skipped; its time stamps measure the search.
///
/// # Errors
///
/// The root does not list (no project: `plugins.error.not_available`).
pub fn discover(host: &mut dyn PluginHost, settings: &Settings) -> Result<Found, Failure> {
    let mut found = Found::default();
    let mut queue: VecDeque<(String, usize, Vec<Rc<Ignore>>)> =
        VecDeque::from([(String::new(), 0, Vec::new())]);
    let (mut searched_count, mut skipped) = (0_usize, 0_usize);
    host.log("info", "searching the project for todo files");
    while let Some((folder, depth, mut ignores)) = queue.pop_front() {
        searched_count += 1;
        if searched_count > MAX_FOLDERS {
            found.truncated = true;
            break;
        }
        let names = match host.project_list(&folder) {
            Ok(names) => names,
            Err(failure) if folder.is_empty() => return Err(failure),
            // A folder that went away or is refused: the others still count.
            Err(_) => continue,
        };
        if names.iter().any(|name| name == ignore::FILE)
            && let Some(own) = read_ignore(host, &folder)
        {
            ignores.push(own);
        }
        for name in names {
            if let Some(child) = name.strip_suffix('/') {
                let path = join(&folder, child);
                if depth < MAX_DEPTH && searched(settings, &ignores, child, &path) {
                    queue.push_back((path, depth + 1, ignores.clone()));
                } else {
                    skipped += 1;
                }
            } else {
                let path = join(&folder, &name);
                if path.ends_with(EXTENSION) && settings.wanted(&path) {
                    if found.paths.len() == MAX_FILES {
                        found.truncated = true;
                        continue;
                    }
                    found.paths.push(path);
                }
            }
        }
    }
    found.paths.sort();
    host.log(
        "info",
        &format!(
            "todo search: {} folders listed, {skipped} skipped, {} todo files{}",
            searched_count.min(MAX_FOLDERS),
            found.paths.len(),
            if found.truncated {
                " (stopped at a bound)"
            } else {
                ""
            },
        ),
    );
    Ok(found)
}

/// `path` as a caller names a todo file: a `.todo` file, not too long. The host holds it to the
/// project and the grants.
///
/// # Errors
///
/// `errors.bad_args` naming `file`.
pub fn check_path(path: &str) -> Result<(), Failure> {
    if path.is_empty() || path.chars().count() > MAX_PATH_CHARS || !path.ends_with(EXTENSION) {
        return Err(bad_args("file"));
    }
    Ok(())
}

/// The bytes of todo file `path` and its text.
///
/// # Errors
///
/// The host's (not found, not granted, untrusted for none), `errors.too_large`,
/// `errors.encoding` (not UTF-8).
pub fn read(host: &mut dyn PluginHost, path: &str) -> Result<(Vec<u8>, String), Failure> {
    check_path(path)?;
    let bytes = host.project_read(path)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(coded(FailureKind::Limit, "errors.too_large")
            .with_param("path", path)
            .with_param("limit", MAX_FILE_BYTES));
    }
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| coded(FailureKind::BadData, "errors.encoding").with_param("path", path))?;
    Ok((bytes, text))
}

/// Whether `failure` says the file is not there.
#[must_use]
pub fn is_missing(failure: &Failure) -> bool {
    failure.code() == Some(codes::NOT_FOUND)
}

/// Writes `bytes` to todo file `path` over `expected` (what was read; `None` for a new file).
///
/// # Errors
///
/// The host's: `plugins.error.write_conflict` when the file changed, `plugins.error.not_found`
/// for a new file's missing folder, ...; an untrusted workspace is `errors.untrusted`.
pub fn write(
    host: &mut dyn PluginHost,
    path: &str,
    bytes: &[u8],
    expected: Option<&[u8]>,
) -> Result<(), Failure> {
    host.project_write(path, bytes, expected)
        .map_err(|failure| {
            if failure.code() == Some(codes::UNTRUSTED_WORKSPACE) {
                coded(FailureKind::AccessDenied, "errors.untrusted").with_param("path", path)
            } else {
                failure
            }
        })
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
