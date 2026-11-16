//! The tab orders of several views compared (RB-070-12): the elements only some views reach, and
//! the elements the views reach at other places. An element is the same in every view when its
//! full path is (`web/src/overlays/tab-order/order.ts` `composedPath`).
//!
//! "At another place" is relative, not by number - one element more in a view moves every number
//! after it: of the elements every view reaches, the first view's order is the reference, and an
//! element is moved when it is not in the longest common subsequence of a view's order and the
//! reference.

use std::collections::HashSet;

use rb_plugin_api::ViewId;
use serde_json::{Value, json};

use crate::budget;

/// The most entries each list of differences carries; the counts are the whole numbers.
pub const MAX_DIFFS: usize = 25;
/// The most bytes each list of differences takes as JSON text (RB-090-17).
pub const MAX_DIFF_BYTES: usize = 6 * 1024;

/// One stop of a view's tab order, as the comparison needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub path: String,
    pub selector: String,
    /// Its place in the view's order, from 1.
    pub index: u32,
}

/// `stops` without an empty path or a path seen before.
fn distinct(stops: &[Stop]) -> Vec<&Stop> {
    let mut seen = HashSet::new();
    stops
        .iter()
        .filter(|stop| !stop.path.is_empty() && seen.insert(stop.path.as_str()))
        .collect()
}

/// The paths of the longest common subsequence of `one` and `two`.
fn common<'a>(one: &[&'a str], two: &[&str]) -> HashSet<&'a str> {
    let width = two.len() + 1;
    let mut table = vec![0_u32; (one.len() + 1) * width];
    for i in (0..one.len()).rev() {
        for j in (0..two.len()).rev() {
            table[i * width + j] = if one[i] == two[j] {
                table[(i + 1) * width + j + 1] + 1
            } else {
                table[(i + 1) * width + j].max(table[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut kept = HashSet::new();
    while i < one.len() && j < two.len() {
        if one[i] == two[j] {
            kept.insert(one[i]);
            i += 1;
            j += 1;
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    kept
}

/// The paths of `order` every view reaches, in its order.
fn shared<'a>(order: &[&'a Stop], everywhere: &HashSet<&str>) -> Vec<&'a str> {
    order
        .iter()
        .map(|stop| stop.path.as_str())
        .filter(|path| everywhere.contains(path))
        .collect()
}

/// The differences between the tab orders of `views`: `{ views, onlyIn: [{ selector, path,
/// views, missing }], onlyInCount, moved: [{ selector, path, positions: [{ view, index }] }],
/// movedCount }`, each list at most [`MAX_DIFFS`] long and [`MAX_DIFF_BYTES`] big.
#[must_use]
pub fn compare(views: &[(ViewId, &[Stop])]) -> Value {
    let ids: Vec<ViewId> = views.iter().map(|(view, _)| *view).collect();
    let orders: Vec<(ViewId, Vec<&Stop>)> = views
        .iter()
        .map(|(view, stops)| (*view, distinct(stops)))
        .collect();
    let mut only_in = Vec::new();
    let mut everywhere: HashSet<&str> = HashSet::new();
    let mut listed: HashSet<&str> = HashSet::new();
    for (_, order) in &orders {
        for stop in order {
            if !listed.insert(stop.path.as_str()) {
                continue;
            }
            let mut present: Vec<ViewId> = Vec::new();
            let mut missing: Vec<ViewId> = Vec::new();
            for (view, order) in &orders {
                if order.iter().any(|other| other.path == stop.path) {
                    present.push(*view);
                } else {
                    missing.push(*view);
                }
            }
            if missing.is_empty() {
                everywhere.insert(stop.path.as_str());
            } else {
                only_in.push(json!({
                    "selector": stop.selector, "path": stop.path,
                    "views": present, "missing": missing,
                }));
            }
        }
    }
    let mut moved_paths: HashSet<&str> = HashSet::new();
    if let Some((_, first)) = orders.first() {
        let reference = shared(first, &everywhere);
        for (_, order) in orders.iter().skip(1) {
            let kept = common(&reference, &shared(order, &everywhere));
            moved_paths.extend(reference.iter().filter(|path| !kept.contains(*path)));
        }
    }
    let moved: Vec<Value> = orders
        .first()
        .map(|(_, first)| first.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|stop| moved_paths.contains(stop.path.as_str()))
        .map(|stop| {
            let positions: Vec<Value> = orders
                .iter()
                .filter_map(|(view, order)| {
                    let found = order.iter().find(|other| other.path == stop.path)?;
                    Some(json!({ "view": view, "index": found.index }))
                })
                .collect();
            json!({ "selector": stop.selector, "path": stop.path, "positions": positions })
        })
        .collect();
    json!({
        "views": ids,
        "onlyInCount": only_in.len(),
        "onlyIn": fitted(only_in),
        "movedCount": moved.len(),
        "moved": fitted(moved),
    })
}

/// The first entries of `list` within [`MAX_DIFFS`] and [`MAX_DIFF_BYTES`].
fn fitted(mut list: Vec<Value>) -> Vec<Value> {
    let keep = budget::share(&[list.as_slice()], MAX_DIFFS, MAX_DIFF_BYTES);
    list.truncate(keep.first().copied().unwrap_or(0));
    list
}

#[cfg(test)]
#[path = "tab_compare_tests.rs"]
mod tests;
