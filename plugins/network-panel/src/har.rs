//! HAR 1.2 export (`har_export`, `Ctrl+Shift+Space P H`): the host's capture of every view
//! (`$console`) as one merged file or one file per view in `.rbrowser/out/network-panel/`, the
//! response bodies the views still hold read with `Network.getResponseBody` (setting
//! `harBodies`). The capture has no headers or timing phases: a HAR's `headers` are empty but for
//! the response's `content-type`, its `timings` one `wait`. Every entry names its view
//! (`_view`, `_viewLabel`) and resource type (`_resourceType`), as HAR's custom fields allow.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::context::FEED_KEY;

/// The largest body put into a HAR, in bytes of text.
pub const MAX_BODY: usize = 1024 * 1024;
/// The most body text of one HAR file.
pub const MAX_BODIES: usize = 6 * 1024 * 1024;
/// The plugin's out folder, relative to the project.
pub const OUT_PATH: &str = ".rbrowser/out/network-panel";

/// `ms` (milliseconds since the Unix epoch) as ISO 8601 in UTC with milliseconds.
#[must_use]
pub fn iso8601(ms: f64) -> String {
    let ms = if ms.is_finite() && ms > 0.0 {
        ms as u64
    } else {
        0
    };
    let (secs, millis) = (ms / 1000, ms % 1000);
    let (date, time) = civil(secs);
    format!("{date}T{time}.{millis:03}Z")
}

/// `secs` as a file name stamp, `2026-11-12T10-30-00Z`.
#[must_use]
pub fn stamp(secs: u64) -> String {
    let (date, time) = civil(secs);
    format!("{date}T{}Z", time.replace(':', "-"))
}

/// The date `YYYY-MM-DD` and time `HH:MM:SS` of `secs` since the epoch (Howard Hinnant's
/// days-to-civil).
fn civil(secs: u64) -> (String, String) {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rest = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{:02}:{:02}:{:02}", rest / 3600, rest / 60 % 60, rest % 60),
    )
}

fn query_string(url: &str) -> Vec<Value> {
    let query = url
        .split_once('?')
        .map_or("", |(_, rest)| rest.split('#').next().unwrap_or(""));
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            json!({ "name": name, "value": value })
        })
        .collect()
}

/// The body of request `id` in `view`, as HAR `content` fields, if the view still holds it.
fn body(host: &mut dyn PluginHost, view: ViewId, id: &str) -> Option<(String, bool)> {
    let params = json!({ "requestId": id }).to_string();
    let answer = host
        .cdp_send(view, "Network.getResponseBody", &params)
        .ok()?;
    let answer: Value = serde_json::from_str(&answer).ok()?;
    let text = answer["body"].as_str()?;
    (text.len() <= MAX_BODY).then(|| (text.to_owned(), answer["base64Encoded"] == true))
}

/// One HAR entry of a capture entry of `view`.
fn entry(raw: &Value, view: ViewId, label: &str, content: Option<(String, bool)>) -> Value {
    let url = raw["url"].as_str().unwrap_or_default();
    let mime = raw["mimeType"].as_str().unwrap_or("x-unknown");
    let size = raw["size"].as_f64();
    let duration = raw["durationMs"].as_f64().unwrap_or(0.0).max(0.0);
    let mut response_content = json!({ "size": size.unwrap_or(0.0), "mimeType": mime });
    if let Some((text, base64)) = content {
        response_content["text"] = json!(text);
        if base64 {
            response_content["encoding"] = json!("base64");
        }
    }
    let headers = raw["mimeType"]
        .as_str()
        .map(|mime| vec![json!({ "name": "content-type", "value": mime })])
        .unwrap_or_default();
    let mut har = json!({
        "startedDateTime": iso8601(raw["time"].as_f64().unwrap_or(0.0)),
        "time": duration,
        "request": {
            "method": raw["method"].as_str().unwrap_or("GET"), "url": url,
            "httpVersion": "HTTP/1.1", "cookies": [], "headers": [],
            "queryString": query_string(url), "headersSize": -1, "bodySize": -1,
        },
        "response": {
            "status": raw["status"].as_u64().unwrap_or(0), "statusText": "",
            "httpVersion": "HTTP/1.1", "cookies": [], "headers": headers,
            "content": response_content, "redirectURL": "", "headersSize": -1,
            "bodySize": size.unwrap_or(-1.0),
        },
        "cache": {},
        "timings": { "send": 0, "wait": duration, "receive": 0 },
        "_view": view,
        "_viewLabel": label,
        "_resourceType": raw["resourceType"].as_str().unwrap_or("Other"),
    });
    if let Some(failed) = raw["failed"].as_str() {
        har["_error"] = json!(failed);
    }
    har
}

/// The HAR of `views` (all when empty) from `args`' capture; bodies when `bodies`.
#[must_use]
pub fn build(host: &mut dyn PluginHost, args: &Value, only: &[ViewId], bodies: bool) -> Value {
    let labels: BTreeMap<ViewId, String> = host
        .views()
        .unwrap_or_default()
        .into_iter()
        .map(|view| (view.id, view.label))
        .collect();
    let mut entries: Vec<Value> = Vec::new();
    let mut body_bytes = 0usize;
    for view in args[FEED_KEY]["views"].as_array().into_iter().flatten() {
        let Some(id) = view["view"]
            .as_u64()
            .and_then(|id| ViewId::try_from(id).ok())
        else {
            continue;
        };
        if !only.is_empty() && !only.contains(&id) {
            continue;
        }
        let label = labels.get(&id).cloned().unwrap_or_else(|| id.to_string());
        for raw in view["network"].as_array().into_iter().flatten() {
            let content = if bodies
                && body_bytes < MAX_BODIES
                && raw["failed"].is_null()
                && raw["status"].is_u64()
            {
                raw["id"]
                    .as_str()
                    .and_then(|request| body(host, id, request))
                    .filter(|(text, _)| body_bytes + text.len() <= MAX_BODIES)
            } else {
                None
            };
            body_bytes += content.as_ref().map_or(0, |(text, _)| text.len());
            entries.push(entry(raw, id, &label, content));
        }
    }
    entries.sort_by(|a, b| {
        a["startedDateTime"]
            .as_str()
            .cmp(&b["startedDateTime"].as_str())
    });
    json!({ "log": {
        "version": "1.2",
        "creator": { "name": "rBrowser network-panel", "version": env!("CARGO_PKG_VERSION") },
        "entries": entries,
    } })
}

/// Writes the HAR - of view `only`, merged, or one file per view with requests - and answers
/// the paths.
///
/// # Errors
///
/// A file could not be written.
pub fn export(
    host: &mut dyn PluginHost,
    args: &Value,
    only: Option<ViewId>,
    merged: bool,
    bodies: bool,
) -> Result<Value, Failure> {
    let stamp = stamp(host.now_unix_seconds());
    let mut groups: Vec<(String, Vec<ViewId>)> = Vec::new();
    if let Some(view) = only {
        groups.push((format!("{stamp}-view{view}.har"), vec![view]));
    } else if merged {
        groups.push((format!("{stamp}.har"), Vec::new()));
    } else {
        for view in args[FEED_KEY]["views"].as_array().into_iter().flatten() {
            if let Some(id) = view["view"]
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                && view["network"]
                    .as_array()
                    .is_some_and(|list| !list.is_empty())
            {
                groups.push((format!("{stamp}-view{id}.har"), vec![id]));
            }
        }
    }
    let mut paths = Vec::new();
    let mut count = 0;
    for (name, views) in groups {
        let har = build(host, args, &views, bodies);
        count += har["log"]["entries"].as_array().map_or(0, Vec::len);
        host.out_write(&name, har.to_string().as_bytes())?;
        paths.push(format!("{OUT_PATH}/{name}"));
    }
    Ok(json!({ "path": paths.first(), "paths": paths, "entries": count }))
}

#[cfg(test)]
#[path = "har_tests.rs"]
pub(crate) mod tests;
