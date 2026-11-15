//! What `apply` makes of a proposal (RB-010-13): the project's breakpoints and grid with the
//! chosen views, replaced or appended. The plugin writes nothing; the host writes the answer
//! into `.rbrowser/config.json` after a person (or an MCP caller) chose.
//!
//! Appending keeps every existing breakpoint as it is (unknown fields included) and skips a
//! chosen view whose width is already there - its own `width`, or the width the host resolved
//! from its device preset (`existingWidths`, RB-010-12); an id that is taken gets its width appended. A grid
//! with cells gets the new views as a row of their own below it; a grid without cells is left
//! to the app's one-row default. Replacing writes the chosen views and a grid of at most four
//! columns.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// One view of a proposal, as `detect` answered it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedBreakpoint {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub is_new: bool,
}

/// The arguments of `apply`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApplyArgs {
    /// The config's `breakpoints`, as they are.
    pub existing: Vec<Value>,
    /// The widths of `existing` as the host resolved them: a breakpoint that leaves out `width`
    /// takes its device preset's, which only the host knows (the user's presets included).
    pub existing_widths: Vec<u32>,
    /// The config's `layout`, as it is.
    pub layout: Value,
    /// The proposal's views.
    pub proposed: Vec<ProposedBreakpoint>,
    /// The ids of the views to write; all new ones when empty.
    pub selected: Vec<String>,
    /// Replace the breakpoints instead of appending.
    pub replace: bool,
}

/// What `apply` answers: the new `breakpoints` and `layout`, and the ids added.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Applied {
    pub breakpoints: Vec<Value>,
    pub layout: Value,
    pub added: Vec<String>,
}

/// The most columns of a grid `apply` builds.
const MAX_COLUMNS: usize = 4;
/// The gap of a grid built from nothing, as the config's default.
const DEFAULT_GAP: u64 = 8;

/// Applies `args`; `added` is empty when there is nothing to write.
#[must_use]
pub fn apply(args: &ApplyArgs) -> Applied {
    let chosen: Vec<&ProposedBreakpoint> = args
        .proposed
        .iter()
        .filter(|view| {
            if args.selected.is_empty() {
                args.replace || view.is_new
            } else {
                args.selected.contains(&view.id)
            }
        })
        .collect();
    if args.replace {
        let breakpoints: Vec<Value> = chosen
            .iter()
            .map(|view| breakpoint(view, &view.id))
            .collect();
        let ids: Vec<String> = chosen.iter().map(|view| view.id.clone()).collect();
        let gap = args.layout.get("gap").and_then(Value::as_u64);
        return Applied {
            layout: grid(&ids, gap.unwrap_or(DEFAULT_GAP)),
            breakpoints,
            added: ids,
        };
    }
    let mut breakpoints = args.existing.clone();
    let mut added = Vec::new();
    for view in chosen {
        let width_taken = args.existing_widths.contains(&view.width)
            || breakpoints.iter().any(|existing| {
                existing.get("width").and_then(Value::as_u64) == Some(view.width.into())
            });
        if width_taken {
            continue;
        }
        let id = free_id(&breakpoints, view);
        breakpoints.push(breakpoint(view, &id));
        added.push(id);
    }
    let layout = appended_row(&args.layout, &added);
    Applied {
        breakpoints,
        layout,
        added,
    }
}

fn breakpoint(view: &ProposedBreakpoint, id: &str) -> Value {
    json!({ "id": id, "label": view.label, "width": view.width, "height": view.height })
}

fn has_id(breakpoints: &[Value], id: &str) -> bool {
    breakpoints
        .iter()
        .any(|existing| existing.get("id").and_then(Value::as_str) == Some(id))
}

fn free_id(breakpoints: &[Value], view: &ProposedBreakpoint) -> String {
    if !has_id(breakpoints, &view.id) {
        return view.id.clone();
    }
    let mut id = format!("{}-{}", view.id, view.width);
    let mut n = 2;
    while has_id(breakpoints, &id) {
        id = format!("{}-{}-{n}", view.id, view.width);
        n += 1;
    }
    id
}

/// A grid of `ids` in order, row by row, at most [`MAX_COLUMNS`] wide.
fn grid(ids: &[String], gap: u64) -> Value {
    let columns = ids.len().clamp(1, MAX_COLUMNS);
    let rows = ids.len().div_ceil(columns).max(1);
    let cells: Vec<Value> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            json!({ "breakpoint": id, "col": index % columns + 1, "row": index / columns + 1 })
        })
        .collect();
    json!({ "columns": columns, "rows": rows, "gap": gap, "cells": cells })
}

/// `layout` with a row of `added` below its cells; unchanged when it has no cells.
fn appended_row(layout: &Value, added: &[String]) -> Value {
    let Some(object) = layout.as_object() else {
        return layout.clone();
    };
    let cells = object.get("cells").and_then(Value::as_array);
    if added.is_empty() || cells.is_none_or(Vec::is_empty) {
        return layout.clone();
    }
    let rows = object.get("rows").and_then(Value::as_u64).unwrap_or(1);
    let columns = object.get("columns").and_then(Value::as_u64).unwrap_or(1);
    let mut cells = cells.cloned().unwrap_or_default();
    for (index, id) in added.iter().enumerate() {
        cells.push(json!({ "breakpoint": id, "col": index + 1, "row": rows + 1 }));
    }
    let mut changed: Map<String, Value> = object.clone();
    changed.insert("rows".into(), json!(rows + 1));
    changed.insert("columns".into(), json!(columns.max(added.len() as u64)));
    changed.insert("cells".into(), Value::Array(cells));
    Value::Object(changed)
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
