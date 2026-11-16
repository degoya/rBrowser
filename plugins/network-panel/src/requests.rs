//! The request list over every view (tool `list`, the panel): the host's capture (`$console`,
//! the network entries of each view, at most 200 per view) grouped by method and URL (without its
//! fragment), each row with what every view saw. A row is `partial` when some view with captured
//! requests did not load it (`srcset` picking another image per breakpoint), and `differs` when
//! the views saw different statuses, a failure in some, or sizes more than 5 % and 256 bytes apart.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::ViewId;
use serde_json::{Value, json};

use crate::context::FEED_KEY;

/// What one view saw of a request (its newest instance).
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    /// The host's request id, for the body.
    pub id: String,
    pub status: Option<u64>,
    pub size: Option<f64>,
    pub duration_ms: Option<f64>,
    pub mime_type: Option<String>,
    pub from_cache: bool,
    pub failed: Option<String>,
    /// Milliseconds since the epoch.
    pub time: f64,
    /// How often the view loaded it.
    pub count: u32,
}

/// One row: a request by method and URL.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub method: String,
    pub url: String,
    pub resource_type: String,
    pub views: BTreeMap<ViewId, Seen>,
    pub partial: bool,
    pub differs: bool,
}

impl Row {
    /// The newest time any view saw it.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.views
            .values()
            .map(|seen| seen.time)
            .fold(0.0, f64::max)
    }
}

/// The capture read: the rows, oldest first, and the views that captured anything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capture {
    pub rows: Vec<Row>,
    pub views: BTreeSet<ViewId>,
    /// The host sent a capture at all.
    pub captured: bool,
}

fn without_fragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(base, _)| base)
}

/// The rows of `args`' capture.
#[must_use]
pub fn capture(args: &Value) -> Capture {
    let Some(views) = args[FEED_KEY]["views"].as_array() else {
        return Capture::default();
    };
    let mut rows: Vec<Row> = Vec::new();
    let mut index: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut captured_views = BTreeSet::new();
    for view in views {
        let Some(id) = view["view"]
            .as_u64()
            .and_then(|id| ViewId::try_from(id).ok())
        else {
            continue;
        };
        for entry in view["network"].as_array().into_iter().flatten() {
            let (Some(url), Some(method)) = (entry["url"].as_str(), entry["method"].as_str())
            else {
                continue;
            };
            captured_views.insert(id);
            let url = without_fragment(url).to_owned();
            let key = (method.to_owned(), url.clone());
            let at = *index.entry(key).or_insert_with(|| {
                rows.push(Row {
                    method: method.to_owned(),
                    url,
                    resource_type: entry["resourceType"].as_str().unwrap_or("Other").to_owned(),
                    views: BTreeMap::new(),
                    partial: false,
                    differs: false,
                });
                rows.len() - 1
            });
            let count = rows[at].views.get(&id).map_or(0, |seen| seen.count) + 1;
            rows[at].views.insert(
                id,
                Seen {
                    id: entry["id"].as_str().unwrap_or_default().to_owned(),
                    status: entry["status"].as_u64(),
                    size: entry["size"].as_f64(),
                    duration_ms: entry["durationMs"].as_f64(),
                    mime_type: entry["mimeType"].as_str().map(str::to_owned),
                    from_cache: entry["fromCache"] == Value::Bool(true),
                    failed: entry["failed"].as_str().map(str::to_owned),
                    time: entry["time"].as_f64().unwrap_or(0.0),
                    count,
                },
            );
        }
    }
    for row in &mut rows {
        row.partial = row.views.len() < captured_views.len();
        row.differs = differs(row.views.values());
    }
    Capture {
        rows,
        views: captured_views,
        captured: true,
    }
}

fn differs<'a>(seen: impl Iterator<Item = &'a Seen> + Clone) -> bool {
    let statuses: BTreeSet<Option<u64>> = seen.clone().map(|s| s.status).collect();
    let failures: BTreeSet<bool> = seen.clone().map(|s| s.failed.is_some()).collect();
    let sizes: Vec<f64> = seen.filter_map(|s| s.size).collect();
    let size_apart = match (
        sizes.iter().copied().reduce(f64::min),
        sizes.iter().copied().reduce(f64::max),
    ) {
        (Some(min), Some(max)) => max - min > 256.0 && max > min * 1.05,
        _ => false,
    };
    statuses.len() > 1 || failures.len() > 1 || size_apart
}

/// Which rows the list shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Lower-case text found in the URL.
    pub search: Option<String>,
    /// Resource types; all when empty.
    pub types: BTreeSet<String>,
    /// `ok` (below 400), `error` (400 and up), `failed` (no answer); all when none.
    pub status: Option<String>,
    /// Only rows that are partial or differ.
    pub differences: bool,
}

impl Filter {
    /// Whether `row` passes.
    #[must_use]
    pub fn passes(&self, row: &Row) -> bool {
        if let Some(search) = &self.search
            && !row.url.to_lowercase().contains(search.as_str())
        {
            return false;
        }
        if !self.types.is_empty() && !self.types.contains(&row.resource_type) {
            return false;
        }
        if self.differences && !row.partial && !row.differs {
            return false;
        }
        let seen = || row.views.values();
        match self.status.as_deref() {
            Some("ok") => seen().any(|s| s.failed.is_none() && s.status.is_some_and(|c| c < 400)),
            Some("error") => seen().any(|s| s.status.is_some_and(|c| c >= 400)),
            Some("failed") => seen().any(|s| s.failed.is_some()),
            _ => true,
        }
    }
}

/// `row` as the answer carries it.
#[must_use]
pub fn row_json(row: &Row) -> Value {
    let views: serde_json::Map<String, Value> = row
        .views
        .iter()
        .map(|(view, seen)| {
            (
                view.to_string(),
                json!({ "id": seen.id, "status": seen.status, "size": seen.size,
                        "durationMs": seen.duration_ms, "mimeType": seen.mime_type,
                        "fromCache": seen.from_cache, "failed": seen.failed,
                        "time": seen.time, "count": seen.count }),
            )
        })
        .collect();
    json!({ "method": row.method, "url": row.url, "type": row.resource_type, "views": views,
            "partial": row.partial, "differs": row.differs })
}

/// The newest `limit` rows `filter` passes, newest first, within `budget` bytes of JSON; and how
/// many passed in all.
#[must_use]
pub fn page(
    capture: &Capture,
    filter: &Filter,
    limit: usize,
    budget: usize,
) -> (Vec<Value>, usize) {
    let mut passing: Vec<&Row> = capture
        .rows
        .iter()
        .filter(|row| filter.passes(row))
        .collect();
    passing.sort_by(|a, b| b.time().total_cmp(&a.time()));
    let total = passing.len();
    let mut used = 0;
    let mut out = Vec::new();
    for row in passing.into_iter().take(limit) {
        let value = row_json(row);
        used += value.to_string().len() + 1;
        if used > budget {
            break;
        }
        out.push(value);
    }
    (out, total)
}

#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
