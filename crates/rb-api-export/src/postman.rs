//! Requests as a Postman collection, v2.1 (`schema.getpostman.com/json/collection/v2.1.0`):
//! a folder of the collection is an item with `item`, a request an item with `request` - the
//! method, every header (a disabled one `disabled`), the URL as written, the body by its mode
//! (`raw` with its language, `urlencoded`, `formdata`, `file`, `graphql`), the auth by its type
//! (`basic`, `bearer`, `apikey`, `digest`, `oauth2`), the comments as `description`;
//! `protocolProfileBehavior` for redirects not followed and no TLS check. Every `{{variable}}`
//! stays as it is - Postman writes them the same way. Postman keeps no timeout per request
//! ([`Note::Timeout`]).

use rb_api_model::request::DEFAULT_TIMEOUT_MS;
use rb_api_model::{ApiRequest, Auth, Body, Entry, KeyPlace, OAuthGrant};
use serde_json::{Map, Value, json};

use crate::Note;

/// The schema a v2.1 collection names.
pub const SCHEMA: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

/// `items` (an id relative to the export's folder, and the request) as a collection named
/// `title`, pretty-printed.
pub(crate) fn collection(
    title: &str,
    items: &[(String, ApiRequest)],
    notes: &mut Vec<Note>,
) -> String {
    let mut root: Vec<Value> = Vec::new();
    for (id, request) in items {
        let mut steps: Vec<&str> = id.split('/').collect();
        let file = steps.pop().unwrap_or_default();
        let mut level: &mut Vec<Value> = &mut root;
        for step in steps {
            let found = level.iter().position(|item| {
                item.get("item").is_some() && item.get("name").and_then(Value::as_str) == Some(step)
            });
            let at = match found {
                Some(at) => at,
                None => {
                    level.push(json!({ "name": step, "item": [] }));
                    level.len() - 1
                }
            };
            level = level[at]["item"]
                .as_array_mut()
                .expect("a folder found or made here holds its items");
        }
        level.push(item(request, file, notes));
    }
    let value = json!({
        "info": { "name": title, "schema": SCHEMA },
        "item": root,
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

fn kv(key: &str, value: &str) -> Value {
    json!({ "key": key, "value": value, "type": "string" })
}

fn item(request: &ApiRequest, file: &str, notes: &mut Vec<Note>) -> Value {
    let name = if request.name.trim().is_empty() {
        file.trim_end_matches(rb_api_model::paths::EXTENSION)
    } else {
        request.name.trim()
    };
    let multipart = matches!(request.body, Body::Multipart { .. });
    let header: Vec<Value> = request
        .headers
        .iter()
        .filter(|h| {
            !(multipart
                && h.name.eq_ignore_ascii_case("Content-Type")
                && h.value.to_ascii_lowercase().contains("multipart/"))
        })
        .map(|h| entry(h, false))
        .collect();
    let mut inner = Map::new();
    inner.insert("method".into(), json!(request.method));
    inner.insert("header".into(), Value::Array(header));
    inner.insert("url".into(), json!(request.url.trim()));
    if let Some(body) = body(request) {
        inner.insert("body".into(), body);
    }
    if let Some(auth) = auth(&request.auth) {
        inner.insert("auth".into(), auth);
    }
    if !request.comments.is_empty() {
        inner.insert("description".into(), json!(request.comments.join("\n")));
    }
    let mut out = Map::new();
    out.insert("name".into(), json!(name));
    out.insert("request".into(), Value::Object(inner));
    let mut behavior = Map::new();
    if !request.options.follow_redirects {
        behavior.insert("followRedirects".into(), json!(false));
    }
    if !request.options.verify_tls {
        behavior.insert("strictSSL".into(), json!(false));
    }
    if !behavior.is_empty() {
        out.insert("protocolProfileBehavior".into(), Value::Object(behavior));
    }
    if request.options.timeout_ms != DEFAULT_TIMEOUT_MS {
        notes.push(Note::Timeout);
    }
    Value::Object(out)
}

/// A header or a field; `text` adds the field's `"type": "text"`.
fn entry(entry: &Entry, text: bool) -> Value {
    let mut out = json!({ "key": entry.name, "value": entry.value });
    if text {
        out["type"] = json!("text");
    }
    if !entry.enabled {
        out["disabled"] = json!(true);
    }
    out
}

/// The language Postman shows a raw body in, by its `Content-Type`.
fn language(request: &ApiRequest) -> &'static str {
    let ty = request
        .header("Content-Type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ty.contains("json") {
        "json"
    } else if ty.contains("xml") {
        "xml"
    } else if ty.contains("html") {
        "html"
    } else if ty.contains("javascript") {
        "javascript"
    } else {
        "text"
    }
}

fn body(request: &ApiRequest) -> Option<Value> {
    Some(match &request.body {
        Body::None => return None,
        Body::Json { text } => json!({
            "mode": "raw", "raw": text, "options": { "raw": { "language": "json" } }
        }),
        Body::Raw { text } => json!({
            "mode": "raw", "raw": text, "options": { "raw": { "language": language(request) } }
        }),
        Body::Form { fields } => json!({
            "mode": "urlencoded",
            "urlencoded": fields.iter().map(|f| entry(f, true)).collect::<Vec<_>>(),
        }),
        Body::Multipart { parts } => {
            let parts: Vec<Value> = parts
                .iter()
                .map(|part| {
                    let mut out = match &part.file {
                        Some(file) => json!({ "key": part.name, "src": file, "type": "file" }),
                        None => json!({ "key": part.name, "value": part.value, "type": "text" }),
                    };
                    if let Some(ty) = &part.content_type {
                        out["contentType"] = json!(ty);
                    }
                    out
                })
                .collect();
            json!({ "mode": "formdata", "formdata": parts })
        }
        Body::Binary { path } => json!({ "mode": "file", "file": { "src": path } }),
        Body::GraphQl { query, variables } => json!({
            "mode": "graphql", "graphql": { "query": query, "variables": variables }
        }),
    })
}

fn auth(auth: &Auth) -> Option<Value> {
    let (kind, entries) = match auth {
        Auth::None => return None,
        Auth::Basic { username, password } => (
            "basic",
            vec![kv("username", username), kv("password", password)],
        ),
        Auth::Digest { username, password } => (
            "digest",
            vec![kv("username", username), kv("password", password)],
        ),
        Auth::Bearer { token } => ("bearer", vec![kv("token", token)]),
        Auth::ApiKey { name, value, place } => (
            "apikey",
            vec![
                kv("key", name),
                kv("value", value),
                kv(
                    "in",
                    match place {
                        KeyPlace::Header => "header",
                        KeyPlace::Query => "query",
                    },
                ),
            ],
        ),
        Auth::OAuth2 {
            grant,
            token_url,
            auth_url,
            client_id,
            client_secret,
            scope,
            redirect_uri,
        } => {
            let mut entries = vec![
                kv(
                    "grant_type",
                    match grant {
                        OAuthGrant::ClientCredentials => "client_credentials",
                        OAuthGrant::AuthorizationCode => "authorization_code_with_pkce",
                    },
                ),
                kv("accessTokenUrl", token_url),
                kv("clientId", client_id),
                kv("addTokenTo", "header"),
            ];
            for (key, value) in [
                ("authUrl", auth_url),
                ("clientSecret", client_secret),
                ("scope", scope),
                ("redirect_uri", redirect_uri),
            ] {
                if !value.is_empty() {
                    entries.push(kv(key, value));
                }
            }
            if *grant == OAuthGrant::AuthorizationCode {
                entries.push(kv("challengeAlgorithm", "S256"));
            }
            ("oauth2", entries)
        }
    };
    let mut out = Map::new();
    out.insert("type".into(), json!(kind));
    out.insert(kind.into(), Value::Array(entries));
    Some(Value::Object(out))
}

#[cfg(test)]
#[path = "postman_tests.rs"]
mod tests;
