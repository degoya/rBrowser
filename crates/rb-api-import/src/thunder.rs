//! Thunder Client's exports: a collection (folders by `containerId`, requests, the collection's
//! and folders' `settings` with their auth and headers) and environments (one, or a list).
//!
//! Mapped: method, URL (its params appended when the URL has no query, path params put in),
//! headers (disabled ones kept off), the collection's headers before each request's, bodies JSON,
//! text, XML, form, multipart (a file as `./<file name>`), GraphQL, binary; auth basic, bearer,
//! digest, OAuth 2 and `inherit` (the folder's, else the collection's); `{{#guid}}` and
//! `{{#timestamp}}` as `{{$guid}}` and `{{$timestamp}}`; the docs as comments. Reported: tests,
//! pre- and post-request scripts, other auth kinds, other `{{#...}}` variables.

use std::collections::HashMap;

use rb_api_model::{ApiRequest, Auth, Body, Entry, OAuthGrant, Part};
use serde_json::Value;

use crate::Source;
use crate::collect::Collector;
use crate::common;
use crate::json::{as_text, flag, list, text};

/// The deepest folder chain followed.
const MAX_NESTING: usize = 32;

/// Reads a Thunder Client collection or environment export.
///
/// # Errors
///
/// Not JSON, or neither a collection nor an environment.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let value: Value =
        serde_json::from_str(&source.text).map_err(|error| format!("not JSON: {error}"))?;
    if let Value::Array(items) = &value {
        for item in items {
            environment(item, out);
        }
        return Ok(());
    }
    if value["environmentName"].is_string()
        || (value["variables"].is_array() && !value["requests"].is_array())
    {
        environment(&value, out);
        return Ok(());
    }
    if !value["requests"].is_array() {
        return Err("no Thunder Client collection or environment".to_owned());
    }
    collection(&value, out);
    Ok(())
}

fn environment(value: &Value, out: &mut Collector) {
    let variables: Vec<(String, String, bool)> = list(value, "variables")
        .iter()
        .map(|v| {
            (
                text(v, "name").into_owned(),
                as_text(&v["value"]).into_owned(),
                false,
            )
        })
        .collect();
    out.environment(&text(value, "environmentName"), &variables);
}

fn collection(value: &Value, out: &mut Collector) {
    let name = text(value, "collectionName").into_owned();
    out.collection(&name);
    let settings = &value["settings"];
    settings_notes(settings, &name, out);
    let folders: HashMap<String, &Value> = list(value, "folders")
        .iter()
        .map(|folder| (text(folder, "_id").into_owned(), folder))
        .collect();
    for folder in list(value, "folders") {
        settings_notes(&folder["settings"], &text(folder, "name"), out);
    }
    let shared: Vec<Entry> = headers(list(settings, "headers"));
    for raw in list(value, "requests") {
        let (path, inherited) = chain(raw, &folders, &settings["auth"]);
        request(raw, &path, inherited, &shared, out);
    }
}

fn settings_notes(settings: &Value, at: &str, out: &mut Collector) {
    if !list(settings, "tests").is_empty() {
        out.reported(at, "tests", &list(settings, "tests").len().to_string());
    }
    for key in ["preReq", "postReq"] {
        if !settings[key].is_null() {
            out.reported(at, "script", key);
        }
    }
}

/// The folder names above `request`, outermost first, and the auth it inherits.
fn chain<'a>(
    request: &Value,
    folders: &HashMap<String, &'a Value>,
    collection: &'a Value,
) -> (Vec<String>, &'a Value) {
    let mut names = Vec::new();
    let mut auth: Option<&'a Value> = None;
    let mut id = text(request, "containerId").into_owned();
    while let Some(&folder) = folders.get(&id) {
        if names.len() >= MAX_NESTING {
            break;
        }
        names.push(text(folder, "name").into_owned());
        let own = &folder["settings"]["auth"];
        if auth.is_none() && own.is_object() && text(own, "type") != "inherit" {
            auth = Some(own);
        }
        id = text(folder, "containerId").into_owned();
    }
    names.reverse();
    (names, auth.unwrap_or(collection))
}

fn headers(items: &[Value]) -> Vec<Entry> {
    items
        .iter()
        .map(|item| Entry {
            name: text(item, "name").into_owned(),
            value: as_text(&item["value"]).into_owned(),
            enabled: !flag(item, "isDisabled"),
        })
        .collect()
}

/// `text` with Thunder Client's `{{#name}}` system variables in rBrowser's form.
fn system(text: &str, at: &str, out: &mut Collector) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("{{#") {
        let Some(close) = rest[open..].find("}}") else {
            break;
        };
        result.push_str(&rest[..open]);
        let name = rest[open + 3..open + close].trim();
        match name {
            "guid" => result.push_str("{{$guid}}"),
            "timestamp" => result.push_str("{{$timestamp}}"),
            other => {
                out.reported(at, "variable", &format!("#{other}"));
                result.push_str(&rest[open..open + close + 2]);
            }
        }
        rest = &rest[open + close + 2..];
    }
    result.push_str(rest);
    result
}

fn request(
    raw: &Value,
    folders: &[String],
    inherited: &Value,
    shared: &[Entry],
    out: &mut Collector,
) {
    let name = text(raw, "name").into_owned();
    let mut url = system(&text(raw, "url"), &name, out);
    let mut query = Vec::new();
    let mut path = Vec::new();
    for param in list(raw, "params") {
        let key = text(param, "name").into_owned();
        let value = as_text(&param["value"]).into_owned();
        if flag(param, "isPath") {
            url = url.replace(&format!("{{{key}}}"), &value);
            path.push((key, value));
        } else if !flag(param, "isDisabled") {
            query.push((key, value));
        }
    }
    let mut url = common::path_params(&url, &path);
    if !url.contains('?') {
        url = common::with_query(&url, &query);
    }
    let mut request = ApiRequest::new(&text(raw, "method"), &url);
    request.name = name.clone();
    let own = headers(list(raw, "headers"));
    let mut all: Vec<Entry> = shared
        .iter()
        .filter(|header| {
            !own.iter()
                .any(|o| o.name.eq_ignore_ascii_case(&header.name))
        })
        .cloned()
        .collect();
    all.extend(own);
    for header in &mut all {
        header.value = system(&header.value, &name, out);
    }
    request.headers = all;
    request.body = body(&raw["body"], &name, &mut request.headers, out);
    let auth = match &raw["auth"] {
        Value::Object(_) if text(&raw["auth"], "type") != "inherit" => &raw["auth"],
        _ => inherited,
    };
    request.auth = auth_of(auth, &name, out);
    let tests = list(raw, "tests").len();
    if tests > 0 {
        out.reported(&name, "tests", &tests.to_string());
    }
    for key in ["preReq", "postReq"] {
        if !raw[key].is_null() {
            out.reported(&name, "script", key);
        }
    }
    let docs = text(raw, "docs");
    if !docs.trim().is_empty() {
        request.comments = docs.lines().map(str::to_owned).collect();
    }
    out.request(folders, request);
}

fn body(body: &Value, at: &str, headers: &mut Vec<Entry>, out: &mut Collector) -> Body {
    let raw = system(&text(body, "raw"), at, out);
    match &*text(body, "type") {
        "json" => common::text_body(raw, "json", headers),
        "xml" => common::text_body(raw, "xml", headers),
        "text" => common::text_body(raw, "text", headers),
        "formencoded" => Body::Form {
            fields: self::headers(list(body, "form")),
        },
        "formdata" => {
            let mut parts: Vec<Part> = list(body, "form")
                .iter()
                .filter(|field| !flag(field, "isDisabled"))
                .map(|field| Part {
                    name: text(field, "name").into_owned(),
                    value: as_text(&field["value"]).into_owned(),
                    file: None,
                    content_type: None,
                })
                .collect();
            for file in list(body, "files") {
                parts.push(Part {
                    name: text(file, "name").into_owned(),
                    value: String::new(),
                    file: Some(common::local_file(&text(file, "value"), at, out)),
                    content_type: None,
                });
            }
            Body::Multipart { parts }
        }
        "graphql" => Body::GraphQl {
            query: text(&body["graphql"], "query").into_owned(),
            variables: text(&body["graphql"], "variables").into_owned(),
        },
        "binary" => {
            let path = text(body, "binary").into_owned();
            if path.trim().is_empty() {
                Body::None
            } else {
                Body::Binary {
                    path: common::local_file(&path, at, out),
                }
            }
        }
        "" | "none" => Body::None,
        other => {
            out.dropped(at, "body", other);
            Body::None
        }
    }
}

fn auth_of(auth: &Value, at: &str, out: &mut Collector) -> Auth {
    let kind = text(auth, "type").into_owned();
    let of = |section: &str, key: &str| as_text(&auth[section][key]).into_owned();
    match kind.as_str() {
        "" | "none" | "inherit" => Auth::None,
        "basic" => Auth::Basic {
            username: of("basic", "username"),
            password: of("basic", "password"),
        },
        "digest" => Auth::Digest {
            username: of("digest", "username"),
            password: of("digest", "password"),
        },
        "bearer" => {
            let prefix = text(auth, "bearerPrefix");
            if !prefix.is_empty() && !prefix.eq_ignore_ascii_case("Bearer") {
                out.reported(at, "auth", &format!("bearer prefix {prefix}"));
            }
            Auth::Bearer {
                token: as_text(&auth["bearer"]).into_owned(),
            }
        }
        "oauth2" => {
            let grant = match of("oauth2", "grantType").as_str() {
                "" | "client_credentials" => OAuthGrant::ClientCredentials,
                "authorization_code" => OAuthGrant::AuthorizationCode,
                other => {
                    out.reported(at, "auth", &format!("oauth2 {other}"));
                    return Auth::None;
                }
            };
            Auth::OAuth2 {
                grant,
                token_url: of("oauth2", "tokenUrl"),
                auth_url: of("oauth2", "authUrl"),
                client_id: of("oauth2", "clientId"),
                client_secret: of("oauth2", "clientSecret"),
                scope: of("oauth2", "scope"),
                redirect_uri: of("oauth2", "callbackUrl"),
            }
        }
        other => {
            out.reported(at, "auth", other);
            Auth::None
        }
    }
}

#[cfg(test)]
#[path = "thunder_tests.rs"]
mod tests;
