//! The API client's import (RB-096-02): the files a person picked or pasted in the panel read by
//! `rb_api_import` and stored as one `.http` file per request below `.rbrowser/api/<folder>/`
//! (the collection's folders below that), the environments added to `http-client.env.json` -
//! a variable the file already has keeps its value. Nothing is overwritten: a request whose
//! file exists gets a free name (`-2`, ...). A preview reads only.
//!
//! The answer names each request (folder, title, method, URL template, id once written), each
//! environment with its variables' names (never a value), what could not be written and the
//! import's report; every secret of the source is already a `{{$secret ...}}` placeholder.

use rb_api_import::{Format, Import, ImportError, Source, collect::folder_name, import};
use rb_api_model::{EnvFile, paths};
use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Map, Value, json};

use super::error;
use super::protocol::ImportFile;
use super::store::Store;

/// The folder of an import whose files name no collection.
pub const DEFAULT_FOLDER: &str = "import";

/// What the panel asked to import.
#[derive(Clone, Debug)]
pub struct Wanted {
    pub files: Vec<ImportFile>,
    pub format: Option<String>,
    pub folder: Option<String>,
    pub environments: bool,
    pub preview: bool,
}

fn failure_of(problem: &ImportError) -> Failure {
    let failure = error(FailureKind::BadData, &format!("import_{}", problem.code()));
    match problem {
        ImportError::Unknown { source } => failure.with_param("source", source.clone()),
        ImportError::Unreadable { source, reason } => failure
            .with_param("source", source.clone())
            .with_param("reason", rb_util::text::cut(reason, 200)),
        ImportError::Empty | ImportError::TooLarge => failure,
    }
}

/// The folder below `.rbrowser/api/` an import goes to.
fn base_folder(wanted: Option<&str>, import: &Import) -> Result<String, Failure> {
    let folder = match wanted.map(|f| f.trim().trim_matches('/')) {
        Some(folder) if !folder.is_empty() => folder.to_owned(),
        _ => import
            .collection
            .as_deref()
            .map_or_else(|| DEFAULT_FOLDER.to_owned(), folder_name),
    };
    if paths::is_folder(&folder) && folder.split('/').count() == 1 {
        Ok(folder)
    } else {
        Err(error(FailureKind::BadData, "bad_id").with_param("id", rb_util::text::cut(&folder, 80)))
    }
}

/// Runs an import over `store`.
///
/// # Errors
///
/// `bad_format`, `import_empty`, `import_too_large`, `import_unknown_format`,
/// `import_unreadable`, `bad_id` for a folder that is none, and the environment file's.
pub fn run(store: &Store, wanted: Wanted) -> Result<Value, Failure> {
    let format = match wanted.format.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(id) => Some(Format::of(id).ok_or_else(|| {
            error(FailureKind::BadData, "bad_format")
                .with_param("format", rb_util::text::cut(id, 40))
        })?),
    };
    let sources: Vec<Source> = wanted
        .files
        .into_iter()
        .map(|file| Source::new(rb_util::text::cut(&file.name, 200), file.text))
        .collect();
    let imported = import(&sources, format).map_err(|problem| failure_of(&problem))?;
    let base = base_folder(wanted.folder.as_deref(), &imported)?;
    // The environments are merged and checked before any request is written, so an import that
    // cannot keep them writes nothing.
    let write_environments =
        wanted.environments && !wanted.preview && !imported.environments.is_empty();
    let (merged, kept) = if write_environments {
        let (merged, kept) = merged_environments(store, &imported)?;
        (Some(merged), kept)
    } else {
        (None, Vec::new())
    };
    let mut requests = Vec::new();
    let mut failed = Vec::new();
    for item in &imported.requests {
        let folder = if item.folder.is_empty() {
            base.clone()
        } else {
            format!("{base}/{}", item.folder)
        };
        let mut shown = json!({
            "folder": folder,
            "name": item.request.name,
            "method": item.request.method,
            "url": item.request.url,
        });
        if !wanted.preview {
            match store.save(None, Some(folder.as_str()), &item.request) {
                Ok(id) => shown["id"] = json!(id),
                Err(failure) => failed.push(json!({
                    "name": item.request.name,
                    "reason": super::failure_code(&failure),
                })),
            }
        }
        requests.push(shown);
    }
    if let Some(merged) = &merged {
        store.write_env_json(merged)?;
    }
    let environments: Vec<Value> = imported
        .environments
        .iter()
        .map(|environment| {
            json!({
                "name": environment.name,
                "variables": environment.variables.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            })
        })
        .collect();
    tracing::info!(
        requests = requests.len(),
        failed = failed.len(),
        preview = wanted.preview,
        "an API collection was imported"
    );
    Ok(json!({
        "preview": wanted.preview,
        "formats": imported.formats,
        "collection": imported.collection,
        "folder": base,
        "requests": requests,
        "environments": environments,
        "environmentsWritten": wanted.environments && !wanted.preview,
        "kept": kept,
        "failed": failed,
        "report": imported.report,
    }))
}

/// `http-client.env.json` with the import's environments added, checked; and the
/// `environment.variable` names it already had, which keep their value.
fn merged_environments(store: &Store, imported: &Import) -> Result<(Value, Vec<String>), Failure> {
    let mut file = match store.env_json()? {
        Value::Object(map) => map,
        Value::Null => Map::new(),
        _ => {
            return Err(error(FailureKind::BadData, "env_unreadable")
                .with_param("reason", "not an object of environments"));
        }
    };
    let mut kept = Vec::new();
    for environment in &imported.environments {
        let block = file
            .entry(environment.name.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(variables) = block else {
            kept.push(environment.name.clone());
            continue;
        };
        for variable in &environment.variables {
            if variables.contains_key(&variable.name) {
                kept.push(format!("{}.{}", environment.name, variable.name));
            } else {
                variables.insert(variable.name.clone(), Value::String(variable.value.clone()));
            }
        }
    }
    let merged = Value::Object(file);
    EnvFile::from_json(&merged).map_err(|reason| {
        error(FailureKind::Limit, "env_unreadable")
            .with_param("reason", rb_util::text::cut(&reason, 200))
    })?;
    Ok((merged, kept))
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
