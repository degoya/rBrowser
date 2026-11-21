//! The endpoint analysis' check rules (RB-096-06), over each view's current page load (the
//! requests after its last new document):
//!
//! - `endpoint.duplicate` (warning): the same method and URL called twice or more;
//! - `endpoint.n-plus-one` (warning): one endpoint called with `endpoints.nPlusOne` or more
//!   different URLs (`/api/items/1`, `/2`, ... - a loop of calls where one would do);
//! - `endpoint.uncompressed` (warning): a text, JSON, script or SVG answer of at least
//!   `endpoints.largeBytes` sent without `Content-Encoding`;
//! - `endpoint.no-cache` (info): a `GET` answered `200` with nothing a cache could keep or
//!   revalidate (no `Cache-Control`, `Expires`, `ETag`, `Last-Modified`);
//! - `endpoint.slow` (warning): the p95 duration at or above `endpoints.slowMs`;
//! - `endpoint.breakpoint-calls` (info): one breakpoint calls an endpoint
//!   `endpoints.breakpointDifference` or more times more often than another (mobile loads more) -
//!   one finding, at the view calling most.
//!
//! A finding names the view's page (without fragment) and the endpoint as its path, so the same
//! finding of every view merges into one. URLs are masked; rules answer only from the capture
//! the host keeps, the responses' facts only from Chromium views.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{Finding, RuleInfo, Severity, ViewId};
use rb_util::mask::mask_url;
use serde_json::{Value, json};

use crate::calls::{Call, compressible};
use crate::endpoint_settings::Limits;
use crate::endpoints::Endpoint;
use crate::stats::Spread;

pub const DUPLICATE: &str = "endpoint.duplicate";
pub const N_PLUS_ONE: &str = "endpoint.n-plus-one";
pub const UNCOMPRESSED: &str = "endpoint.uncompressed";
pub const NO_CACHE: &str = "endpoint.no-cache";
pub const SLOW: &str = "endpoint.slow";
pub const BREAKPOINT_CALLS: &str = "endpoint.breakpoint-calls";

/// The most findings of one run.
pub const MAX_FINDINGS: usize = 500;
/// The longest message parameter, in characters (the host takes 256).
const MAX_PARAM: usize = 200;

const RULES: &[(&str, Severity)] = &[
    (DUPLICATE, Severity::Warning),
    (N_PLUS_ONE, Severity::Warning),
    (UNCOMPRESSED, Severity::Warning),
    (NO_CACHE, Severity::Info),
    (SLOW, Severity::Warning),
    (BREAKPOINT_CALLS, Severity::Info),
];

/// The rules of `check.describe`.
#[must_use]
pub fn describe() -> Vec<RuleInfo> {
    RULES
        .iter()
        .map(|(id, severity)| RuleInfo {
            id: (*id).to_owned(),
            title: crate::key(&format!("rules.{id}")),
            severity: *severity,
            shared_id: None,
            help_url: None,
        })
        .collect()
}

/// A view's page, for the findings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub view: ViewId,
    pub label: String,
    pub url: String,
}

struct Out<'a> {
    enabled: &'a [String],
    only: Option<ViewId>,
    pages: &'a [Page],
    findings: Vec<Finding>,
}

impl Out<'_> {
    fn wants(&self, rule: &str) -> bool {
        self.enabled.is_empty() || self.enabled.iter().any(|id| id == rule)
    }

    fn label(&self, view: ViewId) -> String {
        self.pages
            .iter()
            .find(|page| page.view == view)
            .map_or_else(|| view.to_string(), |page| page.label.clone())
    }

    fn emit(
        &mut self,
        rule: &str,
        view: ViewId,
        path: String,
        params: Vec<(&str, String)>,
        data: Value,
    ) {
        if !self.wants(rule)
            || self.only.is_some_and(|only| only != view)
            || self.findings.len() >= MAX_FINDINGS
        {
            return;
        }
        let page = self.pages.iter().find(|page| page.view == view);
        let severity = RULES
            .iter()
            .find(|(id, _)| *id == rule)
            .map_or(Severity::Info, |(_, severity)| *severity);
        self.findings.push(Finding {
            rule: rule.to_owned(),
            severity,
            message: crate::key(&format!("findings.{rule}")),
            params: params
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.chars().take(MAX_PARAM).collect()))
                .collect(),
            view: Some(view),
            url: page.map(|page| {
                page.url
                    .split_once('#')
                    .map_or(page.url.as_str(), |(at, _)| at)
                    .to_owned()
            }),
            selector: None,
            path: Some(path),
            data: Some(data.to_string()),
        });
    }
}

/// The current page's calls of `endpoint` by view.
fn current<'a>(endpoint: &Endpoint<'a>) -> BTreeMap<ViewId, Vec<&'a Call>> {
    let mut out: BTreeMap<ViewId, Vec<&'a Call>> = BTreeMap::new();
    for call in endpoint.calls.iter().copied().filter(|call| call.current) {
        out.entry(call.view).or_default().push(call);
    }
    out
}

fn ms(value: f64) -> String {
    format!("{value:.0}")
}

fn per_view(
    out: &mut Out<'_>,
    endpoint: &Endpoint<'_>,
    view: ViewId,
    calls: &[&Call],
    limits: &Limits,
) {
    let key = &endpoint.key;
    let mut urls: BTreeMap<String, usize> = BTreeMap::new();
    for call in calls {
        *urls.entry(mask_url(&call.url)).or_default() += 1;
    }
    for (url, count) in urls.iter().filter(|(_, count)| **count >= 2) {
        out.emit(
            DUPLICATE,
            view,
            format!("{} {url}", endpoint.method),
            vec![
                ("url", url.clone()),
                ("method", endpoint.method.clone()),
                ("count", count.to_string()),
            ],
            json!({ "endpoint": key, "url": url, "count": count }),
        );
    }
    if urls.len() >= limits.n_plus_one {
        out.emit(
            N_PLUS_ONE,
            view,
            key.clone(),
            vec![("endpoint", key.clone()), ("count", urls.len().to_string())],
            json!({ "endpoint": key, "urls": urls.len(), "calls": calls.len() }),
        );
    }
    let uncompressed = calls
        .iter()
        .filter(|call| {
            call.response
                .as_ref()
                .is_some_and(|response| response.encoding.is_none())
                && compressible(call.mime_type.as_deref())
        })
        .filter_map(|call| call.body_size())
        .filter(|size| *size >= limits.large_bytes)
        .reduce(f64::max);
    if let Some(size) = uncompressed {
        out.emit(
            UNCOMPRESSED,
            view,
            key.clone(),
            vec![("endpoint", key.clone()), ("size", format!("{size:.0}"))],
            json!({ "endpoint": key, "bytes": size }),
        );
    }
    let uncached = calls
        .iter()
        .filter(|call| {
            call.method == "GET"
                && call.status == Some(200)
                && !call.failed
                && call
                    .response
                    .as_ref()
                    .is_some_and(crate::calls::Response::uncached)
        })
        .count();
    if uncached > 0 {
        out.emit(
            NO_CACHE,
            view,
            key.clone(),
            vec![("endpoint", key.clone()), ("count", uncached.to_string())],
            json!({ "endpoint": key, "calls": uncached }),
        );
    }
    let durations = calls
        .iter()
        .filter(|call| !call.failed)
        .filter_map(|call| call.duration_ms);
    if let Some(spread) = Spread::of(durations)
        && spread.p95 >= limits.slow_ms
    {
        out.emit(
            SLOW,
            view,
            key.clone(),
            vec![
                ("endpoint", key.clone()),
                ("p95", ms(spread.p95)),
                ("median", ms(spread.median)),
                ("limit", ms(limits.slow_ms)),
            ],
            json!({ "endpoint": key, "durationMs": spread.json(), "limitMs": limits.slow_ms }),
        );
    }
}

fn breakpoints(
    out: &mut Out<'_>,
    endpoint: &Endpoint<'_>,
    by_view: &BTreeMap<ViewId, Vec<&Call>>,
    active: &BTreeSet<ViewId>,
    limits: &Limits,
) {
    if active.len() < 2 {
        return;
    }
    let counts: Vec<(ViewId, usize)> = active
        .iter()
        .map(|view| (*view, by_view.get(view).map_or(0, Vec::len)))
        .collect();
    let (Some(most), Some(least)) = (
        counts.iter().max_by_key(|(_, n)| *n),
        counts.iter().min_by_key(|(_, n)| *n),
    ) else {
        return;
    };
    if most.1 - least.1 < limits.breakpoint_difference {
        return;
    }
    let mut ordered = counts.clone();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let named: Vec<String> = ordered
        .iter()
        .map(|(view, n)| format!("{} {n}", out.label(*view)))
        .collect();
    let key = endpoint.key.clone();
    out.emit(
        BREAKPOINT_CALLS,
        most.0,
        key.clone(),
        vec![("endpoint", key.clone()), ("breakpoints", named.join(", "))],
        json!({ "endpoint": key, "calls": counts.iter().map(|(view, n)| json!({ "view": view, "calls": n })).collect::<Vec<_>>() }),
    );
}

/// The findings of `endpoints` (grouped over every view) for the rules `enabled` (empty: all),
/// only `only`'s when a run names a view; at most [`MAX_FINDINGS`].
#[must_use]
pub fn findings(
    endpoints: &[Endpoint<'_>],
    pages: &[Page],
    limits: &Limits,
    enabled: &[String],
    only: Option<ViewId>,
) -> Vec<Finding> {
    let mut out = Out {
        enabled,
        only,
        pages,
        findings: Vec::new(),
    };
    let active: BTreeSet<ViewId> = endpoints
        .iter()
        .flat_map(|endpoint| {
            endpoint
                .calls
                .iter()
                .filter(|call| call.current)
                .map(|call| call.view)
        })
        .collect();
    for endpoint in endpoints {
        let by_view = current(endpoint);
        for (view, calls) in &by_view {
            per_view(&mut out, endpoint, *view, calls, limits);
        }
        breakpoints(&mut out, endpoint, &by_view, &active, limits);
    }
    out.findings
}

#[cfg(test)]
#[path = "endpoint_rules_tests.rs"]
mod tests;
