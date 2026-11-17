//! Edits of a project config file for `config_set` and `breakpoint_add`/`_update`/`_remove`:
//! a dotted key set or removed, a breakpoint added, merged or removed (with its references in
//! both project files, `rb_config::breakpoint_refs`) - pure JSON, checked by
//! the host before anything is written (`Backend::write_config_document`: migrated, merged,
//! `rb_config::check`). What workspace trust guards is refused here first: persisted request
//! headers (`emulation.headers`, `breakpoints[].headers`), the `network` keys, plugins,
//! extensions, the screenshot folder and the other protected parts are never written to a
//! project file over MCP. Live headers are another matter: `emulation_set { headers }` sets them
//! on the running views only, nothing is persisted, which T1 allows (an untrusted workspace
//! sends only live headers).

use rb_config::breakpoint_refs::{References, defined_ids, drop_removed};
use serde_json::{Map, Value};

use crate::backend::{ConfigScope, ToolFailure};

/// The code of a key MCP may not write.
pub const PROTECTED: &str = "config_key_protected";
/// The code of a breakpoint id that is not there (or there already).
pub const BREAKPOINT: &str = "breakpoint_not_found";

/// First segments of keys that workspace trust guards or that run code: refused over MCP.
const REFUSED_ROOTS: &[&str] = &[
    "network",
    "plugins",
    "extensions",
    "trust",
    "flows",
    "mocks",
    "snippets",
    "pageScripts",
];
/// Single keys refused with what lies below them: where screenshots, recordings and exports are
/// written is a person's choice, and only a person shortens the recorder's secret list.
const REFUSED_KEYS: &[&str] = &[
    "capture.dir",
    "automation.recorder.dir",
    "automation.recorderExport.dir",
    "automation.recorder.secretFields",
];
/// A segment refused wherever it stands: request headers (`emulation.headers`,
/// `breakpoints[].headers`).
const REFUSED_SEGMENT: &str = "headers";

/// Why `key` may not be written over MCP, or `None`.
#[must_use]
pub fn refusal(key: &str) -> Option<ToolFailure> {
    let mut segments = key.split('.');
    let root = segments.next().unwrap_or_default();
    let refused = REFUSED_ROOTS.contains(&root)
        || REFUSED_KEYS
            .iter()
            .any(|refused| is_at_or_below(key, refused))
        || key.split('.').any(|segment| segment == REFUSED_SEGMENT);
    refused.then(|| protected(key))
}

/// Why writing `value` to `key` is refused over MCP, or `None`: the key itself ([`refusal`]),
/// request headers anywhere in the value, or a refused key the value sets below `key`
/// (`capture` with a `dir`). The one list `config_set` and the host's `settings_set` both ask
/// (`settings_window::write::plan`), so the two cannot drift.
#[must_use]
pub fn write_refusal(key: &str, value: &Value) -> Option<ToolFailure> {
    if let Some(refused) = refusal(key) {
        return Some(refused);
    }
    let sets_refused_key = REFUSED_KEYS.iter().any(|refused| {
        refused
            .strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(|rest| {
                rest.split('.')
                    .try_fold(value, |node, segment| node.get(segment))
                    .is_some_and(|found| !found.is_null())
            })
    });
    (holds_refused(value) || sets_refused_key).then(|| protected(key))
}

/// Whether `key` is `refused` or a key below it.
fn is_at_or_below(key: &str, refused: &str) -> bool {
    key.strip_prefix(refused)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

fn protected(key: &str) -> ToolFailure {
    ToolFailure::new(
        PROTECTED,
        format!(
            "`{key}` is guarded by workspace trust (headers, network, plugins, extensions, the \
             screenshot folder); only a person can change it in rBrowser"
        ),
    )
}

/// Whether `value` holds a key that [`refusal`] refuses at any depth (`headers`).
fn holds_refused(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, child)| key == REFUSED_SEGMENT || holds_refused(child)),
        Value::Array(items) => items.iter().any(holds_refused),
        _ => false,
    }
}

/// Sets the dotted `key` of `document` to `value` (intermediate objects made); `null` removes
/// the key.
///
/// # Errors
///
/// A refused key or value, or a segment that is no object.
pub fn set(document: &mut Value, key: &str, value: Value) -> Result<(), ToolFailure> {
    if let Some(refused) = write_refusal(key, &value) {
        return Err(refused);
    }
    if key.is_empty() || key.split('.').any(str::is_empty) {
        return Err(invalid(format!("`{key}` is no dotted key")));
    }
    let segments: Vec<&str> = key.split('.').collect();
    let (last, parents) = segments
        .split_last()
        .ok_or_else(|| invalid("an empty key".to_owned()))?;
    let mut current = document;
    for segment in parents {
        let object = current
            .as_object_mut()
            .ok_or_else(|| invalid(format!("`{key}`: `{segment}` is inside no object")))?;
        current = object
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let object = current
        .as_object_mut()
        .ok_or_else(|| invalid(format!("`{key}` is inside no object")))?;
    if value.is_null() {
        object.remove(*last);
    } else {
        object.insert((*last).to_owned(), value);
    }
    Ok(())
}

fn invalid(message: String) -> ToolFailure {
    ToolFailure::new("invalid_arguments", message)
}

fn breakpoints(document: &mut Value) -> Result<&mut Vec<Value>, ToolFailure> {
    let object = document
        .as_object_mut()
        .ok_or_else(|| invalid("the config is no object".to_owned()))?;
    object
        .entry("breakpoints")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| invalid("`breakpoints` is no list".to_owned()))
}

fn id_of(breakpoint: &Value) -> Option<&str> {
    breakpoint.get("id").and_then(Value::as_str)
}

/// Adds `breakpoint` (an object with an `id`) at the end of `breakpoints`.
///
/// # Errors
///
/// No id, an id there already, or request headers in it.
pub fn add_breakpoint(
    document: &mut Value,
    breakpoint: Map<String, Value>,
) -> Result<(), ToolFailure> {
    let breakpoint = Value::Object(breakpoint);
    if holds_refused(&breakpoint) {
        return Err(protected("breakpoints[].headers"));
    }
    let id = id_of(&breakpoint)
        .ok_or_else(|| invalid("a breakpoint needs an `id`".to_owned()))?
        .to_owned();
    let list = breakpoints(document)?;
    if list.iter().any(|known| id_of(known) == Some(id.as_str())) {
        return Err(ToolFailure::new(
            BREAKPOINT,
            format!("a breakpoint `{id}` exists already; use breakpoint_update"),
        ));
    }
    list.push(breakpoint);
    Ok(())
}

/// Merges `fields` into the breakpoint `id` (objects deeply, `null` removes a field).
///
/// # Errors
///
/// No such breakpoint, a new `id`, or request headers in `fields`.
pub fn update_breakpoint(
    document: &mut Value,
    id: &str,
    fields: Map<String, Value>,
) -> Result<(), ToolFailure> {
    let fields = Value::Object(fields);
    if holds_refused(&fields) {
        return Err(protected("breakpoints[].headers"));
    }
    if fields.get("id").is_some_and(|new| new.as_str() != Some(id)) {
        return Err(invalid("the id of a breakpoint does not change".to_owned()));
    }
    let breakpoint = breakpoints(document)?
        .iter_mut()
        .find(|known| id_of(known) == Some(id))
        .ok_or_else(|| missing(id))?;
    merge(breakpoint, fields);
    Ok(())
}

/// Removes the breakpoint `id`.
///
/// # Errors
///
/// No such breakpoint.
pub fn remove_breakpoint(document: &mut Value, id: &str) -> Result<(), ToolFailure> {
    let list = breakpoints(document)?;
    let before = list.len();
    list.retain(|known| id_of(known) != Some(id));
    if list.len() == before {
        return Err(missing(id));
    }
    Ok(())
}

/// Removes the breakpoint `id` from `document` (the project file `scope`) and every reference to
/// it - cells, hidden selections, mode fields, a preset's focus, `envCompare.breakpoints` - from
/// `document` and from `other` (the other project file), unless `other` still defines it: the
/// settings window's own function, `rb_config::breakpoint_refs` (RB-090-61).
///
/// # Errors
///
/// No such breakpoint in `document`.
pub fn remove_breakpoint_everywhere(
    document: &mut Value,
    other: &mut Value,
    scope: ConfigScope,
    id: &str,
) -> Result<References, ToolFailure> {
    let before = match scope {
        ConfigScope::Project => defined_ids(document, Some(&*other)),
        ConfigScope::Local => defined_ids(other, Some(&*document)),
    };
    remove_breakpoint(document, id)?;
    Ok(match scope {
        ConfigScope::Project => drop_removed(&before, document, Some(other)),
        ConfigScope::Local => drop_removed(&before, other, Some(document)),
    })
}

fn missing(id: &str) -> ToolFailure {
    ToolFailure::new(BREAKPOINT, format!("no breakpoint has the id `{id}`"))
}

/// Merges `patch` into `target`: objects key by key, `null` removes, anything else replaces.
pub fn merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    target.remove(&key);
                } else if let Some(existing) = target.get_mut(&key) {
                    merge(existing, value);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (target, patch) => *target = patch,
    }
}

#[cfg(test)]
#[path = "config_edit_tests.rs"]
mod tests;
