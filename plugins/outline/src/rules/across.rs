//! The rules over the breakpoints (rBrowser's own): a heading or landmark another view of the
//! same page shows that this view does not - missing or hidden at this width. The finding is the
//! view's that lacks it, at the element's path in the first view showing it, so the host merges
//! the views that lack the same element into one finding.

use rb_plugin_api::{Finding, ViewId};
use serde_json::Value;

use crate::merge::{self, Entry, Row};
use crate::page::{Located, PageOutline};

use super::{HEADING_DIFFERS, LANDMARK_DIFFERS, Sink};

/// One view of the page: its id, its label (for the finding) and its outline.
#[derive(Clone, Copy, Debug)]
pub struct Peer<'a> {
    pub view: ViewId,
    pub label: &'a str,
    pub outline: &'a PageOutline,
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

fn differences<T: Entry>(
    sink: &mut Sink<'_>,
    rule: &str,
    (view, own): (ViewId, &str),
    url: &str,
    peers: &[Peer<'_>],
    list: fn(&PageOutline) -> &[T],
    label: fn(&Value) -> String,
) {
    let lists: Vec<(ViewId, &[T])> = peers.iter().map(|p| (p.view, list(p.outline))).collect();
    for Row { item, views, .. } in merge::rows(&lists) {
        let shown: Vec<&Peer<'_>> = peers
            .iter()
            .filter(|p| views.get(&p.view).is_some_and(|cell| cell.visible))
            .collect();
        let Some(first) = shown.first() else {
            continue;
        };
        if shown.iter().any(|p| p.view == view) {
            continue;
        }
        let Some(cell) = views.get(&first.view) else {
            continue;
        };
        let at = Located {
            selector: cell.selector.clone(),
            path: cell.path.clone(),
            ..Located::default()
        };
        let names: Vec<&str> = shown.iter().map(|p| p.label).collect();
        sink.emit(
            rule,
            (url, view),
            Some(&at),
            &[
                ("text", label(&item)),
                ("shown", names.join(", ")),
                ("view", own.to_owned()),
            ],
        );
    }
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
    let has = |name: &str| sections.is_empty() || sections.iter().any(|s| s == name);
    let mut sink = Sink::new(enabled);
    let Some(own) = peers.iter().find(|p| p.view == view) else {
        return Vec::new();
    };
    if peers.len() < 2 {
        return Vec::new();
    }
    let url = own.outline.url.as_str();
    if has("headings") && sink.wants(HEADING_DIFFERS) {
        differences(
            &mut sink,
            HEADING_DIFFERS,
            (view, own.label),
            url,
            peers,
            |o| &o.headings,
            heading_label,
        );
    }
    if has("landmarks") && sink.wants(LANDMARK_DIFFERS) {
        differences(
            &mut sink,
            LANDMARK_DIFFERS,
            (view, own.label),
            url,
            peers,
            |o| &o.landmarks,
            landmark_label,
        );
    }
    sink.findings
}

#[cfg(test)]
#[path = "across_tests.rs"]
mod tests;
