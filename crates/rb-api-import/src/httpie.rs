//! HTTPie Desktop's export (`meta.format: httpie`): a collection, or a workspace of collections
//! (each its own folder).
//!
//! Mapped: method, URL, enabled query and path parameters, headers (disabled ones kept off),
//! bodies text (JSON by its format), form, multipart (a file as `./<file name>`), file, GraphQL;
//! auth basic, bearer, digest, API key and `inherited` (the collection's). Reported: other auth
//! kinds; dropped: disabled query parameters and fields.

use rb_api_model::{ApiRequest, Auth, Body, Entry, KeyPlace, Part};
use serde_json::Value;

use crate::Source;
use crate::collect::Collector;
use crate::common;
use crate::json::{as_text, list, text};

/// Reads an HTTPie Desktop export.
///
/// # Errors
///
/// Not JSON, or no HTTPie export.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let value: Value =
        serde_json::from_str(&source.text).map_err(|error| format!("not JSON: {error}"))?;
    if text(&value["meta"], "format") != "httpie" {
        return Err("no HTTPie export".to_owned());
    }
    let entry = &value["entry"];
    out.collection(&text(entry, "name"));
    if entry["collections"].is_array() {
        for collection in list(entry, "collections") {
            let folder = vec![text(collection, "name").into_owned()];
            requests(collection, &folder, out);
        }
    } else {
        requests(entry, &[], out);
    }
    Ok(())
}

fn enabled(item: &Value) -> bool {
    item["enabled"] != Value::Bool(false)
}

fn requests(collection: &Value, folders: &[String], out: &mut Collector) {
    let inherited = &collection["auth"];
    for raw in list(collection, "requests") {
        let name = text(raw, "name").into_owned();
        let mut query = Vec::new();
        for param in list(raw, "queryParams") {
            let key = text(param, "name").into_owned();
            if enabled(param) {
                query.push((key, as_text(&param["value"]).into_owned()));
            } else {
                out.dropped(&name, "disabled", &format!("query {key}"));
            }
        }
        let path: Vec<(String, String)> = list(raw, "pathParams")
            .iter()
            .map(|p| {
                (
                    text(p, "name").into_owned(),
                    as_text(&p["value"]).into_owned(),
                )
            })
            .collect();
        let url = common::path_params(&common::with_query(&text(raw, "url"), &query), &path);
        let mut request = ApiRequest::new(&text(raw, "method"), &url);
        request.name = name.clone();
        request.headers = list(raw, "headers")
            .iter()
            .map(|header| Entry {
                name: text(header, "name").into_owned(),
                value: as_text(&header["value"]).into_owned(),
                enabled: enabled(header),
            })
            .collect();
        request.body = body(&raw["body"], &name, &mut request.headers, out);
        let auth = match &*text(&raw["auth"], "type") {
            "" | "inherited" | "inherit" => inherited,
            _ => &raw["auth"],
        };
        request.auth = auth_of(auth, &name, out);
        out.request(folders, request);
    }
}

fn body(body: &Value, at: &str, headers: &mut Vec<Entry>, out: &mut Collector) -> Body {
    match &*text(body, "type") {
        "" | "none" => Body::None,
        "text" => {
            let format = text(&body["text"], "format").to_ascii_lowercase();
            let raw = text(&body["text"], "value").into_owned();
            if format.contains("json") {
                common::text_body(raw, "json", headers)
            } else {
                if !format.is_empty() && common::content_type(headers).is_empty() {
                    headers.push(Entry::new("Content-Type", &format));
                }
                common::text_body(raw, "", headers)
            }
        }
        "form" => {
            let form = &body["form"];
            let fields = list(form, "fields");
            if form["isMultipart"] == Value::Bool(true) {
                let mut parts = Vec::new();
                for field in fields {
                    let name = text(field, "name").into_owned();
                    if !enabled(field) {
                        out.dropped(at, "disabled", &format!("part {name}"));
                        continue;
                    }
                    let file = (text(field, "type") == "file")
                        .then(|| common::local_file(&text(field, "value"), at, out));
                    parts.push(Part {
                        value: if file.is_some() {
                            String::new()
                        } else {
                            as_text(&field["value"]).into_owned()
                        },
                        name,
                        file,
                        content_type: None,
                    });
                }
                Body::Multipart { parts }
            } else {
                Body::Form {
                    fields: fields
                        .iter()
                        .map(|field| Entry {
                            name: text(field, "name").into_owned(),
                            value: as_text(&field["value"]).into_owned(),
                            enabled: enabled(field),
                        })
                        .collect(),
                }
            }
        }
        "file" => {
            let path = text(&body["file"], "name").into_owned();
            if path.trim().is_empty() {
                Body::None
            } else {
                Body::Binary {
                    path: common::local_file(&path, at, out),
                }
            }
        }
        "graphql" => Body::GraphQl {
            query: text(&body["graphql"], "query").into_owned(),
            variables: text(&body["graphql"], "variables").into_owned(),
        },
        other => {
            out.dropped(at, "body", other);
            Body::None
        }
    }
}

fn auth_of(auth: &Value, at: &str, out: &mut Collector) -> Auth {
    let credentials = &auth["credentials"];
    let username = text(credentials, "username").into_owned();
    let password = text(credentials, "password").into_owned();
    match &*text(auth, "type") {
        "" | "none" | "inherited" | "inherit" => Auth::None,
        "basic" => Auth::Basic { username, password },
        "digest" => Auth::Digest { username, password },
        "bearer" => Auth::Bearer {
            token: Some(text(credentials, "token").into_owned())
                .filter(|token| !token.is_empty())
                .unwrap_or(password),
        },
        "apiKey" | "apikey" => Auth::ApiKey {
            name: username,
            value: password,
            place: if text(auth, "target")
                .to_ascii_lowercase()
                .starts_with("query")
            {
                KeyPlace::Query
            } else {
                KeyPlace::Header
            },
        },
        other => {
            out.reported(at, "auth", other);
            Auth::None
        }
    }
}

#[cfg(test)]
#[path = "httpie_tests.rs"]
mod tests;
