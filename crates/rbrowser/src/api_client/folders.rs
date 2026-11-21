//! The collections as folders (RB-096-11): a folder below `.rbrowser/api/` made, renamed,
//! duplicated or deleted, a request moved into another one or duplicated beside itself. Every
//! folder is checked like an id ([`collection_ok`]: no `.`/`..` or hidden segment, a few levels,
//! never a name ending in `.http`), nothing is written over what is there (`exists`), and no link
//! is followed: a link on the way is refused, a link inside a folder is neither copied nor
//! counted, and a delete removes the link, never what it points to.

use std::fs;
use std::path::Path;

use rb_api_model::paths::{self, EXTENSION, MAX_DEPTH};
use rb_plugin_types::{Failure, FailureKind};

use super::error;
use super::store::{MAX_REQUESTS, Store, io_failure};

/// Whether `folder` (trimmed of `/`) may be a collection: a folder's path, not the top, at most
/// [`MAX_DEPTH`] levels, no segment that reads as a request's file.
#[must_use]
pub fn collection_ok(folder: &str) -> bool {
    !folder.is_empty()
        && paths::is_folder(folder)
        && folder.split('/').count() <= MAX_DEPTH
        && !folder
            .split('/')
            .any(|segment| segment.ends_with(EXTENSION))
}

fn bad_folder(folder: &str) -> Failure {
    error(FailureKind::BadData, "bad_folder").with_param("folder", rb_util::text::cut(folder, 80))
}

fn checked(folder: &str) -> Result<String, Failure> {
    let folder = folder.trim_matches('/');
    if collection_ok(folder) {
        Ok(folder.to_owned())
    } else {
        Err(bad_folder(folder))
    }
}

fn exists(what: &str) -> Failure {
    error(FailureKind::BadData, "exists").with_param("name", rb_util::text::cut(what, 80))
}

/// How many levels of folders lie below `dir`, through no link; at most `most` are looked at.
fn levels_below(dir: &Path, most: usize) -> usize {
    if most == 0 {
        return 0;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && paths::is_folder(&entry.file_name().to_string_lossy())
        })
        .map(|entry| 1 + levels_below(&entry.path(), most - 1))
        .max()
        .unwrap_or(0)
}

/// The requests below `dir`, through no link.
fn count(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => count(&entry.path()),
            Ok(kind)
                if kind.is_file() && entry.file_name().to_string_lossy().ends_with(EXTENSION) =>
            {
                1
            }
            _ => 0,
        })
        .sum()
}

/// Copies the folders and `.http` files of `from` into the new folder `to`; the requests copied.
fn copy(from: &Path, to: &Path, depth: usize, copied: &mut usize) -> Result<(), Failure> {
    fs::create_dir(to).map_err(|e| io_failure("not_written", &e))?;
    let entries = fs::read_dir(from).map_err(|e| io_failure("unreadable", &e))?;
    for entry in entries.flatten() {
        if *copied >= MAX_REQUESTS {
            return Ok(());
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if kind.is_dir() && depth < MAX_DEPTH && paths::is_folder(&text) {
            copy(&entry.path(), &to.join(&name), depth + 1, copied)?;
        } else if kind.is_file() && paths::is_request_id(&text) {
            fs::copy(entry.path(), to.join(&name)).map_err(|e| io_failure("not_written", &e))?;
            *copied += 1;
        }
    }
    Ok(())
}

impl Store {
    /// Every folder below the collections, sorted - an empty one too.
    ///
    /// # Errors
    ///
    /// The folder is a link, or does not read.
    pub fn folders(&self) -> Result<Vec<String>, Failure> {
        fn walk(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<String>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let id = paths::join(prefix, &name);
                let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
                if is_dir && depth < MAX_DEPTH && collection_ok(&id) && out.len() < MAX_REQUESTS {
                    out.push(id.clone());
                    walk(&entry.path(), &id, depth + 1, out);
                }
            }
        }
        let dir = self.inside("")?;
        let mut out = Vec::new();
        if dir.is_dir() {
            walk(&dir, "", 0, &mut out);
        }
        out.sort();
        Ok(out)
    }

    /// The folder `folder` when it is one: checked, through no link.
    fn existing(&self, folder: &str) -> Result<(String, std::path::PathBuf), Failure> {
        let folder = checked(folder)?;
        let path = self.inside(&folder)?;
        if !path.is_dir() {
            return Err(
                error(FailureKind::NotFound, "folder_not_found").with_param("folder", folder)
            );
        }
        Ok((folder, path))
    }

    /// `folder` as a free path, its parents made; `exists` when something is there.
    fn free(&self, folder: &str) -> Result<(String, std::path::PathBuf), Failure> {
        let folder = checked(folder)?;
        let path = self.inside(&folder)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(exists(&folder));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_failure("not_written", &e))?;
        }
        // The folders just made are checked again: none may have become a link meanwhile.
        let path = self.inside(&folder)?;
        Ok((folder, path))
    }

    /// Makes the collection `folder`; its path.
    ///
    /// # Errors
    ///
    /// `bad_folder`, `exists`, a link on the way, a write that failed.
    pub fn create_folder(&self, folder: &str) -> Result<String, Failure> {
        let (folder, path) = self.free(folder)?;
        fs::create_dir(&path).map_err(|e| io_failure("not_written", &e))?;
        Ok(folder)
    }

    /// Renames the collection `from` to `to`, never over another nor into itself; the new path.
    ///
    /// # Errors
    ///
    /// `bad_folder`, `folder_not_found`, `exists`, `too_deep` (a subfolder would lie deeper than
    /// [`MAX_DEPTH`] levels), a link, a rename that failed.
    pub fn rename_folder(&self, from: &str, to: &str) -> Result<String, Failure> {
        let (from, source) = self.existing(from)?;
        let target = checked(to)?;
        if target == from || target.starts_with(&format!("{from}/")) {
            return Err(bad_folder(&target));
        }
        // The moved subfolders stay within the levels the panel lists (the w32 review).
        let depth = target.split('/').count();
        if depth + levels_below(&source, MAX_DEPTH) > MAX_DEPTH {
            return Err(error(FailureKind::BadData, "too_deep")
                .with_param("folder", rb_util::text::cut(&target, 80))
                .with_param("most", MAX_DEPTH));
        }
        let (to, path) = self.free(&target)?;
        fs::rename(&source, &path).map_err(|e| io_failure("not_written", &e))?;
        Ok(to)
    }

    /// Copies the collection `folder` to the first free `<folder>-copy`, `-copy-2`, ...; the copy's
    /// path and the requests in it.
    ///
    /// # Errors
    ///
    /// `bad_folder`, `folder_not_found`, `exists` when no name is free, a link, a write that failed.
    pub fn duplicate_folder(&self, folder: &str) -> Result<(String, usize), Failure> {
        let (folder, source) = self.existing(folder)?;
        let wanted = format!("{folder}-copy");
        let free = (1..100)
            .map(|n| {
                if n == 1 {
                    wanted.clone()
                } else {
                    format!("{wanted}-{n}")
                }
            })
            .find(|name| {
                checked(name).is_ok()
                    && self
                        .inside(name)
                        .is_ok_and(|path| fs::symlink_metadata(path).is_err())
            })
            .ok_or_else(|| exists(&wanted))?;
        let (to, path) = self.free(&free)?;
        let depth = to.split('/').count();
        let mut copied = 0;
        copy(&source, &path, depth, &mut copied)?;
        Ok((to, copied))
    }

    /// Deletes the collection `folder` and every request in it; how many there were.
    ///
    /// # Errors
    ///
    /// `bad_folder` (the top among them), `folder_not_found`, a link, a delete that failed.
    pub fn delete_folder(&self, folder: &str) -> Result<usize, Failure> {
        let (_, path) = self.existing(folder)?;
        let requests = count(&path);
        // `remove_dir_all` removes a link inside, never what it points to.
        fs::remove_dir_all(&path).map_err(|e| io_failure("not_written", &e))?;
        Ok(requests)
    }

    /// Moves the request `id` into `folder` (`""` the top), its file name kept; its new id.
    ///
    /// # Errors
    ///
    /// A bad id or folder, `not_found`, `exists`, a link, a rename that failed.
    pub fn move_request(&self, id: &str, folder: &str) -> Result<String, Failure> {
        let source = self.path_of(id)?;
        if !source.is_file() {
            return Err(error(FailureKind::NotFound, "not_found").with_param("id", id));
        }
        let folder = folder.trim_matches('/');
        if !folder.is_empty() {
            checked(folder)?;
        }
        let name = id.rsplit('/').next().unwrap_or(id);
        let target = paths::join(folder, name);
        let path = self.path_of(&target)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(exists(&target));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_failure("not_written", &e))?;
        }
        let path = self.path_of(&target)?;
        fs::rename(&source, &path).map_err(|e| io_failure("not_written", &e))?;
        Ok(target)
    }

    /// A copy of the request `id` in its folder, titled `name` (its own title without one), in a
    /// free file named after the title; the copy's id.
    ///
    /// # Errors
    ///
    /// [`Store::read`]'s and [`Store::save`]'s.
    pub fn duplicate(&self, id: &str, name: Option<&str>) -> Result<String, Failure> {
        let (mut request, _) = self.read(id)?;
        if let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) {
            request.name = name.to_owned();
        }
        let folder = id.rsplit_once('/').map_or("", |(folder, _)| folder);
        self.save(None, Some(folder), &request)
    }
}

#[cfg(test)]
#[path = "folders_tests.rs"]
mod tests;
