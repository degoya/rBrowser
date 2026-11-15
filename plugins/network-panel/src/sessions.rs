//! Sessions (RB-080-37): the captured requests of every view saved as one HAR 1.2 file in
//! `.rbrowser/out/network-panel/sessions/<stamp>.har` (`session_save`), with rBrowser's own
//! fields per entry - `_view`, `_viewLabel`, `_breakpoint`, `_resourceType`, `_partial`,
//! `_differs` - and the secrets masked with the app's one list (`rb_util::mask`: tokens in
//! URLs, cookie and authorization headers). A saved session opens for display only (`session_open`): its entries
//! grouped into the list's rows, never applied as rules. `sessions_list` names the files.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::context::Settings;
use crate::requests::{self, Capture};
use crate::{bad_args, coded, har, mocks};

/// The sessions folder, relative to the plugin's out folder.
pub const DIR: &str = "sessions";
/// The sessions folder, relative to the project.
pub const PATH: &str = ".rbrowser/out/network-panel/sessions";
/// The most entries a session file is read with.
pub const MAX_ENTRIES: usize = 5000;

pub use rb_util::mask::mask_url;
use rb_util::mask::{MASK, sensitive_header};

/// Masks the sensitive headers of a HAR header list in place.
fn mask_headers(headers: &mut Value) {
    if let Value::Array(list) = headers {
        for header in list.iter_mut() {
            if header["name"].as_str().is_some_and(sensitive_header) {
                header["value"] = json!(MASK);
            }
        }
    }
}

/// `har` with every entry masked and the rows' marks added.
fn annotate(har: &mut Value, capture: &Capture, breakpoints: &BTreeMap<ViewId, String>) {
    let Some(entries) = har["log"]["entries"].as_array_mut() else {
        return;
    };
    for entry in entries {
        if let Some(url) = entry["request"]["url"].as_str() {
            let masked = mask_url(url);
            entry["request"]["url"] = json!(masked);
            entry["request"]["queryString"] = Value::Array(
                masked
                    .split_once('?')
                    .map(|(_, rest)| rest.split('#').next().unwrap_or_default())
                    .unwrap_or_default()
                    .split('&')
                    .filter(|pair| !pair.is_empty())
                    .map(|pair| {
                        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                        json!({ "name": name, "value": value })
                    })
                    .collect(),
            );
        }
        mask_headers(&mut entry["request"]["headers"]);
        mask_headers(&mut entry["response"]["headers"]);
        let view = entry["_view"]
            .as_u64()
            .and_then(|view| ViewId::try_from(view).ok());
        if let Some(view) = view {
            entry["_breakpoint"] = json!(breakpoints.get(&view));
        }
        let (method, url) = (
            entry["request"]["method"].as_str().unwrap_or("GET"),
            entry["request"]["url"].as_str().unwrap_or_default(),
        );
        if let Some(row) = capture
            .rows
            .iter()
            .find(|row| row.method == method && mask_url(&row.url) == url)
        {
            entry["_partial"] = json!(row.partial);
            entry["_differs"] = json!(row.differs);
        }
    }
}

/// The milliseconds since the epoch of an ISO 8601 UTC stamp (`2026-11-13T10:30:00.123Z`); 0
/// for anything else.
#[must_use]
pub fn time_of(stamp: &str) -> f64 {
    let digits = |from: usize, to: usize| stamp.get(from..to).and_then(|s| s.parse::<i64>().ok());
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        digits(0, 4),
        digits(5, 7),
        digits(8, 10),
        digits(11, 13),
        digits(14, 16),
        digits(17, 19),
    ) else {
        return 0.0;
    };
    let millis = digits(20, 23).unwrap_or(0);
    // Howard Hinnant's days-from-civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second;
    (secs * 1000 + millis) as f64
}

/// `session_save`: the capture of every view as `sessions/<stamp>.har`; `name` (plain, at most
/// 64 characters) names the file instead of the stamp.
///
/// # Errors
///
/// A bad `name`, or the file cannot be written.
pub fn save(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let name = match &args["name"] {
        Value::Null => har::stamp(host.now_unix_seconds()),
        Value::String(name)
            if (1..=64).contains(&name.len())
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) =>
        {
            name.clone()
        }
        _ => return Err(bad_args("name")),
    };
    let settings = Settings::read(host);
    let breakpoints: BTreeMap<ViewId, String> = host
        .views()
        .unwrap_or_default()
        .into_iter()
        .map(|view| (view.id, view.breakpoint))
        .collect();
    let capture = requests::capture(args);
    let mut har = har::build(host, args, &[], settings.har_bodies);
    annotate(&mut har, &capture, &breakpoints);
    har["log"]["_rbrowser"] = json!({ "kind": "session", "views": breakpoints.len() });
    let entries = har["log"]["entries"].as_array().map_or(0, Vec::len);
    let file = format!("{name}.har");
    host.out_write(&format!("{DIR}/{file}"), har.to_string().as_bytes())?;
    let path = format!("{PATH}/{file}");
    Ok(json!({ "path": path, "entries": entries,
               "$post": [{ "target": "panel", "message": { "sessionsChanged": true } }] }))
}

/// `sessions_list`: the saved sessions, newest name last.
#[must_use]
pub fn list(host: &mut dyn PluginHost) -> Value {
    let mut names: Vec<String> = host
        .project_list(PATH)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| name.ends_with(".har") && !name.contains('/'))
        .collect();
    names.sort();
    json!({ "sessions": names.iter().map(|name| json!({ "name": name, "path": format!("{PATH}/{name}") })).collect::<Vec<_>>() })
}

/// `session_open`: a HAR below `.rbrowser/` as the list's rows, for display only.
///
/// # Errors
///
/// `plugin.network-panel.errors.har_path` for another path, `...har_read` when it cannot be
/// read, `...har_invalid` when it is no HAR.
pub fn open(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let path = args["path"].as_str().ok_or_else(|| bad_args("path"))?;
    if !path
        .strip_prefix(".rbrowser/")
        .is_some_and(|rest| mocks::relative_path(rest) && rest.ends_with(".har"))
    {
        return Err(coded(FailureKind::BadData, "errors.har_path"));
    }
    let text = host
        .project_read(path)
        .map_err(|_| coded(FailureKind::NotFound, "errors.har_read"))?;
    let har: Value = serde_json::from_slice(&text).map_err(|_| {
        coded(FailureKind::BadData, "errors.har_invalid").with_param("reason", "no JSON")
    })?;
    let entries = har["log"]["entries"].as_array().ok_or_else(|| {
        coded(FailureKind::BadData, "errors.har_invalid").with_param("reason", "no log.entries")
    })?;
    // The entries as the capture's feed, so the rows come out as the live list's.
    let mut views: BTreeMap<u64, (String, Vec<Value>)> = BTreeMap::new();
    for entry in entries.iter().take(MAX_ENTRIES) {
        let view = entry["_view"].as_u64().unwrap_or(0);
        let label = entry["_viewLabel"]
            .as_str()
            .map_or_else(|| view.to_string(), str::to_owned);
        let request = &entry["request"];
        let response = &entry["response"];
        let network = json!({
            "id": format!("{view}.{}", entry["startedDateTime"].as_str().unwrap_or_default()),
            "url": request["url"], "method": request["method"],
            "resourceType": entry["_resourceType"].as_str().unwrap_or("Other"),
            "status": response["status"].as_u64().filter(|status| *status > 0),
            "mimeType": response["content"]["mimeType"],
            "size": response["content"]["size"],
            "time": time_of(entry["startedDateTime"].as_str().unwrap_or_default()),
            "durationMs": entry["time"],
            "fromCache": false,
            "failed": entry["_error"],
        });
        views
            .entry(view)
            .or_insert_with(|| (label, Vec::new()))
            .1
            .push(network);
    }
    let feed = json!({ "views": views.iter().map(|(view, (_, network))| json!({ "view": view, "network": network })).collect::<Vec<_>>() });
    let capture = requests::capture(&json!({ crate::context::FEED_KEY: feed }));
    let (rows, total) = requests::page(&capture, &requests::Filter::default(), 500, 48 * 1024);
    Ok(json!({
        "path": path,
        "rows": rows,
        "total": total,
        "views": views.iter().map(|(view, (label, _))| json!({ "id": view, "label": label })).collect::<Vec<_>>(),
        "readOnly": true,
    }))
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
