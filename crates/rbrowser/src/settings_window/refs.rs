//! A project write that removes a breakpoint takes its references along (RB-090-61): the cells,
//! hidden selections, mode fields, preset focus and compare list that named it, in the file
//! written and in the other project file - else the check refuses the write and the breakpoint
//! stays (`/layout/cells/13/breakpoint: No breakpoint has this id.`). The places are
//! `rb_config::breakpoint_refs`, which MCP's `breakpoint_remove` uses as well.
//!
//! The merge joins the `breakpoints` of `config.json` and `config.local.json` by id, so a
//! breakpoint the list written leaves out goes from both files first ([`drop_definitions`],
//! RB-094-25): written to one file alone, the other brought it back and Delete changed nothing.

use std::collections::BTreeSet;

use rb_config::ConfigFile;
use rb_config::breakpoint_refs::{defined_ids, drop_removed, remove_definitions};
use serde_json::Value;

use super::values::ProjectDocuments;

/// The settings key of the breakpoints.
pub const BREAKPOINTS_KEY: &str = "breakpoints";

/// The other project file's content as stored: `config.local.json` beside `config.json` and
/// the other way round.
pub fn other_file(project: &ProjectDocuments, file: ConfigFile) -> Option<Value> {
    match file {
        ConfigFile::Project => project.local_file.clone(),
        ConfigFile::Local => Some(project.project_file.clone()),
    }
}

/// The breakpoints `old` (the list in effect) has and `new` (the list written) leaves out: their
/// entries leave `old`, `document` (the file written) and `other` (the other project file), so
/// the rest of the write changes only what differs and no file brings one back. Returns whether
/// a breakpoint went.
pub fn drop_definitions(
    old: &mut Value,
    new: &Value,
    document: &mut Value,
    other: Option<&mut Value>,
) -> bool {
    let kept: BTreeSet<&str> = ids(new).collect();
    let gone: BTreeSet<String> = ids(old)
        .filter(|id| !kept.contains(id))
        .map(str::to_owned)
        .collect();
    if gone.is_empty() {
        return false;
    }
    if let Some(list) = old.as_array_mut() {
        list.retain(|entry| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .is_none_or(|id| !gone.contains(id))
        });
    }
    remove_definitions(&gone, document, other);
    true
}

fn ids(list: &Value) -> impl Iterator<Item = &str> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
}

/// Removes from `document` (the new content of `file`) and from `other` (the other project
/// file's new content, [`other_file`] when the write leaves it as it is) every reference to a
/// breakpoint the two files no longer define. Returns the other file's new content when it
/// differs from what is stored, to be written with `document` (`write_order`).
pub fn drop_dangling(
    project: &ProjectDocuments,
    file: ConfigFile,
    document: &mut Value,
    other: Option<Value>,
) -> Option<(ConfigFile, Value)> {
    let before = defined_ids(&project.project_file, project.local_file.as_ref());
    match file {
        ConfigFile::Project => {
            let mut local = other;
            drop_removed(&before, document, local.as_mut());
            local
                .filter(|local| Some(local) != project.local_file.as_ref())
                .map(|local| (ConfigFile::Local, local))
        }
        ConfigFile::Local => {
            let mut project_file = other.unwrap_or_else(|| project.project_file.clone());
            drop_removed(&before, &mut project_file, Some(document));
            (project_file != project.project_file).then_some((ConfigFile::Project, project_file))
        }
    }
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
