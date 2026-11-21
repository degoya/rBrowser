//! The rules over the breakpoints (rBrowser's own): a heading, landmark or interactive element
//! another view of the same page shows that this view does not - missing or hidden at this width
//! (RB-091-03, RB-094-02). The finding is the view's that lacks it, at the element's path in the
//! first view showing it, with the same words in every view lacking it (`shown`, `missing`), so
//! the host merges those views into one finding with their breakpoints.

use std::collections::BTreeMap;

use rb_plugin_api::{Finding, ViewId};
use serde::Serialize;
use serde_json::Value;

use crate::merge::{self, Cell, Entry, Row};
use crate::page::{Located, PageOutline};

use super::{HEADING_DIFFERS, INTERACTIVE_DIFFERS, LANDMARK_DIFFERS, Sink};

/// One view of the page: its id, its label (for the finding) and its outline.
#[derive(Clone, Copy, Debug)]
pub struct Peer<'a> {
    pub view: ViewId,
    pub label: &'a str,
    pub outline: &'a PageOutline,
}

/// One element some views of a page show and others lack or hide: a row of the table element x
/// breakpoint (`outline_breakpoint_diff`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    /// `interactive`, `headings` or `landmarks` (`outline.diff.elements`).
    pub kind: &'static str,
    /// `H2 Promotion`, `navigation "Main"`, `link "Pricing"`.
    pub label: String,
    /// Where it is in the first view showing it: what the finding sits at.
    pub selector: String,
    pub path: String,
    /// The cells of the views that have it, shown or hidden.
    pub views: BTreeMap<ViewId, Cell>,
    /// The views showing it, in the peers' order.
    pub shown: Vec<ViewId>,
    /// The views lacking it or hiding it, in the peers' order.
    pub missing: Vec<ViewId>,
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn heading_label(item: &Value) -> String {
    format!(
        "H{} {}",
        item["level"].as_i64().unwrap_or(2),
        text(&item["text"])
    )
}

fn landmark_label(item: &Value) -> String {
    let name = text(&item["name"]);
    if name.is_empty() {
        text(&item["role"]).to_owned()
    } else {
        format!("{} \"{name}\"", text(&item["role"]))
    }
}

/// `link "Docs"`: an interactive element's role (or tag) and name.
fn interactive_label(item: &Value) -> String {
    let role = text(&item["role"]);
    let kind = if role.is_empty() {
        text(&item["tag"])
    } else {
        role
    };
    let name = text(&item["name"]);
    if name.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind} \"{name}\"")
    }
}

/// The differences of one section's entries over `peers`.
fn of<T: Entry>(
    peers: &[Peer<'_>],
    kind: &'static str,
    list: fn(&PageOutline) -> &[T],
    label: fn(&Value) -> String,
) -> Vec<Difference> {
    let lists: Vec<(ViewId, &[T])> = peers.iter().map(|p| (p.view, list(p.outline))).collect();
    merge::rows(&lists)
        .into_iter()
        .filter_map(|Row { item, views, .. }| {
            let (shown, missing): (Vec<ViewId>, Vec<ViewId>) = peers
                .iter()
                .map(|p| p.view)
                .partition(|view| views.get(view).is_some_and(|cell| cell.visible));
            let first = views.get(shown.first()?)?;
            if missing.is_empty() {
                return None;
            }
            Some(Difference {
                kind,
                label: label(&item),
                selector: first.selector.clone(),
                path: first.path.clone(),
                views,
                shown,
                missing,
            })
        })
        .collect()
}

/// What `peers` - the views showing one page - do not all show: headings, landmarks and
/// interactive elements (the focus list), in that order. `sections` the sections compared
/// (`headings`, `landmarks`, `focus`; every one when empty).
#[must_use]
pub fn differences(peers: &[Peer<'_>], sections: &[String]) -> Vec<Difference> {
    let has = |name: &str| sections.is_empty() || sections.iter().any(|s| s == name);
    let mut found = Vec::new();
    if peers.len() < 2 {
        return found;
    }
    if has("headings") {
        found.extend(of(peers, "headings", |o| &o.headings, heading_label));
    }
    if has("landmarks") {
        found.extend(of(peers, "landmarks", |o| &o.landmarks, landmark_label));
    }
    if has("focus") {
        found.extend(of(peers, "interactive", |o| &o.focus, interactive_label));
    }
    found
}

/// The rule of a kind of [`Difference`].
fn rule_of(kind: &str) -> &'static str {
    match kind {
        "headings" => HEADING_DIFFERS,
        "landmarks" => LANDMARK_DIFFERS,
        _ => INTERACTIVE_DIFFERS,
    }
}

/// The labels of `views` among `peers`, joined.
fn labels(peers: &[Peer<'_>], views: &[ViewId]) -> String {
    views
        .iter()
        .filter_map(|view| peers.iter().find(|p| p.view == *view).map(|p| p.label))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The findings of `view` against `peers`, the views showing the same page (`view` among them);
/// `sections` the sections compared (every one when empty), `enabled` the rules the run asks for.
#[must_use]
pub fn across(
    enabled: &[String],
    view: ViewId,
    peers: &[Peer<'_>],
    sections: &[String],
) -> Vec<Finding> {
    let mut sink = Sink::new(enabled);
    let Some(own) = peers.iter().find(|p| p.view == view) else {
        return Vec::new();
    };
    let url = own.outline.url.as_str();
    for difference in differences(peers, sections) {
        let rule = rule_of(difference.kind);
        if !sink.wants(rule) || !difference.missing.contains(&view) {
            continue;
        }
        let at = Located {
            selector: difference.selector.clone(),
            path: difference.path.clone(),
            ..Located::default()
        };
        sink.emit(
            rule,
            (url, view),
            Some(&at),
            &[
                ("text", difference.label.clone()),
                ("shown", labels(peers, &difference.shown)),
                ("view", own.label.to_owned()),
                ("missing", labels(peers, &difference.missing)),
            ],
        );
    }
    sink.findings
}

#[cfg(test)]
#[path = "across_tests.rs"]
mod tests;
