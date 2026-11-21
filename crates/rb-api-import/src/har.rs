//! A HAR capture (the browser's network log): each call a request, in a folder per host.
//!
//! Mapped: method, URL, headers, the posted body (JSON, form, multipart with a file as
//! `./<file name>`, other text); every cookie, token and password a placeholder like any other
//! secret. Dropped, and named once: the calls for images, styles, scripts, fonts and media
//! (`_resourceType`), the headers HTTP/2 or the connection sets (`:authority`, `Host`,
//! `Content-Length`, ...). The answers are not part of a request and stay out.

use rb_api_model::{ApiRequest, Body, Entry, Part};
use serde_json::Value;

use crate::Source;
use crate::collect::Collector;
use crate::common;
use crate::json::{as_text, list, text};

/// The resource types of static files, not taken.
const STATIC: &[&str] = &[
    "image",
    "stylesheet",
    "script",
    "font",
    "media",
    "manifest",
    "texttrack",
    "preflight",
];
/// The headers the connection sets, not taken.
const TRANSPORT: &[&str] = &[
    "host",
    "content-length",
    "connection",
    "transfer-encoding",
    "keep-alive",
    "upgrade",
];

/// Reads a HAR capture.
///
/// # Errors
///
/// Not JSON, or no `log.entries`.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let value: Value =
        serde_json::from_str(&source.text).map_err(|error| format!("not JSON: {error}"))?;
    if !value["log"]["entries"].is_array() {
        return Err("no HAR log".to_owned());
    }
    let file = source.name.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let name = if stem.is_empty() { "HAR" } else { stem };
    out.collection(name);
    let mut skipped = 0usize;
    let mut transport: Vec<String> = Vec::new();
    for entry in list(&value["log"], "entries") {
        let kind = text(entry, "_resourceType").to_ascii_lowercase();
        if STATIC.contains(&kind.as_str()) {
            skipped += 1;
            continue;
        }
        let raw = &entry["request"];
        let url = text(raw, "url").into_owned();
        let mut request = ApiRequest::new(&text(raw, "method"), &url);
        for header in list(raw, "headers") {
            let header_name = text(header, "name").into_owned();
            let lower = header_name.to_ascii_lowercase();
            if lower.starts_with(':') || TRANSPORT.contains(&lower.as_str()) {
                if !transport.contains(&lower) {
                    transport.push(lower);
                }
                continue;
            }
            request.headers.push(Entry::new(
                header_name,
                as_text(&header["value"]).into_owned(),
            ));
        }
        let at = crate::collect::title_of(&request);
        request.body = body(&raw["postData"], &at, &request.headers, out);
        let host = host_of(&url);
        let folder: Vec<String> = if host.is_empty() {
            Vec::new()
        } else {
            vec![host]
        };
        out.request(&folder, request);
    }
    if skipped > 0 {
        out.dropped(name, "skipped", &skipped.to_string());
    }
    if !transport.is_empty() {
        out.dropped(name, "transport", &transport.join(", "));
    }
    Ok(())
}

fn host_of(url: &str) -> String {
    url.split_once("://")
        .map_or("", |(_, rest)| rest)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_owned()
}

fn body(data: &Value, at: &str, headers: &[Entry], out: &mut Collector) -> Body {
    if !data.is_object() {
        return Body::None;
    }
    let mime = text(data, "mimeType").to_ascii_lowercase();
    let raw = text(data, "text").into_owned();
    let params = list(data, "params");
    if mime.starts_with("application/x-www-form-urlencoded") {
        let fields = if params.is_empty() {
            raw.split('&')
                .filter(|pair| !pair.is_empty())
                .map(|pair| {
                    let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                    Entry::new(
                        rb_api_model::http_parse::form_unescape(name),
                        rb_api_model::http_parse::form_unescape(value),
                    )
                })
                .collect()
        } else {
            params
                .iter()
                .map(|p| {
                    Entry::new(
                        text(p, "name").into_owned(),
                        as_text(&p["value"]).into_owned(),
                    )
                })
                .collect()
        };
        return Body::Form { fields };
    }
    if mime.starts_with("multipart/form-data") && !params.is_empty() {
        let parts = params
            .iter()
            .map(|p| {
                let file_name = text(p, "fileName").into_owned();
                let file = (!file_name.is_empty()).then(|| common::local_file(&file_name, at, out));
                Part {
                    name: text(p, "name").into_owned(),
                    value: if file.is_some() {
                        String::new()
                    } else {
                        as_text(&p["value"]).into_owned()
                    },
                    file,
                    content_type: Some(text(p, "contentType").into_owned())
                        .filter(|t| !t.is_empty()),
                }
            })
            .collect();
        return Body::Multipart { parts };
    }
    let mut headers = headers.to_vec();
    let language = if mime.contains("json") { "json" } else { "" };
    common::text_body(raw, language, &mut headers)
}

#[cfg(test)]
#[path = "har_tests.rs"]
mod tests;
