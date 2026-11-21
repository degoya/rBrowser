//! Insomnia's export format 4, JSON or YAML: the workspace (the collection's name), request
//! groups (folders), requests, environments (the base one as `$shared`, a private one's values
//! all secrets).
//!
//! Mapped: method, URL and its parameters, headers (disabled ones kept off), bodies JSON, form,
//! multipart (a file as `./<file name>`), GraphQL, a file, other text; auth basic, bearer, API
//! key, digest, OAuth 2; `settingFollowRedirects`; the description as comments; `{{ _.name }}`
//! as `{{name}}`, `{% uuid %}` and `{% now 'unix' %}` as `{{$guid}}` and `{{$timestamp}}`.
//! Reported: other template tags (left as written), pre-request and after-response scripts,
//! unit tests, other auth kinds; dropped: WebSocket and gRPC requests, cookie jars, disabled
//! parameters.

use std::collections::HashMap;

use rb_api_model::vars::is_name;
use rb_api_model::{ApiRequest, Auth, Body, Entry, KeyPlace, OAuthGrant, Part};
use serde_json::Value;

use crate::Source;
use crate::collect::Collector;
use crate::common;
use crate::json::{as_text, flag, json_or_yaml, list, text};

/// The deepest folder chain followed.
const MAX_NESTING: usize = 32;

/// Reads an Insomnia v4 export.
///
/// # Errors
///
/// Neither JSON nor YAML, another export format.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let value = json_or_yaml(&source.text)?;
    let format = value["__export_format"].as_i64().unwrap_or(0);
    if format != 4 {
        return Err(format!(
            "Insomnia export format {format} is not read; export as format 4"
        ));
    }
    let resources = list(&value, "resources");
    let by_id: HashMap<String, &Value> = resources
        .iter()
        .map(|resource| (text(resource, "_id").into_owned(), resource))
        .collect();
    let kind = |id: &str| by_id.get(id).map(|r| text(r, "_type").into_owned());
    for resource in resources {
        let name = text(resource, "name").into_owned();
        match &*text(resource, "_type") {
            "workspace" => out.collection(&name),
            "request" => request(resource, &folders(resource, &by_id), out),
            "request_group" => scripts(resource, &name, out),
            "environment" => {
                let parent = text(resource, "parentId").into_owned();
                let base = kind(&parent).as_deref() == Some("workspace");
                let name = if base {
                    rb_api_model::env::SHARED.to_owned()
                } else {
                    name
                };
                let private = flag(resource, "isPrivate");
                let mut variables = Vec::new();
                flatten("", &resource["data"], private, &mut variables, 0);
                out.environment(&name, &variables);
            }
            "unit_test_suite" | "unit_test" => out.reported(&name, "tests", "unit test"),
            "cookie_jar" => {
                if !list(resource, "cookies").is_empty() {
                    out.dropped(&name, "item", "cookie jar");
                }
            }
            "api_spec" | "proto_file" | "proto_directory" => {}
            other => out.dropped(&name, "item", other),
        }
    }
    Ok(())
}

/// The names of the request groups above `resource`, outermost first.
fn folders(resource: &Value, by_id: &HashMap<String, &Value>) -> Vec<String> {
    let mut names = Vec::new();
    let mut parent = text(resource, "parentId").into_owned();
    while let Some(group) = by_id.get(&parent) {
        if text(group, "_type") != "request_group" || names.len() >= MAX_NESTING {
            break;
        }
        names.push(text(group, "name").into_owned());
        parent = text(group, "parentId").into_owned();
    }
    names.reverse();
    names
}

/// An environment's data as variables, nested objects as dotted names.
fn flatten(
    prefix: &str,
    data: &Value,
    secret: bool,
    out: &mut Vec<(String, String, bool)>,
    depth: usize,
) {
    let Value::Object(map) = data else {
        return;
    };
    for (key, value) in map {
        let name = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            Value::Object(_) if depth < 8 => flatten(&name, value, secret, out, depth + 1),
            Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                out.push((name, as_text(value).into_owned(), secret));
            }
            Value::Null => {}
            other => out.push((name, other.to_string(), secret)),
        }
    }
}

fn scripts(resource: &Value, at: &str, out: &mut Collector) {
    if !text(resource, "preRequestScript").trim().is_empty() {
        out.reported(at, "script", "pre-request");
    }
    if !text(resource, "afterResponseScript").trim().is_empty() {
        out.reported(at, "script", "after-response");
    }
}

/// `text` with Insomnia's templates in rBrowser's form where it has them.
pub(crate) fn nunjucks(text: &str, at: &str, out: &mut Collector) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let variable = rest.find("{{");
        let tag = rest.find("{%");
        let (open, is_tag) = match (variable, tag) {
            (Some(v), Some(t)) if t < v => (t, true),
            (Some(v), _) => (v, false),
            (None, Some(t)) => (t, true),
            (None, None) => break,
        };
        let close_mark = if is_tag { "%}" } else { "}}" };
        let Some(close) = rest[open + 2..].find(close_mark) else {
            break;
        };
        result.push_str(&rest[..open]);
        let whole = &rest[open..open + 2 + close + 2];
        let inner = rest[open + 2..open + 2 + close].trim();
        if is_tag {
            let word = inner.split_whitespace().next().unwrap_or("");
            match word {
                "uuid" => result.push_str("{{$guid}}"),
                "now" if inner.contains("unix") => result.push_str("{{$timestamp}}"),
                _ => {
                    out.reported(at, "template", word);
                    result.push_str(whole);
                }
            }
        } else {
            let name = inner.strip_prefix("_.").unwrap_or(inner);
            if is_name(name) {
                result.push_str("{{");
                result.push_str(name);
                result.push_str("}}");
            } else {
                result.push_str(whole);
            }
        }
        rest = &rest[open + 2 + close + 2..];
    }
    result.push_str(rest);
    result
}

fn request(resource: &Value, folders: &[String], out: &mut Collector) {
    let name = text(resource, "name").into_owned();
    let mut t = |value: &str| nunjucks(value, &name, out);
    let url = t(&text(resource, "url"));
    let mut query = Vec::new();
    let mut disabled = Vec::new();
    for param in list(resource, "parameters") {
        let key = t(&text(param, "name"));
        if flag(param, "disabled") {
            disabled.push(key);
        } else {
            query.push((key, t(&as_text(&param["value"]))));
        }
    }
    let headers: Vec<Entry> = list(resource, "headers")
        .iter()
        .map(|header| Entry {
            name: t(&text(header, "name")),
            value: t(&as_text(&header["value"])),
            enabled: !flag(header, "disabled"),
        })
        .collect();
    let mut request = ApiRequest::new(&text(resource, "method"), &common::with_query(&url, &query));
    request.name = name.clone();
    request.headers = headers;
    let body = &resource["body"];
    let raw = t(&text(body, "text"));
    let params: Vec<(String, String, bool, String)> = list(body, "params")
        .iter()
        .map(|param| {
            (
                t(&text(param, "name")),
                t(&as_text(&param["value"])),
                !flag(param, "disabled"),
                if text(param, "type") == "file" {
                    text(param, "fileName").into_owned()
                } else {
                    String::new()
                },
            )
        })
        .collect();
    let auth = auth_of(&resource["authentication"], &mut t);
    for key in disabled {
        out.dropped(&name, "disabled", &format!("query {key}"));
    }
    request.body = body_of(
        &text(body, "mimeType"),
        raw,
        &params,
        &text(body, "fileName"),
        &name,
        out,
    );
    request.auth = match auth {
        Ok(auth) => auth,
        Err(kind) => {
            out.reported(&name, "auth", &kind);
            Auth::None
        }
    };
    match &*text(resource, "settingFollowRedirects") {
        "off" => request.options.follow_redirects = false,
        "on" => request.options.follow_redirects = true,
        _ => {}
    }
    let description = text(resource, "description");
    if !description.trim().is_empty() {
        request.comments = description.lines().map(str::to_owned).collect();
    }
    scripts(resource, &name, out);
    out.request(folders, request);
}

fn body_of(
    mime: &str,
    raw: String,
    params: &[(String, String, bool, String)],
    file: &str,
    at: &str,
    out: &mut Collector,
) -> Body {
    let mime = mime.to_ascii_lowercase();
    if mime == "application/x-www-form-urlencoded" {
        return Body::Form {
            fields: params
                .iter()
                .map(|(name, value, enabled, _)| Entry {
                    name: name.clone(),
                    value: value.clone(),
                    enabled: *enabled,
                })
                .collect(),
        };
    }
    if mime == "multipart/form-data" {
        let mut parts = Vec::new();
        for (name, value, enabled, file) in params {
            if !enabled {
                out.dropped(at, "disabled", &format!("part {name}"));
                continue;
            }
            let file = (!file.is_empty()).then(|| common::local_file(file, at, out));
            parts.push(Part {
                name: name.clone(),
                value: if file.is_some() {
                    String::new()
                } else {
                    value.clone()
                },
                file,
                content_type: None,
            });
        }
        return Body::Multipart { parts };
    }
    if mime == "application/graphql" {
        let parsed: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        let variables = match &parsed["variables"] {
            Value::Null => String::new(),
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        return Body::GraphQl {
            query: text(&parsed, "query").into_owned(),
            variables,
        };
    }
    if !file.trim().is_empty() {
        return Body::Binary {
            path: common::local_file(file, at, out),
        };
    }
    if raw.trim().is_empty() {
        Body::None
    } else if mime.contains("json") {
        Body::Json { text: raw }
    } else {
        Body::Raw { text: raw }
    }
}

/// The auth; `Err` with the kind for one rBrowser has no counterpart for.
fn auth_of(auth: &Value, t: &mut dyn FnMut(&str) -> String) -> Result<Auth, String> {
    if flag(auth, "disabled") {
        return Ok(Auth::None);
    }
    let mut get = |key: &str| t(&as_text(&auth[key]));
    Ok(match &*text(auth, "type") {
        "" | "none" => Auth::None,
        "basic" => Auth::Basic {
            username: get("username"),
            password: get("password"),
        },
        "digest" => Auth::Digest {
            username: get("username"),
            password: get("password"),
        },
        "bearer" => {
            let prefix = get("prefix");
            if !prefix.is_empty() && !prefix.eq_ignore_ascii_case("Bearer") {
                return Err(format!("bearer prefix {prefix}"));
            }
            Auth::Bearer {
                token: get("token"),
            }
        }
        "apikey" => {
            let place = match &*get("addTo") {
                "queryParams" => KeyPlace::Query,
                "" | "header" => KeyPlace::Header,
                other => return Err(format!("apikey {other}")),
            };
            Auth::ApiKey {
                name: get("key"),
                value: get("value"),
                place,
            }
        }
        "oauth2" => {
            let grant = match &*get("grantType") {
                "" | "client_credentials" => OAuthGrant::ClientCredentials,
                "authorization_code" => OAuthGrant::AuthorizationCode,
                other => return Err(format!("oauth2 {other}")),
            };
            Auth::OAuth2 {
                grant,
                token_url: get("accessTokenUrl"),
                auth_url: get("authorizationUrl"),
                client_id: get("clientId"),
                client_secret: get("clientSecret"),
                scope: get("scope"),
                redirect_uri: get("redirectUrl"),
            }
        }
        other => return Err(other.to_owned()),
    })
}

#[cfg(test)]
#[path = "insomnia_tests.rs"]
mod tests;
