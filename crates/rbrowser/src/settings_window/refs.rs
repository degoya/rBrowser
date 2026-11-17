//! A project write that removes a breakpoint takes its references along (RB-090-61): the cells,
//! hidden selections, mode fields, preset focus and compare list that named it, in the file
//! written and in the other project file - else the check refuses the write and the breakpoint
//! stays (`/layout/cells/13/breakpoint: No breakpoint has this id.`). The places are
//! `rb_config::breakpoint_refs`, which MCP's `breakpoint_remove` uses as well.

use rb_config::ConfigFile;
use rb_config::breakpoint_refs::{defined_ids, drop_removed};
use serde_json::Value;

use super::values::ProjectDocuments;

/// Removes from `document` (the new content of `file`) and from the other project file every
/// reference to a breakpoint the write removed. Returns the other file's new content when it
/// changed, to be written before `document`.
pub fn drop_dangling(
    project: &ProjectDocuments,
    file: ConfigFile,
    document: &mut Value,
) -> Option<(ConfigFile, Value)> {
    let before = defined_ids(&project.project_file, project.local_file.as_ref());
    match file {
        ConfigFile::Project => {
            let mut local = project.local_file.clone();
            drop_removed(&before, document, local.as_mut());
            local
                .filter(|local| Some(local) != project.local_file.as_ref())
                .map(|local| (ConfigFile::Local, local))
        }
        ConfigFile::Local => {
            let mut project_file = project.project_file.clone();
            drop_removed(&before, &mut project_file, Some(document));
            if project_file == project.project_file {
                None
            } else {
                Some((ConfigFile::Project, project_file))
            }
        }
    }
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
