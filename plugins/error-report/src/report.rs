//! The report: every view's errors ([`crate::classify`]) deduplicated over the views, each entry
//! with the views that reported it, and "only in views ..." when not every view did.
//!
//! What a view reported before its page last loaded is left out (`sinceNavigation`: the newest
//! document request of the view's current URL marks the load), as is everything at or before the
//! clear mark, another site's errors without `thirdParty`, the rules switched off in `rules` and
//! the entries an `ignore` glob matches (URL, source or text). Only Chromium views capture, so
//! "only in" is relative to them.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{Severity, ViewId, ViewInfo};
use serde_json::{Map, Value, json};

use crate::classify::{self, Category, Event, RequestInfo, Source};
use crate::config::{RuleSetting, Settings};
use crate::feed::{Feed, ViewFeed};
use crate::normalize::{glob, id_of, same_site};

/// One error, deduplicated over the views.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub category: Category,
    pub severity: Severity,
    pub text: String,
    pub url: Option<String>,
    pub source: Option<Source>,
    pub stack: Option<String>,
    pub request: Option<RequestInfo>,
    pub detail: Option<String>,
    /// How often each view reported it.
    pub per_view: BTreeMap<ViewId, u32>,
    /// The names of its views when not every capturing view reported it.
    pub only_in: Option<Vec<String>>,
    pub first: f64,
    pub last: f64,
}

impl Entry {
    /// The views that reported it, in id order.
    #[must_use]
    pub fn views(&self) -> Vec<ViewId> {
        self.per_view.keys().copied().collect()
    }

    /// How often it was reported, over every view.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.per_view.values().sum()
    }
}

/// The report over the views.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// The host handed a capture.
    pub captured: bool,
    /// Every view the host knows, in grid order.
    pub views: Vec<ViewInfo>,
    /// The views that capture (Chromium), in id order.
    pub capturing: Vec<ViewId>,
    /// In category order, then by first sight.
    pub entries: Vec<Entry>,
}

/// The name of a view in "only in": its breakpoint, else its label, else its id.
#[must_use]
pub fn view_name(views: &[ViewInfo], id: ViewId) -> String {
    views
        .iter()
        .find(|view| view.id == id)
        .map(|view| {
            if view.breakpoint.is_empty() {
                view.label.clone()
            } else {
                view.breakpoint.clone()
            }
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| id.to_string())
}

fn without_fragment(url: &str) -> &str {
    url.split('#').next().unwrap_or_default()
}

/// When the view's current page last loaded: its newest document request of `page`.
#[must_use]
pub fn navigated_at(feed: &ViewFeed, page: &str) -> Option<f64> {
    let page = without_fragment(page);
    feed.network
        .iter()
        .filter(|request| {
            request
                .resource_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("document"))
                && without_fragment(&request.url) == page
        })
        .map(|request| request.time)
        .reduce(f64::max)
}

fn kept(event: &Event, page: &str, settings: &Settings) -> bool {
    if settings.rules.get(event.category.rule()) == Some(&RuleSetting::Off) {
        return false;
    }
    if settings.third_party || page.is_empty() {
        return true;
    }
    let origin = event
        .url
        .as_deref()
        .or(event.source.as_ref().map(|s| s.url.as_str()));
    origin.is_none_or(|url| same_site(url, page))
}

fn ignored(entry: &Entry, patterns: &[String]) -> bool {
    let fields = [
        entry.url.as_deref(),
        entry.source.as_ref().map(|s| s.url.as_str()),
        Some(entry.text.as_str()),
    ];
    patterns
        .iter()
        .any(|pattern| fields.iter().flatten().any(|field| glob(pattern, field)))
}

fn severity(category: Category, settings: &Settings) -> Severity {
    match settings.rules.get(category.rule()) {
        Some(RuleSetting::Severity(severity)) => *severity,
        _ => category.severity(),
    }
}

/// The report of `feed` over `views` with `settings`; entries at or before `cleared_at` left out.
#[must_use]
pub fn build(feed: &Feed, views: &[ViewInfo], settings: &Settings, cleared_at: f64) -> Report {
    let info = |id: ViewId| views.iter().find(|view| view.id == id);
    let capturing: BTreeSet<ViewId> = feed
        .views
        .iter()
        .map(|view| view.view)
        .filter(|id| info(*id).is_none_or(|view| view.engine == "chromium"))
        .collect();
    let mut groups: BTreeMap<String, Entry> = BTreeMap::new();
    for view in &feed.views {
        let page = info(view.view).map(|v| v.url.clone()).unwrap_or_default();
        let since = if settings.since_navigation {
            navigated_at(view, &page).unwrap_or(f64::NEG_INFINITY)
        } else {
            f64::NEG_INFINITY
        };
        for event in classify::events(view) {
            if event.time <= cleared_at || event.time < since || !kept(&event, &page, settings) {
                continue;
            }
            add(&mut groups, event, settings);
        }
    }
    let mut entries: Vec<Entry> = groups
        .into_values()
        .filter(|entry| !ignored(entry, &settings.ignore))
        .collect();
    for entry in &mut entries {
        let partial = capturing.len() > 1
            && entry.per_view.len() < capturing.len()
            && entry.per_view.keys().all(|id| capturing.contains(id));
        if partial {
            entry.only_in = Some(
                entry
                    .per_view
                    .keys()
                    .map(|id| view_name(views, *id))
                    .collect(),
            );
        }
    }
    entries.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(a.first.total_cmp(&b.first))
    });
    Report {
        captured: feed.captured,
        views: views.to_vec(),
        capturing: capturing.into_iter().collect(),
        entries,
    }
}

fn add(groups: &mut BTreeMap<String, Entry>, event: Event, settings: &Settings) {
    let entry = groups.entry(event.key.clone()).or_insert_with(|| Entry {
        id: id_of(&event.key),
        category: event.category,
        severity: severity(event.category, settings),
        text: event.text.clone(),
        url: event.url.clone(),
        source: event.source.clone(),
        stack: event.stack.clone(),
        request: event.request.clone(),
        detail: event.detail.clone(),
        per_view: BTreeMap::new(),
        only_in: None,
        first: event.time,
        last: event.time,
    });
    *entry.per_view.entry(event.view).or_insert(0) += 1;
    entry.first = entry.first.min(event.time);
    entry.last = entry.last.max(event.time);
}

fn source_json(source: &Source) -> Value {
    json!({ "url": source.url, "line": source.line, "column": source.column })
}

fn request_json(request: &RequestInfo) -> Value {
    json!({ "method": request.method, "status": request.status,
            "resourceType": request.resource_type, "mimeType": request.mime_type })
}

/// One entry as answers and the JSON report hold it.
#[must_use]
pub fn entry_json(entry: &Entry) -> Value {
    let mut out = Map::new();
    out.insert("id".into(), json!(entry.id));
    out.insert("rule".into(), json!(entry.category.rule()));
    out.insert("severity".into(), json!(entry.severity.as_str()));
    out.insert("text".into(), json!(entry.text));
    let optional = [
        ("url", entry.url.as_ref().map(|u| json!(u))),
        ("source", entry.source.as_ref().map(source_json)),
        ("stack", entry.stack.as_ref().map(|s| json!(s))),
        ("request", entry.request.as_ref().map(request_json)),
        ("detail", entry.detail.as_ref().map(|d| json!(d))),
        ("onlyIn", entry.only_in.as_ref().map(|names| json!(names))),
    ];
    for (name, value) in optional {
        if let Some(value) = value {
            out.insert(name.into(), value);
        }
    }
    let per_view: Map<String, Value> = entry
        .per_view
        .iter()
        .map(|(id, count)| (id.to_string(), json!(count)))
        .collect();
    out.insert("views".into(), json!(entry.views()));
    out.insert("perView".into(), Value::Object(per_view));
    out.insert("count".into(), json!(entry.count()));
    out.insert("first".into(), json!(entry.first));
    out.insert("last".into(), json!(entry.last));
    Value::Object(out)
}

/// The entries of `report` that `view` reported (all without one), at most `limit` of them.
#[must_use]
pub fn to_json(report: &Report, view: Option<ViewId>, limit: usize) -> Value {
    let shown: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|entry| view.is_none_or(|view| entry.per_view.contains_key(&view)))
        .collect();
    let mut summary = Map::new();
    for category in Category::ALL {
        let count = shown.iter().filter(|e| e.category == category).count();
        summary.insert(category.rule().into(), json!(count));
    }
    let mut severities = Map::new();
    for severity in Severity::ALL {
        let count = shown.iter().filter(|e| e.severity == severity).count();
        severities.insert(severity.as_str().into(), json!(count));
    }
    let views: Vec<Value> = report
        .views
        .iter()
        .map(|v| {
            json!({ "id": v.id, "label": v.label, "breakpoint": v.breakpoint,
                    "engine": v.engine, "width": v.width, "url": v.url,
                    "captures": report.capturing.contains(&v.id) })
        })
        .collect();
    json!({
        "captured": report.captured,
        "total": shown.len(),
        "truncated": shown.len() > limit,
        "summary": summary,
        "severities": severities,
        "views": views,
        "entries": shown.iter().take(limit).map(|e| entry_json(e)).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
