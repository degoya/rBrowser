//! The views' outlines side by side: one row per entry - the same heading, link or field in
//! every view that has it - with a cell per view (visible, hidden, its selector for the mark) and
//! the views it shows in, so the panel can mark "only on desktop".
//!
//! An entry is the same across views by its key (a heading's level and text, a link's URL and
//! text, ...) and its occurrence of that key in the view, so two "Docs" links stay two rows. A row
//! a later view adds is placed after the row of the entry before it in that view.

use std::collections::{BTreeMap, HashMap};

use rb_plugin_api::ViewId;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::page::{Located, PageOutline};

/// One view's side of a row.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    /// Its place in the view's list (the focus order, for `focus`).
    pub index: usize,
    pub visible: bool,
    pub hidden: bool,
    pub selector: String,
    pub path: String,
}

/// One entry across the views.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// The entry's own fields, from the first view that has it.
    pub item: Value,
    /// The cells by view id.
    pub views: BTreeMap<ViewId, Cell>,
    /// The views it is visible in, when that is not every compared view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<ViewId>>,
}

/// An entry of a section: its key across views, where it is, and its own fields.
pub trait Entry {
    fn key(&self) -> String;
    fn at(&self) -> &Located;
    fn item(&self) -> Value;
}

/// The entry's fields as JSON without the per-view ones.
fn fields<T: Serialize>(entry: &T) -> Value {
    let mut value = serde_json::to_value(entry).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        for name in ["selector", "path", "hidden", "visible", "box"] {
            map.remove(name);
        }
    }
    value
}

macro_rules! entry {
    ($type:ty, |$this:ident| $key:expr) => {
        impl Entry for $type {
            fn key(&self) -> String {
                let $this = self;
                $key
            }
            fn at(&self) -> &Located {
                &self.at
            }
            fn item(&self) -> Value {
                fields(self)
            }
        }
    };
}

entry!(crate::page::Heading, |h| format!(
    "{}|{}",
    h.level,
    h.text.to_lowercase()
));
entry!(crate::page::Landmark, |l| format!(
    "{}|{}",
    l.role,
    l.name.to_lowercase()
));
entry!(crate::page::Link, |l| format!(
    "{}|{}",
    l.href,
    l.text.to_lowercase()
));
entry!(crate::page::Image, |i| i.src.clone());
entry!(crate::page::Field, |f| format!(
    "{}|{}|{}",
    f.kind, f.name, f.label
));
entry!(crate::page::Focusable, |f| format!(
    "{}|{}|{}",
    f.tag, f.role, f.name
));
entry!(crate::page::CustomHit, |h| h.text.to_lowercase());

/// The rows of one section over `views` (each view's id and its list, in grid order).
#[must_use]
pub fn rows<T: Entry>(views: &[(ViewId, &[T])]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut place: HashMap<String, usize> = HashMap::new();
    for (view, list) in views {
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut previous: Option<usize> = None;
        for (index, entry) in list.iter().enumerate() {
            let base = entry.key();
            let occurrence = seen.entry(base.clone()).or_insert(0);
            let key = format!("{base}#{occurrence}");
            *occurrence += 1;
            let at = entry.at();
            let cell = Cell {
                index,
                visible: at.visible,
                hidden: at.hidden,
                selector: at.selector.clone(),
                path: at.path.clone(),
            };
            let row = if let Some(&row) = place.get(&key) {
                row
            } else {
                let row = previous.map_or(rows.len(), |before| before + 1);
                for slot in place.values_mut() {
                    if *slot >= row {
                        *slot += 1;
                    }
                }
                rows.insert(
                    row,
                    Row {
                        item: entry.item(),
                        views: BTreeMap::new(),
                        only: None,
                    },
                );
                place.insert(key, row);
                row
            };
            rows[row].views.insert(*view, cell);
            previous = Some(row);
        }
    }
    let all: Vec<ViewId> = views.iter().map(|(view, _)| *view).collect();
    for row in &mut rows {
        let shown: Vec<ViewId> = all
            .iter()
            .copied()
            .filter(|view| row.views.get(view).is_some_and(|cell| cell.visible))
            .collect();
        if shown.len() != all.len() {
            row.only = Some(shown);
        }
    }
    rows
}

/// The rows of section `name` over the outlines of `views`; `null` for an unknown section.
#[must_use]
pub fn section(name: &str, views: &[(ViewId, &PageOutline)]) -> Value {
    fn of<T: Entry>(views: &[(ViewId, &PageOutline)], list: fn(&PageOutline) -> &[T]) -> Value {
        let lists: Vec<(ViewId, &[T])> = views.iter().map(|(id, o)| (*id, list(o))).collect();
        json!({ "rows": rows(&lists) })
    }
    let mut value = match name {
        "headings" => of(views, |o| &o.headings),
        "landmarks" => of(views, |o| &o.landmarks),
        "links" => of(views, |o| &o.links),
        "images" => of(views, |o| &o.images),
        "forms" => of(views, |o| &o.fields),
        "focus" => of(views, |o| &o.focus),
        "custom" => custom(views),
        _ => return Value::Null,
    };
    let list_name = if name == "forms" { "fields" } else { name };
    let truncated: Vec<ViewId> = views
        .iter()
        .filter(|(_, outline)| outline.truncated.iter().any(|t| t == list_name))
        .map(|(id, _)| *id)
        .collect();
    if !truncated.is_empty() {
        value["truncatedIn"] = json!(truncated);
    }
    value
}

/// The custom selectors: per selector the count in each view and its hits as rows.
fn custom(views: &[(ViewId, &PageOutline)]) -> Value {
    let Some((_, first)) = views.first() else {
        return json!({ "selectors": [] });
    };
    let selectors: Vec<Value> = first
        .custom
        .iter()
        .enumerate()
        .map(|(at, result)| {
            let mut counts = Map::new();
            let mut invalid = false;
            let mut lists = Vec::new();
            for (view, outline) in views {
                if let Some(own) = outline.custom.get(at) {
                    counts.insert(view.to_string(), json!(own.count));
                    invalid |= own.invalid;
                    lists.push((*view, own.hits.as_slice()));
                }
            }
            json!({
                "label": result.label,
                "selector": result.selector,
                "invalid": invalid,
                "counts": counts,
                "rows": rows(&lists),
            })
        })
        .collect();
    json!({ "selectors": selectors })
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
