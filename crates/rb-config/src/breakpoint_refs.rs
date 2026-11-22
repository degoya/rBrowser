//! Where a project file names a breakpoint by its id, and removing those references when the
//! breakpoint goes (RB-090-61): the cells of `layout` and of every preset's `layout`, the hidden
//! selections (`layout.hidden`, a preset's `hidden`), the fields of the split and canvas modes
//! (`layout.modes.*`, a preset's `primary`, `secondary`, `items`), a preset's `focus` and
//! `envCompare.breakpoints`. Environments name no breakpoint.
//!
//! The functions work on a config file's JSON as it is on disk, so the settings window
//! (`setSetting`) and MCP's `breakpoint_remove` edit the very files they write; both call
//! [`drop_removed`]. The UI's confirmation counts the same places (`ui/src/settings/breakpointRefs.ts`).
//!
//! The merge joins the two files' `breakpoints` by id, so a breakpoint is gone only when neither
//! file defines it: [`remove_definitions`] takes its entry out of both (RB-094-25).

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

/// The modes of `layout.modes` that name breakpoints.
const MODES: [&str; 3] = ["horizontal", "split", "canvas"];

/// How many references to a breakpoint were removed, by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct References {
    /// Cells of `layout.cells` and of the presets' layouts.
    pub cells: usize,
    /// Entries of `layout.hidden` and of the presets' `hidden`.
    pub hidden: usize,
    /// The rest: split sides, canvas items, a preset's `focus`, `envCompare.breakpoints`.
    pub other: usize,
}

impl References {
    /// All references together.
    #[must_use]
    pub const fn total(self) -> usize {
        self.cells + self.hidden + self.other
    }

    fn add(&mut self, more: Self) {
        self.cells += more.cells;
        self.hidden += more.hidden;
        self.other += more.other;
    }
}

/// The ids the `breakpoints` of `project` and `local` define together.
#[must_use]
pub fn defined_ids(project: &Value, local: Option<&Value>) -> BTreeSet<String> {
    [Some(project), local]
        .into_iter()
        .flatten()
        .filter_map(|document| document.get("breakpoints").and_then(Value::as_array))
        .flatten()
        .filter_map(|breakpoint| breakpoint.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// Removes the entries of the breakpoints `ids` from the `breakpoints` of `project` and `local`,
/// wherever either defines them (RB-094-25): an entry left in one file brings the breakpoint
/// back through the merge. Returns how many entries went.
pub fn remove_definitions(
    ids: &BTreeSet<String>,
    project: &mut Value,
    local: Option<&mut Value>,
) -> usize {
    [Some(project), local]
        .into_iter()
        .flatten()
        .map(|document| {
            retain(document.get_mut("breakpoints"), |entry| {
                entry
                    .get("id")
                    .and_then(Value::as_str)
                    .is_none_or(|id| !ids.contains(id))
            })
        })
        .sum()
}

/// Removes from `project` and `local` every reference to an id of `before` (the ids the two
/// files defined before an edit, [`defined_ids`]) that the edited files no longer define. An id
/// still defined in either file keeps its references.
pub fn drop_removed(
    before: &BTreeSet<String>,
    project: &mut Value,
    mut local: Option<&mut Value>,
) -> References {
    let after = defined_ids(project, local.as_deref());
    let mut found = References::default();
    for id in before.difference(&after) {
        found.add(remove_references(project, id));
        if let Some(local) = local.as_deref_mut() {
            found.add(remove_references(local, id));
        }
    }
    found
}

/// Removes every reference to the breakpoint `id` from one config file's JSON and counts them.
/// A cell, a canvas item or a list entry goes; a preset's `focus` is removed (the first
/// breakpoint then); the breakpoint itself stays for the caller.
pub fn remove_references(document: &mut Value, id: &str) -> References {
    let mut found = References::default();
    if let Some(layout) = document.get_mut("layout") {
        strip_layout(layout, id, &mut found);
    }
    if let Some(presets) = document
        .pointer_mut("/layouts/presets")
        .and_then(Value::as_object_mut)
    {
        for preset in presets.values_mut() {
            if let Some(layout) = preset.get_mut("layout") {
                strip_layout(layout, id, &mut found);
            }
            found.hidden += strip_ids(preset.get_mut("hidden"), id);
            strip_mode_fields(preset, id, &mut found);
            if let Some(object) = preset.as_object_mut()
                && object.get("focus").and_then(Value::as_str) == Some(id)
            {
                object.remove("focus");
                found.other += 1;
            }
        }
    }
    found.other += strip_ids(document.pointer_mut("/envCompare/breakpoints"), id);
    found
}

fn strip_layout(layout: &mut Value, id: &str, found: &mut References) {
    found.cells += strip_items(layout.get_mut("cells"), id);
    found.hidden += strip_ids(layout.get_mut("hidden"), id);
    if let Some(modes) = layout.get_mut("modes").and_then(Value::as_object_mut) {
        for mode in MODES {
            if let Some(fields) = modes.get_mut(mode) {
                strip_mode_fields(fields, id, found);
            }
        }
    }
}

fn strip_mode_fields(fields: &mut Value, id: &str, found: &mut References) {
    found.other += strip_ids(fields.get_mut("primary"), id)
        + strip_ids(fields.get_mut("secondary"), id)
        + strip_items(fields.get_mut("items"), id);
}

/// Removes the entries equal to `id` from a list of ids; how many went.
fn strip_ids(list: Option<&mut Value>, id: &str) -> usize {
    retain(list, |entry| entry.as_str() != Some(id))
}

/// Removes the objects whose `breakpoint` is `id` (cells, canvas items); how many went.
fn strip_items(list: Option<&mut Value>, id: &str) -> usize {
    retain(list, |entry| {
        entry.get("breakpoint").and_then(Value::as_str) != Some(id)
    })
}

fn retain(list: Option<&mut Value>, keep: impl Fn(&Value) -> bool) -> usize {
    let Some(entries) = list.and_then(Value::as_array_mut) else {
        return 0;
    };
    let before = entries.len();
    entries.retain(|entry| keep(entry));
    before - entries.len()
}

#[cfg(test)]
#[path = "breakpoint_refs_tests.rs"]
mod tests;
