//! The tools: `list` reads in pages ([`page`]), `add`, `toggle`, `edit` and `archive` change one
//! file each (`archive` without a file: every one). A change reads the file, checks the caller's
//! `rev` (the version it was shown), changes the document and writes it back over the bytes
//! read; the answer carries the first page of the file as it is now and posts
//! `{ "changed": <path> }` to the panel, so an open panel shows a change made over MCP.

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::clock::{self, MAX_OFFSET_MINUTES};
use crate::config::Settings;
use crate::edit::{self, Refusal};
use crate::files::{self, Found};
use crate::format::{Document, Kind, State, revision};
use crate::page::{self, ENTRIES_BYTES};
use crate::search::{MAX_QUERY_CHARS, Query};
use crate::{POST_KEY, bad_args, coded};

/// The todo file a new project gets when the project has none.
pub const DEFAULT_FILE: &str = "TODO.todo";

fn refused(name: Refusal) -> Failure {
    coded(FailureKind::BadData, name)
}

fn optional_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, Failure> {
    match &args[key] {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.as_str())),
        _ => Err(bad_args(key)),
    }
}

/// The 0-based index of the argument `key`, a 1-based line number.
fn line_of(args: &Value, key: &str) -> Result<Option<usize>, Failure> {
    match &args[key] {
        Value::Null => Ok(None),
        value => value
            .as_u64()
            .filter(|line| *line >= 1)
            .and_then(|line| usize::try_from(line - 1).ok())
            .map(Some)
            .ok_or_else(|| bad_args(key)),
    }
}

/// The 0-based index of the argument `key` (`from`); 0 without one.
fn index_of(args: &Value, key: &str) -> Result<usize, Failure> {
    match &args[key] {
        Value::Null => Ok(0),
        value => value
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| bad_args(key)),
    }
}

/// Refuses `text` of `path` when the caller's `rev` names another version of it.
fn check_rev(path: &str, rev: Option<&str>, text: &str) -> Result<(), Failure> {
    match rev {
        Some(rev) if rev != revision(text.as_bytes()) => {
            Err(coded(FailureKind::Other, "errors.changed").with_param("path", path))
        }
        _ => Ok(()),
    }
}

/// `utcOffset`: minutes east of UTC; 0 without one.
fn offset_of(args: &Value) -> Result<i64, Failure> {
    match &args["utcOffset"] {
        Value::Null => Ok(0),
        value => value
            .as_i64()
            .filter(|minutes| minutes.abs() <= MAX_OFFSET_MINUTES)
            .ok_or_else(|| bad_args("utcOffset")),
    }
}

fn timestamp(host: &mut dyn PluginHost, args: &Value) -> Result<String, Failure> {
    let offset = offset_of(args)?;
    let format = Settings::read(host).format;
    Ok(clock::stamp(host.now_unix_seconds(), offset, format))
}

/// The todo file the argument `file` names; none is refused.
fn file_of<'a>(args: &'a Value) -> Result<&'a str, Failure> {
    let path = optional_str(args, "file")?.ok_or_else(|| bad_args("file"))?;
    files::check_path(path)?;
    Ok(path)
}

/// Reads `path`, checks `rev`, runs `apply` on its document and writes the result over the
/// bytes read (nothing when nothing changed). `missing_is_new` reads a file that is not there as
/// empty and creates it. The answer: `apply`'s with the file as it is now and the panel's post.
fn change(
    host: &mut dyn PluginHost,
    args: &Value,
    path: &str,
    missing_is_new: bool,
    apply: impl FnOnce(&mut Document) -> Result<Value, Refusal>,
) -> Result<Value, Failure> {
    let (old, text) = match files::read(host, path) {
        Ok(read) => (Some(read.0), read.1),
        Err(failure) if missing_is_new && files::is_missing(&failure) => (None, String::new()),
        Err(failure) => return Err(failure),
    };
    check_rev(path, optional_str(args, "rev")?, &text)?;
    let mut document = Document::parse(&text);
    let mut answer = apply(&mut document).map_err(refused)?;
    let rendered = document.render();
    if old.is_none() || rendered != text {
        files::write(host, path, rendered.as_bytes(), old.as_deref())?;
    }
    answer["file"] = page::file_page(path, &rendered, 0);
    answer[POST_KEY] = json!([{ "target": "panel", "message": { "changed": path } }]);
    Ok(answer)
}

/// The paths of the argument `files`, each a todo file; `None` without it.
fn paths_of(args: &Value) -> Result<Option<Vec<String>>, Failure> {
    let list = match &args["files"] {
        Value::Null => return Ok(None),
        Value::Array(list) if list.len() <= files::MAX_FILES => list,
        _ => return Err(bad_args("files")),
    };
    list.iter()
        .map(|path| {
            let path = path.as_str().ok_or_else(|| bad_args("files"))?;
            files::check_path(path)?;
            Ok(path.to_owned())
        })
        .collect::<Result<Vec<_>, Failure>>()
        .map(Some)
}

/// The search of the argument `query` ([`Query`]); `None` without one or with no term in it.
fn query_of(args: &Value) -> Result<Option<Query>, Failure> {
    match optional_str(args, "query")? {
        Some(text) if text.chars().count() > MAX_QUERY_CHARS => Err(bad_args("query")),
        Some(text) => Ok(Query::parse(text)),
        None => Ok(None),
    }
}

/// `list`, in pages that fit a panel's message ([`page`]): without `file` the heads (path, rev,
/// counts) of every todo file of the settings - or of the `files` named, the panel's look for
/// changes without searching the folders again - from file `from`; with `file` that file's items
/// from item `from`, refused as `errors.changed` when `rev` names another version (the file
/// changed between two pages). `next`, while more remain, is the `from` to go on with. With
/// `query` (RB-110-18) the heads of the files with a match only, each with `matches`, and a
/// file's matches with their context ([`crate::search`]).
///
/// # Errors
///
/// Arguments that do not read (`rev` needs `file`); no project; the one file's read, its
/// `errors.changed`.
pub fn list(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let from = index_of(args, "from")?;
    let rev = optional_str(args, "rev")?;
    let query = query_of(args)?;
    let named = match (optional_str(args, "file")?, paths_of(args)?) {
        (Some(_), Some(_)) => return Err(bad_args("files")),
        (Some(path), None) => {
            let (_, text) = files::read(host, path)?;
            check_rev(path, rev, &text)?;
            return Ok(match &query {
                Some(query) => page::search_page(path, &text, from, query),
                None => page::file_page(path, &text, from),
            });
        }
        (None, paths) => paths,
    };
    if rev.is_some() {
        return Err(bad_args("rev"));
    }
    let found = match named {
        Some(paths) => Found {
            paths,
            truncated: false,
        },
        None => {
            let settings = Settings::read(host);
            files::discover(host, &settings)?
        }
    };
    if let Some(query) = query {
        return Ok(search_heads(host, &found, from, &query));
    }
    let (heads, next) = page::take(found.paths.len(), from, ENTRIES_BYTES, |index| {
        let Some(path) = found.paths.get(index) else {
            return Value::Null;
        };
        match files::read(host, path) {
            Ok((_, text)) => page::head(path, &text),
            Err(failure) => failure_entry(path, &failure),
        }
    });
    let mut answer = json!({ "files": heads, "truncated": found.truncated });
    if let Some(next) = next {
        answer["next"] = json!(next);
    }
    Ok(answer)
}

/// `list`'s heads with a `query`: every file is read for its matches, the files without one are
/// left out, and the pages count the files with one (or that failed) from `from`.
fn search_heads(host: &mut dyn PluginHost, found: &Found, from: usize, query: &Query) -> Value {
    let entries: Vec<Value> = found
        .paths
        .iter()
        .filter_map(|path| match files::read(host, path) {
            Ok((_, text)) => page::search_head(path, &text, query),
            Err(failure) => Some(failure_entry(path, &failure)),
        })
        .collect();
    let (heads, next) = page::take(entries.len(), from, ENTRIES_BYTES, |index| {
        entries.get(index).cloned().unwrap_or_default()
    });
    let mut answer = json!({ "files": heads, "truncated": found.truncated });
    if let Some(next) = next {
        answer["next"] = json!(next);
    }
    answer
}

/// The entry of a file that failed: its path, the failure's code and parameters.
fn failure_entry(path: &str, failure: &Failure) -> Value {
    json!({ "path": path, "error": failure.code(), "params": failure.params })
}

/// `add`: a new open task `text` in `file` (the project's only todo file without one, or
/// `TODO.todo` when it has none - made then), below the project or task at `under` (a line) or
/// the project `project` (`Website.Design`; a top-level one is made when missing), else at the
/// top level.
///
/// # Errors
///
/// Arguments that do not read, `errors.several_files`, `errors.no_file` (none the settings
/// show, and `TODO.todo` not among them), `errors.search_incomplete` (a bound stopped the
/// search), the change's or the write's.
pub fn add(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let text = optional_str(args, "text")?.ok_or_else(|| bad_args("text"))?;
    let under = line_of(args, "under")?;
    let project = optional_str(args, "project")?;
    if under.is_some() && project.is_some() {
        return Err(bad_args("under"));
    }
    let path = match optional_str(args, "file")? {
        Some(_) => file_of(args)?.to_owned(),
        None => {
            let settings = Settings::read(host);
            let found = files::discover(host, &settings)?;
            // A stopped search may have missed the file meant, and a TODO.todo the settings
            // leave out would not show: say which file instead of guessing one.
            if found.truncated {
                return Err(coded(FailureKind::Limit, "errors.search_incomplete"));
            }
            match found.paths.as_slice() {
                [] if settings.wanted(DEFAULT_FILE) => DEFAULT_FILE.to_owned(),
                [] => return Err(coded(FailureKind::NotFound, "errors.no_file")),
                [only] => only.clone(),
                _ => return Err(coded(FailureKind::BadData, "errors.several_files")),
            }
        }
    };
    change(host, args, &path, true, |document| {
        let parent = match (under, project) {
            (Some(line), _) => Some(line),
            (None, Some(name)) => Some(match edit::find_project(document, name) {
                Some(line) => line,
                None if name.contains('.') => return Err("errors.no_project"),
                None => edit::add_project(document, name)?,
            }),
            (None, None) => None,
        };
        let line = edit::add(document, text, parent)?;
        Ok(json!({ "line": line + 1 }))
    })
}

/// `toggle`: task `line` of `file` to `state` (`open`, `done`, `cancelled`), or toggled - open to
/// done, done and cancelled to open - with the time in `utcOffset` (minutes east of UTC).
///
/// # Errors
///
/// Arguments that do not read, `errors.not_a_task`, the change's or the write's.
pub fn toggle(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let path = file_of(args)?.to_owned();
    let index = line_of(args, "line")?.ok_or_else(|| bad_args("line"))?;
    let wanted = match optional_str(args, "state")? {
        Some(name) => Some(State::parse(name).ok_or_else(|| bad_args("state"))?),
        None => None,
    };
    let stamp = timestamp(host, args)?;
    change(host, args, &path, false, |document| {
        let Some(Kind::Task(current)) = document.parsed(index).map(|parsed| parsed.kind) else {
            return Err(if index < document.lines.len() {
                "errors.not_a_task"
            } else {
                "errors.no_line"
            });
        };
        let state = wanted.unwrap_or_else(|| edit::toggled(current));
        edit::set_state(document, index, state, &stamp)?;
        Ok(json!({ "line": index + 1, "state": state.as_str() }))
    })
}

/// `edit`: item `line` of `file` - `action` `text` (with `text`), `delete` (with its children),
/// `up` or `down` (swapped with its sibling).
///
/// # Errors
///
/// Arguments that do not read, the change's or the write's.
pub fn edit(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let path = file_of(args)?.to_owned();
    let index = line_of(args, "line")?.ok_or_else(|| bad_args("line"))?;
    let action = optional_str(args, "action")?.ok_or_else(|| bad_args("action"))?;
    let text = optional_str(args, "text")?;
    if !matches!(action, "text" | "delete" | "up" | "down") || (action == "text") != text.is_some()
    {
        return Err(bad_args("action"));
    }
    change(host, args, &path, false, |document| {
        let line = match (action, text) {
            ("text", Some(text)) => edit::set_text(document, index, text).map(|()| Some(index))?,
            ("delete", _) => edit::delete(document, index).map(|()| None)?,
            (direction, _) => Some(edit::move_item(document, index, direction == "up")?),
        };
        Ok(json!({ "line": line.map(|line| line + 1) }))
    })
}

/// `archive`: Todo+' "Archive" in `file` (its failure the answer's), or in every todo file
/// without one (a file that fails is named in `failed`, the others are archived). `files` are
/// the heads of the files changed (their items are `list`'s), `omitted` counts the entries of
/// `files` and `failed` half a page did not hold.
///
/// # Errors
///
/// Arguments that do not read (`rev` needs `file`), no project, the one file's failure.
pub fn archive(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let single = optional_str(args, "file")?.is_some();
    if !single && optional_str(args, "rev")?.is_some() {
        return Err(bad_args("rev"));
    }
    let paths = if single {
        vec![file_of(args)?.to_owned()]
    } else {
        let settings = Settings::read(host);
        files::discover(host, &settings)?.paths
    };
    let mut archived = 0;
    let mut changed = Vec::new();
    let mut failures = Vec::new();
    for path in &paths {
        let mut moved = 0;
        let outcome = change(host, args, path, false, |document| {
            moved = edit::archive(document);
            Ok(json!({}))
        });
        match outcome {
            Ok(answer) if moved > 0 => {
                archived += moved;
                let file = &answer["file"];
                changed.push(json!({ "path": file["path"], "rev": file["rev"],
                                     "counts": file["counts"] }));
            }
            Ok(_) => {}
            Err(failure) if single => return Err(failure),
            Err(failure) => failures.push(failure_entry(path, &failure)),
        }
    }
    // Heads only, and as many as half a page holds each: the rest is counted in `omitted`.
    let (files, more) = page::take(changed.len(), 0, ENTRIES_BYTES / 2, |index| {
        changed.get(index).cloned().unwrap_or_default()
    });
    let (failed, more_failed) = page::take(failures.len(), 0, ENTRIES_BYTES / 2, |index| {
        failures.get(index).cloned().unwrap_or_default()
    });
    let omitted = more.map_or(0, |from| changed.len() - from)
        + more_failed.map_or(0, |from| failures.len() - from);
    Ok(json!({
        "archived": archived,
        "files": files,
        "failed": failed,
        "omitted": omitted,
        POST_KEY: [{ "target": "panel", "message": { "changed": Value::Null } }],
    }))
}
