//! Postman: a collection v2.0 or v2.1 (folders, requests, auth inherited from folder and
//! collection, collection variables as `$shared`), an environment, the globals (`$shared`).
//!
//! Mapped: method, URL (`:name` path variables put in), headers (disabled ones kept off), bodies
//! raw (JSON or text), urlencoded, form-data (a file part as `./<file name>`), file, GraphQL;
//! auth none, basic, bearer, API key, digest, OAuth 2 (client credentials, authorization code);
//! `followRedirects` and `strictSSL`; the description as comments. Reported: pre-request and test
//! scripts, saved examples, other auth kinds, other request settings, a file a body names;
//! dropped: disabled query parameters and form-data parts, disabled variables.

use rb_api_model::{ApiRequest, Auth, Body, Entry, KeyPlace, OAuthGrant, Part};
use serde_json::Value;

use crate::collect::Collector;
use crate::json::{as_text, flag, list, text};
use crate::{Source, common};

/// The deepest folder read.
const MAX_NESTING: usize = 32;

/// Reads a Postman collection, environment or globals file.
///
/// # Errors
///
/// Not JSON, or neither a collection nor an environment.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let value: Value =
        serde_json::from_str(&source.text).map_err(|error| format!("not JSON: {error}"))?;
    if value["values"].is_array() && !value["item"].is_array() {
        environment(&value, out);
        return Ok(());
    }
    if !value["item"].is_array() {
        return Err("no Postman collection or environment".to_owned());
    }
    let name = text(&value["info"], "name").into_owned();
    out.collection(&name);
    let at = if name.trim().is_empty() {
        "collection".to_owned()
    } else {
        name
    };
    events(&value, &at, out);
    let variables = variables(list(&value, "variable"), "key", &at, out);
    if !variables.is_empty() {
        out.environment(rb_api_model::env::SHARED, &variables);
    }
    items(list(&value, "item"), &[], &value["auth"], out, 0);
    Ok(())
}

/// Postman's variables as (name, value, secret); disabled ones dropped.
fn variables(
    items: &[Value],
    key: &str,
    at: &str,
    out: &mut Collector,
) -> Vec<(String, String, bool)> {
    let mut taken = Vec::new();
    for item in items {
        let name = text(item, key).into_owned();
        if item["enabled"] == Value::Bool(false) || flag(item, "disabled") {
            out.dropped(at, "disabled", &name);
            continue;
        }
        let secret = text(item, "type") == "secret";
        taken.push((name, as_text(&item["value"]).into_owned(), secret));
    }
    taken
}

fn environment(value: &Value, out: &mut Collector) {
    let globals = text(value, "_postman_variable_scope") == "globals";
    let name = if globals {
        rb_api_model::env::SHARED.to_owned()
    } else {
        text(value, "name").into_owned()
    };
    let at = format!("environment {name}");
    let variables = variables(list(value, "values"), "key", &at, out);
    out.environment(&name, &variables);
}

fn items(
    items: &[Value],
    folders: &[String],
    inherited: &Value,
    out: &mut Collector,
    depth: usize,
) {
    for item in items {
        let name = text(item, "name").into_owned();
        if item["item"].is_array() {
            if depth >= MAX_NESTING {
                out.dropped(&name, "item", "too deep");
                continue;
            }
            events(item, &name, out);
            let auth = if item["auth"].is_object() {
                &item["auth"]
            } else {
                inherited
            };
            let mut path = folders.to_vec();
            path.push(name);
            self::items(list(item, "item"), &path, auth, out, depth + 1);
        } else if item["request"].is_object() || item["request"].is_string() {
            request(item, &name, folders, inherited, out);
        } else {
            out.dropped(&name, "item", "no request");
        }
    }
}

fn events(item: &Value, at: &str, out: &mut Collector) {
    for event in list(item, "event") {
        let exec = &event["script"]["exec"];
        let code: String = match exec {
            Value::Array(lines) => lines
                .iter()
                .map(|line| as_text(line).into_owned())
                .collect(),
            other => as_text(other).into_owned(),
        };
        if code.trim().is_empty() {
            continue;
        }
        match &*text(event, "listen") {
            "test" => out.reported(at, "tests", "post-response script"),
            "prerequest" => out.reported(at, "script", "pre-request"),
            other => out.reported(at, "script", other),
        }
    }
}

fn request(item: &Value, name: &str, folders: &[String], inherited: &Value, out: &mut Collector) {
    let raw = &item["request"];
    let mut request = ApiRequest::new(
        &text(raw, "method").to_ascii_uppercase(),
        &url_of(if raw.is_string() { raw } else { &raw["url"] }, name, out),
    );
    request.name = name.to_owned();
    request.headers = headers(&raw["header"]);
    request.body = body(&raw["body"], name, &mut request.headers, out);
    let auth = if raw["auth"].is_object() {
        &raw["auth"]
    } else {
        inherited
    };
    request.auth = auth_of(auth, name, out);
    let description = match &raw["description"] {
        Value::Object(_) => text(&raw["description"], "content").into_owned(),
        other => as_text(other).into_owned(),
    };
    if !description.trim().is_empty() {
        request.comments = description.lines().map(str::to_owned).collect();
    }
    events(item, name, out);
    let examples = list(item, "response").len();
    if examples > 0 {
        out.reported(name, "examples", &examples.to_string());
    }
    if let Value::Object(behaviour) = &item["protocolProfileBehavior"] {
        for (key, value) in behaviour {
            match (key.as_str(), value) {
                ("followRedirects", Value::Bool(follow)) => {
                    request.options.follow_redirects = *follow
                }
                ("strictSSL", Value::Bool(strict)) => request.options.verify_tls = *strict,
                _ => out.reported(name, "setting", key),
            }
        }
    }
    out.request(folders, request);
}

fn url_of(url: &Value, at: &str, out: &mut Collector) -> String {
    let Value::Object(_) = url else {
        return common::path_params(&as_text(url), &[]);
    };
    let mut raw = text(url, "raw").into_owned();
    if raw.is_empty() {
        raw = built(url);
    }
    for param in list(url, "query") {
        if flag(param, "disabled") {
            out.dropped(at, "disabled", &format!("query {}", text(param, "key")));
        }
    }
    let values: Vec<(String, String)> = list(url, "variable")
        .iter()
        .map(|v| {
            (
                text(v, "key").into_owned(),
                as_text(&v["value"]).into_owned(),
            )
        })
        .collect();
    common::path_params(&raw, &values)
}

/// A URL from its parts, when `raw` is missing.
fn built(url: &Value) -> String {
    let joined = |key: &str, by: &str| -> String {
        match &url[key] {
            Value::Array(parts) => parts
                .iter()
                .map(|part| as_text(part).into_owned())
                .collect::<Vec<_>>()
                .join(by),
            other => as_text(other).into_owned(),
        }
    };
    let protocol = text(url, "protocol");
    let mut out = String::new();
    if !protocol.is_empty() {
        out.push_str(&protocol);
        out.push_str("://");
    }
    out.push_str(&joined("host", "."));
    let path = joined("path", "/");
    if !path.is_empty() {
        out.push('/');
        out.push_str(path.trim_start_matches('/'));
    }
    let query: Vec<String> = list(url, "query")
        .iter()
        .filter(|param| !flag(param, "disabled"))
        .map(|param| format!("{}={}", text(param, "key"), as_text(&param["value"])))
        .collect();
    if !query.is_empty() {
        out.push('?');
        out.push_str(&query.join("&"));
    }
    out
}

fn headers(value: &Value) -> Vec<Entry> {
    match value {
        Value::String(lines) => lines
            .lines()
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| Entry::new(name.trim(), value.trim()))
            .collect(),
        Value::Array(items) => items
            .iter()
            .map(|item| Entry {
                name: text(item, "key").into_owned(),
                value: as_text(&item["value"]).into_owned(),
                enabled: !flag(item, "disabled"),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn body(value: &Value, at: &str, headers: &mut Vec<Entry>, out: &mut Collector) -> Body {
    if !value.is_object() || flag(value, "disabled") {
        return Body::None;
    }
    match &*text(value, "mode") {
        "raw" => {
            let raw = text(value, "raw").into_owned();
            let language = text(&value["options"]["raw"], "language").into_owned();
            common::text_body(raw, &language, headers)
        }
        "urlencoded" => Body::Form {
            fields: list(value, "urlencoded")
                .iter()
                .map(|field| Entry {
                    name: text(field, "key").into_owned(),
                    value: as_text(&field["value"]).into_owned(),
                    enabled: !flag(field, "disabled"),
                })
                .collect(),
        },
        "formdata" => {
            let mut parts = Vec::new();
            for field in list(value, "formdata") {
                let name = text(field, "key").into_owned();
                if flag(field, "disabled") {
                    out.dropped(at, "disabled", &format!("part {name}"));
                    continue;
                }
                let content_type =
                    Some(text(field, "contentType").into_owned()).filter(|t| !t.is_empty());
                let file = if text(field, "type") == "file" {
                    let src = match &field["src"] {
                        Value::Array(files) => files
                            .first()
                            .map(|f| as_text(f).into_owned())
                            .unwrap_or_default(),
                        other => as_text(other).into_owned(),
                    };
                    Some(common::local_file(&src, at, out))
                } else {
                    None
                };
                parts.push(Part {
                    value: if file.is_some() {
                        String::new()
                    } else {
                        as_text(&field["value"]).into_owned()
                    },
                    name,
                    file,
                    content_type,
                });
            }
            Body::Multipart { parts }
        }
        "file" => {
            let src = text(&value["file"], "src").into_owned();
            if src.trim().is_empty() {
                out.dropped(at, "body", "file without a path");
                Body::None
            } else {
                Body::Binary {
                    path: common::local_file(&src, at, out),
                }
            }
        }
        "graphql" => Body::GraphQl {
            query: text(&value["graphql"], "query").into_owned(),
            variables: as_text(&value["graphql"]["variables"]).into_owned(),
        },
        "" => Body::None,
        other => {
            out.dropped(at, "body", other);
            Body::None
        }
    }
}

/// A parameter of an auth: v2.1's `[{key, value}]` or v2.0's object.
fn param(auth: &Value, kind: &str, key: &str) -> String {
    match &auth[kind] {
        Value::Array(items) => items
            .iter()
            .find(|item| text(item, "key") == key)
            .map(|item| as_text(&item["value"]).into_owned())
            .unwrap_or_default(),
        other => as_text(&other[key]).into_owned(),
    }
}

fn auth_of(auth: &Value, at: &str, out: &mut Collector) -> Auth {
    let kind = text(auth, "type").into_owned();
    let get = |key: &str| param(auth, &kind, key);
    match kind.as_str() {
        "" | "noauth" => Auth::None,
        "basic" => Auth::Basic {
            username: get("username"),
            password: get("password"),
        },
        "digest" => Auth::Digest {
            username: get("username"),
            password: get("password"),
        },
        "bearer" => Auth::Bearer {
            token: get("token"),
        },
        "apikey" => Auth::ApiKey {
            name: Some(get("key"))
                .filter(|k| !k.is_empty())
                .unwrap_or_else(|| "X-API-Key".to_owned()),
            value: get("value"),
            place: if get("in") == "query" {
                KeyPlace::Query
            } else {
                KeyPlace::Header
            },
        },
        "oauth2" => {
            let grant = match get("grant_type").as_str() {
                "" | "client_credentials" => OAuthGrant::ClientCredentials,
                "authorization_code" | "authorization_code_with_pkce" => {
                    OAuthGrant::AuthorizationCode
                }
                other => {
                    out.reported(at, "auth", &format!("oauth2 {other}"));
                    return Auth::None;
                }
            };
            Auth::OAuth2 {
                grant,
                token_url: get("accessTokenUrl"),
                auth_url: get("authUrl"),
                client_id: get("clientId"),
                client_secret: get("clientSecret"),
                scope: get("scope"),
                redirect_uri: get("redirect_uri"),
            }
        }
        other => {
            out.reported(at, "auth", other);
            Auth::None
        }
    }
}

#[cfg(test)]
#[path = "postman_tests.rs"]
mod tests;
