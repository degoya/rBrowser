//! Requests of a folder as one file ([`Format`]): a Postman v2.1 collection, one `.http` file
//! (the requests one after the other, as REST Client and the JetBrains HTTP Client run them) or
//! a curl script for a shell. Nothing is resolved: the collection and the `.http` file keep
//! `{{variables}}`, the script reads each from the environment and names them at its top. A
//! body's file, written relative to its request's file, is put relative to the exported folder
//! ([`rebase`]), the one place every request of the export shares.

use rb_api_model::{ApiRequest, Body, to_http};

use crate::shell::Renderer;
use crate::word::Words;
use crate::{Format, Note, Shell, Variables, curl, postman};

/// A request of the export: its id below `.rbrowser/api/` and the request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub request: ApiRequest,
}

/// An export's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    pub text: String,
    /// The environment variables a script reads, each once.
    pub env: Vec<String>,
    pub notes: Vec<Note>,
}

/// The file name of an export of `stem`.
#[must_use]
pub fn file_name(stem: &str, format: Format, shell: Shell) -> String {
    let extension = match (format, shell) {
        (Format::Postman, _) => ".postman_collection.json",
        (Format::Http, _) => ".http",
        (Format::Curl, Shell::Posix) => ".sh",
        (Format::Curl, Shell::PowerShell) => ".ps1",
        (Format::Curl, Shell::Cmd) => ".cmd",
    };
    format!("{stem}{extension}")
}

/// `path` (relative to a request's file in `dir`, below the exported folder) relative to the
/// exported folder; an absolute path, a drive's and one starting with a variable stay as they
/// are.
#[must_use]
pub fn rebase(dir: &str, path: &str) -> String {
    if path.starts_with(['/', '\\']) || path.contains(':') || path.starts_with("{{") {
        return path.to_owned();
    }
    let mut parts: Vec<String> = dir
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    let mut ups = 0usize;
    for step in path.replace('\\', "/").split('/') {
        match step {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    ups += 1;
                }
            }
            other => parts.push(other.to_owned()),
        }
    }
    let joined = parts.join("/");
    if ups == 0 {
        format!("./{joined}")
    } else {
        format!("{}{joined}", "../".repeat(ups))
    }
}

/// `id` below the folder `base`.
fn relative<'a>(id: &'a str, base: &str) -> &'a str {
    let base = base.trim_matches('/');
    if base.is_empty() {
        return id;
    }
    id.strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(id)
}

/// `request` with its body's files rebased from its file's folder `dir`.
fn rebased(request: &ApiRequest, dir: &str) -> ApiRequest {
    let mut out = request.clone();
    match &mut out.body {
        Body::Binary { path } => *path = rebase(dir, path),
        Body::Multipart { parts } => {
            for part in parts {
                if let Some(file) = &mut part.file {
                    *file = rebase(dir, file);
                }
            }
        }
        _ => {}
    }
    out
}

/// The requests of `items`, the folder `base` exported, as `format`, the collection titled
/// `title`; a script for `shell`.
#[must_use]
pub fn export(title: &str, base: &str, items: &[Item], format: Format, shell: Shell) -> Exported {
    let mut notes = Vec::new();
    let moved: Vec<(String, ApiRequest)> = items
        .iter()
        .map(|item| {
            let id = relative(&item.id, base);
            let dir = id.rsplit_once('/').map_or("", |(dir, _)| dir);
            (id.to_owned(), rebased(&item.request, dir))
        })
        .collect();
    let (text, env) = match format {
        Format::Postman => (postman::collection(title, &moved, &mut notes), Vec::new()),
        Format::Http => (
            moved
                .iter()
                .map(|(_, request)| to_http(request))
                .collect::<Vec<_>>()
                .join("\n"),
            Vec::new(),
        ),
        Format::Curl => script(title, base, &moved, shell, &mut notes),
    };
    notes.sort_unstable();
    notes.dedup();
    Exported { text, env, notes }
}

fn script(
    title: &str,
    base: &str,
    items: &[(String, ApiRequest)],
    shell: Shell,
    notes: &mut Vec<Note>,
) -> (String, Vec<String>) {
    let comment = Renderer::new(shell, true).comment();
    let mut words = Words::new(Variables::Environment);
    let mut body = String::new();
    for (id, request) in items {
        let name = if request.name.trim().is_empty() {
            id.as_str()
        } else {
            request.name.trim()
        };
        body.push_str(&format!("\n{comment} {name}\n"));
        body.push_str(&curl::command(request, &mut words, shell, true, notes));
        body.push('\n');
    }
    let env = words.into_env();
    let mut head = match shell {
        Shell::Posix => "#!/bin/sh\n".to_owned(),
        Shell::PowerShell => String::new(),
        Shell::Cmd => "@echo off\n".to_owned(),
    };
    let count = if items.len() == 1 {
        "1 request".to_owned()
    } else {
        format!("{} requests", items.len())
    };
    head.push_str(&format!(
        "{comment} {title}: {count} from rBrowser's API client.\n"
    ));
    if !env.is_empty() {
        head.push_str(&format!("{comment} Set first: {}\n", env.join(", ")));
    }
    let folder = match base.trim_matches('/') {
        "" => rb_api_model::paths::API_DIR.to_owned(),
        base => format!("{}/{base}", rb_api_model::paths::API_DIR),
    };
    head.push_str(&format!("{comment} File paths are relative to {folder}.\n"));
    (format!("{head}{body}"), env)
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod tests;
