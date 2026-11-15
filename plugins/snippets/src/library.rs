//! Saving from the panel (RB-070-04): the snippets the panel's editor saves live in
//! `.rbrowser/out/snippets/library.json` (`{ "version": 1, "snippets": [{ "name", "code" }] }`),
//! the one place a plugin may write. The file lies in the project tree, so it is read and
//! written only in a trusted workspace (`catalog::probe`); the person's snippets for every
//! project are `snippets.library` in the settings.
use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::catalog::{self, LIBRARY_FILE, MAX_CODE_CHARS, MAX_PER_SOURCE, Probe};
use crate::header::{self, MAX_NAME_CHARS};

fn refused_unless_trusted(host: &mut dyn PluginHost) -> Result<(), Failure> {
    match catalog::probe(host) {
        Probe::Trusted(_) => Ok(()),
        Probe::Untrusted => Err(crate::coded(
            FailureKind::AccessDenied,
            "errors.save_untrusted",
        )),
        Probe::NoProject => Err(crate::coded(FailureKind::NotSupported, "errors.no_project")),
    }
}

fn write(host: &mut dyn PluginHost, entries: &[Value]) -> Result<(), Failure> {
    let library = json!({ "version": 1, "snippets": entries });
    let text = serde_json::to_string_pretty(&library)
        .map_err(|_| crate::coded(FailureKind::Other, "errors.save_failed"))?;
    host.out_write(LIBRARY_FILE, text.as_bytes())
}

/// `save`: `{ name, code }` saves (replacing one of that name), `{ name, delete: true }` removes.
/// Answers `{ saved | deleted: name }` and tells the panels to read the list again.
///
/// # Errors
///
/// An untrusted workspace or none, a name or code that does not read, a header that does not,
/// a full library, an unknown name to delete, or the write's failure.
pub fn save(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let name = args["name"]
        .as_str()
        .and_then(|name| header::line_text(name, MAX_NAME_CHARS))
        .ok_or_else(|| crate::bad_args("name"))?;
    refused_unless_trusted(host)?;
    let mut entries = catalog::saved_entries(host);
    let at = entries
        .iter()
        .position(|entry| entry["name"] == name.as_str());
    let post = json!([{ "target": "panel", "message": { "changed": true } }]);
    if args["delete"] == json!(true) {
        let at = at.ok_or_else(|| {
            crate::coded(FailureKind::NotFound, "errors.not_found").with_param("name", &name)
        })?;
        entries.remove(at);
        write(host, &entries)?;
        return Ok(json!({ "deleted": name, "$post": post }));
    }
    let code = args["code"]
        .as_str()
        .ok_or_else(|| crate::bad_args("code"))?;
    if code.trim().is_empty() {
        return Err(crate::coded(FailureKind::BadData, "errors.empty"));
    }
    if code.chars().count() > MAX_CODE_CHARS {
        return Err(crate::coded(FailureKind::Limit, "errors.code_too_long")
            .with_param("limit", MAX_CODE_CHARS));
    }
    if let Err(bad) = header::parse(code) {
        return Err(crate::coded(FailureKind::BadData, "errors.header").with_param("tag", bad.tag));
    }
    let entry = json!({ "name": name, "code": code });
    match at {
        Some(at) => entries[at] = entry,
        None if entries.len() >= MAX_PER_SOURCE => {
            return Err(crate::coded(FailureKind::Limit, "errors.library_full")
                .with_param("limit", MAX_PER_SOURCE));
        }
        None => entries.push(entry),
    }
    write(host, &entries)?;
    Ok(json!({ "saved": name, "$post": post }))
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
