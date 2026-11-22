//! The last step of a planned project write before the files are written: what a renamed
//! environment carried along (`env_rename::carry`, RB-094-24) is checked like the plan, and the
//! two project files are written in the order that keeps them valid on disk in between - as
//! MCP's `breakpoint_remove` does (RB-094-25).

use std::collections::BTreeSet;

use rb_config::ConfigFile;
use rb_config::breakpoint_refs::defined_ids;
use rb_ipc::SettingIssue;
use serde_json::Value;

use super::values::ProjectDocuments;
use super::write::validate_project;

/// The files of the write of `key` to `file` (`document`, with the other project file `also`),
/// in the order to write them. The other file goes first (it loses only references to a
/// breakpoint `file` still defines on disk, RB-090-61) unless it loses a breakpoint `file` never
/// defined - which `file` may still name until it is written - or a rename (`renamed`) is
/// carried, whose new ids `file` defines. After a rename both files are checked again.
///
/// # Errors
///
/// The issues of the files after the rename.
pub fn writes(
    project: &ProjectDocuments,
    key: &str,
    file: ConfigFile,
    document: Value,
    also: Option<(ConfigFile, Value)>,
    renamed: &[(String, String)],
) -> Result<Vec<(ConfigFile, Value)>, Vec<SettingIssue>> {
    if !renamed.is_empty() {
        let other = also.as_ref().map(|(_, other)| other);
        let (project_file, local_file) = match file {
            ConfigFile::Project => (&document, other.or(project.local_file.as_ref())),
            ConfigFile::Local => (other.unwrap_or(&project.project_file), Some(&document)),
        };
        validate_project(project_file, local_file, key)?;
    }
    let other_first = renamed.is_empty()
        && !also
            .as_ref()
            .is_some_and(|(other_file, other)| drops_foreign(project, file, *other_file, other));
    let main = (file, document);
    Ok(match also {
        Some(also) if other_first => vec![also, main],
        Some(also) => vec![main, also],
        None => vec![main],
    })
}

/// Whether `other` (the new content of `other_file`) loses a breakpoint definition that `file`
/// did not hold as stored.
fn drops_foreign(
    project: &ProjectDocuments,
    file: ConfigFile,
    other_file: ConfigFile,
    other: &Value,
) -> bool {
    let stored = |target: ConfigFile| match target {
        ConfigFile::Project => Some(&project.project_file),
        ConfigFile::Local => project.local_file.as_ref(),
    };
    let ids = |document: Option<&Value>| -> BTreeSet<String> {
        document.map_or_else(BTreeSet::new, |document| defined_ids(document, None))
    };
    let held = ids(stored(file));
    let kept = ids(Some(other));
    ids(stored(other_file))
        .iter()
        .any(|id| !kept.contains(id) && !held.contains(id))
}

#[cfg(test)]
#[path = "write_order_tests.rs"]
mod tests;
