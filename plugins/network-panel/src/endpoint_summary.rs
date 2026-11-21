//! One endpoint as the tab Endpoints and MCP's `networkpanel_endpoints` show it (RB-096-06):
//! calls, failures, median and p95 of the duration and of the waiting time (time to first
//! byte), the sizes transferred and decoded, the status mix, the caching headers and the
//! compression, per view too, and its newest requests to jump to. Every URL is masked
//! (`rb_util::mask::mask_url`: user info and the values of secret query parameters); no header
//! but `Cache-Control` and `Content-Encoding` ever reaches an endpoint. `apiClient` is the request
//! the API client opens (RB-096-01, RB-096-08): method and the newest URL, masked, with that
//! call's view and request id for `api_open`.

use std::collections::{BTreeMap, BTreeSet};

use rb_util::mask::mask_url;
use serde_json::{Map, Value, json};

use crate::calls::Call;
use crate::endpoints::{Endpoint, by_view};
use crate::stats::{Spread, median, round, sorted};

/// The most requests of an endpoint listed, the newest.
pub const MAX_REQUESTS: usize = 20;
/// The most distinct `Cache-Control` values listed.
const MAX_CACHE_CONTROLS: usize = 3;

fn spread<'a>(calls: impl Iterator<Item = &'a Call>, of: impl Fn(&Call) -> Option<f64>) -> Value {
    Spread::of(calls.filter_map(of)).map_or(Value::Null, Spread::json)
}

fn sizes<'a>(calls: impl Iterator<Item = &'a Call>, of: impl Fn(&Call) -> Option<f64>) -> Value {
    let values = sorted(calls.filter_map(of));
    match median(&values) {
        Some(middle) => json!({ "total": round(values.iter().sum()), "median": round(middle) }),
        None => Value::Null,
    }
}

fn ttfb(call: &Call) -> Option<f64> {
    call.response.as_ref().and_then(|response| response.ttfb_ms)
}

fn counts(keys: impl Iterator<Item = String>) -> Value {
    let mut out: BTreeMap<String, u64> = BTreeMap::new();
    for key in keys {
        *out.entry(key).or_default() += 1;
    }
    Value::Object(out.into_iter().map(|(key, n)| (key, json!(n))).collect())
}

/// `endpoint` as the answers carry it.
#[must_use]
pub fn summary(endpoint: &Endpoint<'_>) -> Value {
    let calls = || endpoint.calls.iter().copied();
    let statuses = counts(calls().map(|call| match (call.failed, call.status) {
        (true, _) | (false, None) => "failed".to_owned(),
        (false, Some(status)) => status.to_string(),
    }));
    let encodings = counts(calls().filter_map(|call| {
        let response = call.response.as_ref()?;
        Some(
            response
                .encoding
                .clone()
                .unwrap_or_else(|| "none".to_owned()),
        )
    }));
    let cache_control: BTreeSet<&str> = calls()
        .filter_map(|call| call.response.as_ref()?.cache_control.as_deref())
        .collect();
    let validators = calls()
        .filter(|call| {
            call.response
                .as_ref()
                .is_some_and(|response| response.etag || response.last_modified)
        })
        .count();
    let views: Vec<Value> = by_view(endpoint)
        .into_iter()
        .map(|(view, of_view)| {
            json!({
                "view": view,
                "calls": of_view.len(),
                "durationMs": spread(of_view.iter().copied(), |call| call.duration_ms),
                "ttfbMs": spread(of_view.iter().copied(), ttfb),
            })
        })
        .collect();
    let mut newest: Vec<&Call> = calls().collect();
    newest.sort_by(|a, b| b.time.total_cmp(&a.time));
    let requests: Vec<Value> = newest
        .iter()
        .take(MAX_REQUESTS)
        .map(|call| json!({ "view": call.view, "id": call.id, "url": mask_url(&call.url) }))
        .collect();
    let mut answer = Map::new();
    answer.insert("key".to_owned(), json!(endpoint.key));
    answer.insert("method".to_owned(), json!(endpoint.method));
    answer.insert("origin".to_owned(), json!(endpoint.origin));
    answer.insert("path".to_owned(), json!(endpoint.path));
    answer.insert("calls".to_owned(), json!(endpoint.calls.len()));
    answer.insert(
        "failed".to_owned(),
        json!(calls().filter(|call| call.failed).count()),
    );
    answer.insert(
        "fromCache".to_owned(),
        json!(calls().filter(|call| call.from_cache).count()),
    );
    answer.insert(
        "durationMs".to_owned(),
        spread(calls(), |call| call.duration_ms),
    );
    answer.insert("ttfbMs".to_owned(), spread(calls(), ttfb));
    answer.insert("transferred".to_owned(), sizes(calls(), |call| call.size));
    answer.insert("decoded".to_owned(), sizes(calls(), Call::body_size));
    answer.insert("statuses".to_owned(), statuses);
    answer.insert("encodings".to_owned(), encodings);
    answer.insert(
        "cacheControl".to_owned(),
        json!(
            cache_control
                .into_iter()
                .take(MAX_CACHE_CONTROLS)
                .collect::<Vec<_>>()
        ),
    );
    answer.insert("validators".to_owned(), json!(validators));
    answer.insert("views".to_owned(), json!(views));
    answer.insert("requests".to_owned(), json!(requests));
    if let Some(call) = newest.first() {
        answer.insert(
            "apiClient".to_owned(),
            json!({ "method": endpoint.method, "url": mask_url(&call.url),
                    "view": call.view, "id": call.id }),
        );
    }
    Value::Object(answer)
}

#[cfg(test)]
#[path = "endpoint_summary_tests.rs"]
mod tests;
