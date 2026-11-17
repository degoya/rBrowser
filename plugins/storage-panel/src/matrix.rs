//! The comparison over the views: one row per cookie (domain, path, name, partition) or storage
//! key (origin, key), one cell per view the entry applies to - a cookie to the views whose URL
//! it is sent with, a storage key to the views showing its origin. A cell is the view's entry or
//! missing; a row is `same`, `differs` (two views hold different values) or `missing` (a view it
//! applies to lacks it). Views that were not read (another engine, a failure) have no cells.

use std::collections::BTreeMap;

use rb_plugin_api::ViewId;
use serde_json::{Map, Value, json};

use crate::cookie::Cookie;
use crate::origin;
use crate::read::{Kind, ViewData};

/// One view's entry of a row.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub value: String,
    /// The cookie, for the cookie rows.
    pub cookie: Option<Cookie>,
}

impl Cell {
    fn same_as(&self, other: &Self) -> bool {
        match (&self.cookie, &other.cookie) {
            (Some(a), Some(b)) => a.same_as(b),
            _ => self.value == other.value,
        }
    }
}

/// How a row compares over its views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Same,
    Differs,
    Missing,
}

impl State {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Differs => "differs",
            Self::Missing => "missing",
        }
    }
}

/// One cookie or storage key over the views.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub kind: Kind,
    /// The origin of a storage key, the domain of a cookie (as CDP reports it).
    pub scope: String,
    /// A cookie's path; empty for storage.
    pub path: String,
    pub name: String,
    /// A partitioned cookie's top-level site; empty otherwise.
    pub partition: String,
    /// The views the entry applies to: their entry, or `None` where it is missing.
    pub cells: BTreeMap<ViewId, Option<Cell>>,
}

/// FNV-1a, 64 bit, over `parts`: the stable ids of rows and pins.
pub fn fnv(parts: &[&str]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

impl Row {
    /// A stable id of the row (kind, scope, path, name, partition).
    #[must_use]
    pub fn id(&self) -> String {
        fnv(&[
            self.kind.name(),
            &self.scope,
            &self.path,
            &self.name,
            &self.partition,
        ])
    }

    /// How the row compares.
    #[must_use]
    pub fn state(&self) -> State {
        let present: Vec<&Cell> = self.cells.values().flatten().collect();
        if present.windows(2).any(|pair| !pair[0].same_as(pair[1])) {
            State::Differs
        } else if present.len() < self.cells.len() {
            State::Missing
        } else {
            State::Same
        }
    }

    /// The views grouped by equal entries (the missing ones last, as one group), so a reader
    /// sees which views agree without seeing a value.
    #[must_use]
    pub fn groups(&self) -> Vec<Vec<ViewId>> {
        let mut groups: Vec<(Cell, Vec<ViewId>)> = Vec::new();
        let mut missing = Vec::new();
        for (view, cell) in &self.cells {
            match cell {
                None => missing.push(*view),
                Some(cell) => match groups.iter_mut().find(|(seen, _)| seen.same_as(cell)) {
                    Some((_, views)) => views.push(*view),
                    None => groups.push((cell.clone(), vec![*view])),
                },
            }
        }
        let mut out: Vec<Vec<ViewId>> = groups.into_iter().map(|(_, views)| views).collect();
        if !missing.is_empty() {
            out.push(missing);
        }
        out
    }
}

fn applies(kind: Kind, data: &ViewData, scope: &str, path: &str) -> bool {
    if data.status != "ok" {
        return false;
    }
    match kind {
        Kind::Cookies => {
            let host = origin::host_of(&data.column.url).unwrap_or_default();
            origin::domain_matches(&host, scope)
                && origin::path_matches(&origin::path_of(&data.column.url), path)
        }
        Kind::Local | Kind::Session => data.origin.as_deref() == Some(scope),
    }
}

/// The rows of `kind` over `data`, sorted by scope, path and name.
#[must_use]
pub fn rows(kind: Kind, data: &[ViewData]) -> Vec<Row> {
    // (scope, path, name, partition) -> view -> cell
    let mut found: BTreeMap<(String, String, String, String), BTreeMap<ViewId, Cell>> =
        BTreeMap::new();
    for view in data.iter().filter(|view| view.status == "ok") {
        match kind {
            Kind::Cookies => {
                for cookie in &view.cookies {
                    found.entry(cookie.identity()).or_default().insert(
                        view.column.id,
                        Cell {
                            value: cookie.value.clone(),
                            cookie: Some(cookie.clone()),
                        },
                    );
                }
            }
            Kind::Local | Kind::Session => {
                let Some(scope) = &view.origin else { continue };
                for (key, value) in view.storage(kind) {
                    let at = (scope.clone(), String::new(), key.clone(), String::new());
                    found.entry(at).or_default().insert(
                        view.column.id,
                        Cell {
                            value: value.clone(),
                            cookie: None,
                        },
                    );
                }
            }
        }
    }
    found
        .into_iter()
        .map(|((scope, path, name, partition), mut present)| {
            let cells = data
                .iter()
                .filter_map(|view| {
                    let id = view.column.id;
                    match present.remove(&id) {
                        Some(cell) => Some((id, Some(cell))),
                        None if applies(kind, view, &scope, &path) => Some((id, None)),
                        None => None,
                    }
                })
                .collect();
            Row {
                kind,
                scope,
                path,
                name,
                partition,
                cells,
            }
        })
        .collect()
}

/// How values appear in an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Values {
    /// Each value, cut to this many characters (`truncated` marks a cut one).
    Shown(usize),
    /// No value at all, only its length.
    Hidden,
}

/// A value as an answer carries it: `value` (cut) or nothing, `length` in characters.
#[must_use]
pub fn value_json(value: &str, values: Values) -> Map<String, Value> {
    let length = value.chars().count();
    let mut out = Map::new();
    out.insert("length".to_owned(), json!(length));
    if let Values::Shown(limit) = values {
        out.insert(
            "value".to_owned(),
            json!(value.chars().take(limit).collect::<String>()),
        );
        if length > limit {
            out.insert("truncated".to_owned(), json!(true));
        }
    }
    out
}

/// A row as the panel shows it.
#[must_use]
pub fn row_json(row: &Row, values: Values, pinned: bool) -> Value {
    let cells: Map<String, Value> = row
        .cells
        .iter()
        .map(|(view, cell)| {
            let cell = cell.as_ref().map_or(Value::Null, |cell| {
                let mut out = value_json(&cell.value, values);
                if let Some(cookie) = &cell.cookie
                    && let Value::Object(attributes) = cookie.attributes()
                {
                    out.extend(attributes);
                }
                Value::Object(out)
            });
            (view.to_string(), cell)
        })
        .collect();
    let mut out = json!({
        "id": row.id(),
        "scope": row.scope,
        "name": row.name,
        "state": row.state().name(),
        "pinned": pinned,
        "cells": cells,
    });
    if row.kind == Kind::Cookies {
        out["path"] = json!(row.path);
        if !row.partition.is_empty() {
            out["partition"] = json!(row.partition);
        }
    }
    out
}

/// A row's place in the MCP answer's `differences`: who agrees, who lacks it - no value.
#[must_use]
pub fn difference_json(row: &Row) -> Value {
    let mut out = json!({
        "kind": row.kind.name(),
        "scope": row.scope,
        "name": row.name,
        "state": row.state().name(),
        "groups": row.groups(),
        "missing": row.cells.iter().filter(|(_, c)| c.is_none()).map(|(v, _)| *v).collect::<Vec<_>>(),
    });
    if row.kind == Kind::Cookies {
        out["path"] = json!(row.path);
    }
    out
}

#[cfg(test)]
#[path = "matrix_tests.rs"]
mod tests;
