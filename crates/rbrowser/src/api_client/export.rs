//! A request as code, and a folder of requests as a file (RB-096-03), over `rb_api_export`:
//!
//! - **`code`** (the panel's "Copy as", MCP's `api_code`): the saved request `id`, or the
//!   panel's draft, in curl, HTTPie, `fetch` or a Playwright test. With
//!   `api-client.codeVariables` `environment` (the default) the plain values of the environment
//!   in effect are put in - the project's `http-client.env.json`, the rBrowser environment's
//!   `baseUrl`, `$timestamp`, `$guid` - and every other variable becomes an environment
//!   variable of the shell or script; with `template` every `{{variable}}` stays. A private
//!   value (`api-client.secrets`) and a process variable (`RB_SECRET_*`) are never read here:
//!   a snippet names them, it never holds them. The shell is `api-client.codeShell`.
//! - **`export`** (the panel only - it writes a file): the requests of a folder, or every
//!   one, as a Postman v2.1 collection, one `.http` file or a curl script, written to
//!   `.rbrowser/out/api-client/exports/` (the project's output, not checked in; no link on the
//!   way followed). Nothing is resolved in an export.

use std::fs;
use std::path::{Path, PathBuf};

use rb_api_export::{Format, Item, Language, Options, Shell, Variables};
use rb_api_model::ApiRequest;
use rb_api_model::paths::{self, EXTENSION};
use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Map, Value, json};

use super::error;
use super::ops::{Context, environment_of, layers};
use super::resolve::Variables as Lookup;
use super::store::Store;

/// The settings key of the shell a command is written for.
pub const SHELL_KEY: &str = "api-client.codeShell";
/// The settings key of how a variable shows in the code.
pub const VARIABLES_KEY: &str = "api-client.codeVariables";
/// Where an export is written, below the project root.
pub const EXPORT_DIR: &str = ".rbrowser/out/api-client/exports";

/// How code is written: `api-client.codeShell` and `api-client.codeVariables`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeSettings {
    pub shell: Shell,
    pub variables: Variables,
}

impl Default for CodeSettings {
    /// PowerShell, the terminal Windows opens; the variables as environment variables, so a
    /// snippet runs once its secrets are set.
    fn default() -> Self {
        Self {
            shell: Shell::PowerShell,
            variables: Variables::Environment,
        }
    }
}

/// The settings values in effect; one that does not read is its default.
#[must_use]
pub fn settings_of(values: &Map<String, Value>) -> CodeSettings {
    let defaults = CodeSettings::default();
    CodeSettings {
        shell: values
            .get(SHELL_KEY)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or(defaults.shell),
        variables: values
            .get(VARIABLES_KEY)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or(defaults.variables),
    }
}

/// Reads no variable of the process: a snippet never holds a secret.
fn no_process(_name: &str) -> Option<String> {
    None
}

/// The saved request `id` or the draft `request` as code in `language`.
///
/// # Errors
///
/// Neither given (`bad_args`), the store's, `invalid_request`, `secrets_unreadable`.
pub fn code(
    ctx: &Context,
    store: &Store,
    id: Option<&str>,
    request: Option<ApiRequest>,
    language: Language,
    environment: Option<String>,
) -> Result<Value, Failure> {
    let request = match (request, id) {
        (Some(request), _) => request,
        (None, Some(id)) => store.read(id)?.0,
        (None, None) => {
            return Err(error(FailureKind::BadData, "bad_args").with_param("reason", "id"));
        }
    };
    request.check().map_err(|invalid| {
        error(FailureKind::BadData, "invalid_request").with_param("reason", invalid.reason())
    })?;
    let settings = ctx.settings.code;
    let environment = environment_of(ctx, environment);
    let request = match settings.variables {
        Variables::Environment => {
            let lookup = Lookup {
                layers: layers(ctx, store, environment.clone())?,
                env: &no_process,
                now: ctx.now,
            };
            rb_api_export::fill(&request, &lookup)
        }
        Variables::Template => request,
    };
    let code = rb_api_export::code(
        &request,
        language,
        Options {
            shell: settings.shell,
            variables: settings.variables,
        },
    );
    Ok(json!({
        "language": code.language,
        "text": code.text,
        "env": code.env,
        "notes": code.notes,
        "shell": settings.shell,
        "variables": settings.variables,
        "environment": environment,
    }))
}

/// Whether request `id` lies in `folder` (`""` holds every one).
fn in_folder(id: &str, folder: &str) -> bool {
    folder.is_empty()
        || id
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The requests of `folder` (every one without it) as `format`, written below
/// [`EXPORT_DIR`]; the file's path, the count, the requests that did not read.
///
/// # Errors
///
/// A bad folder (`bad_id`), no request in it (`nothing_to_export`), the store's, a link on the
/// way or a write that failed (`not_exported`).
pub fn export(
    ctx: &Context,
    store: &Store,
    root: &Path,
    folder: Option<&str>,
    format: Format,
) -> Result<Value, Failure> {
    let folder = folder.unwrap_or_default().trim_matches('/');
    if !paths::is_folder(folder) {
        return Err(
            error(FailureKind::BadData, "bad_id").with_param("id", rb_util::text::cut(folder, 80))
        );
    }
    let mut items = Vec::new();
    let mut skipped = Vec::new();
    for listed in store.list()? {
        if !in_folder(&listed.id, folder) {
            continue;
        }
        match store.read(&listed.id) {
            Ok((request, _)) => items.push(Item {
                id: listed.id,
                request,
            }),
            Err(_) => skipped.push(listed.id),
        }
    }
    if items.is_empty() {
        return Err(error(FailureKind::NotFound, "nothing_to_export").with_param("folder", folder));
    }
    let title = if folder.is_empty() {
        root.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "API client".to_owned())
    } else {
        folder.to_owned()
    };
    let shell = ctx.settings.code.shell;
    let stem = paths::file_name(&title);
    let stem = stem.strip_suffix(EXTENSION).unwrap_or(&stem);
    let name = rb_api_export::file_name(stem, format, shell);
    let exported = rb_api_export::export(&title, folder, &items, format, shell);
    let relative = format!("{EXPORT_DIR}/{name}");
    write(root, &relative, exported.text.as_bytes())?;
    Ok(json!({
        "path": relative,
        "format": format,
        "count": items.len(),
        "skipped": skipped,
        "env": exported.env,
        "notes": exported.notes,
    }))
}

/// `relative` below `root`, when no step on the way is a link.
fn path_below(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for step in relative.split('/') {
        path.push(step);
        if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return None;
        }
    }
    Some(path)
}

fn write(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), Failure> {
    let not_written = |reason: String| {
        error(FailureKind::Io, "not_exported")
            .with_param("reason", rb_util::text::cut(&reason, 200))
    };
    let link = || error(FailureKind::AccessDenied, "link");
    let path = path_below(root, relative).ok_or_else(link)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| not_written(e.kind().to_string()))?;
    }
    // The folders just made are checked again: none may have become a link meanwhile.
    let path = path_below(root, relative).ok_or_else(link)?;
    rb_util::fs::write_atomic(&path, bytes).map_err(|e| not_written(e.kind().to_string()))
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
