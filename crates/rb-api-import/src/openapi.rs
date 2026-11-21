//! OpenAPI 3.x and Swagger 2.0, JSON or YAML: one request per operation, in a folder per first
//! tag, its title the summary (else the operation id), with an example body.
//!
//! Mapped: the first server (its variables as `$shared` variables; a relative one after
//! `{{baseUrl}}`) or Swagger's scheme, host and base path; path parameters as `{{name}}`; query
//! and header parameters that are required or have an example; the request body of the first
//! media type (JSON, form, multipart with a file part as `./<name>`, text, binary) or Swagger's
//! `body` and `formData` parameters; security as auth with variables (`{{token}}`,
//! `{{username}}`/`{{password}}`, `{{<scheme>}}` for an API key, OAuth 2 client credentials or
//! authorization code with `{{clientId}}`/`{{clientSecret}}`); deprecated and the description as
//! comments. Reported: the other servers, cookie parameters, callbacks, other security kinds,
//! references to other files; webhooks are dropped.

use rb_api_model::vars::is_name;
use rb_api_model::{ApiRequest, Body, Entry};
use serde_json::Value;

use crate::Source;
use crate::collect::Collector;
use crate::common;
use crate::json::{as_text, json_or_yaml, list, text};
use crate::openapi_example::{Examples, as_field, body};

const METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Reads an OpenAPI 3.x or Swagger 2.0 document.
///
/// # Errors
///
/// Neither JSON nor YAML, or another version.
pub fn read(source: &Source, out: &mut Collector) -> Result<(), String> {
    let root = json_or_yaml(&source.text)?;
    let version = text(&root, "openapi").into_owned();
    let swagger = text(&root, "swagger") == "2.0";
    if !swagger && !version.starts_with("3.") {
        let named = if version.is_empty() {
            text(&root, "swagger").into_owned()
        } else {
            version
        };
        return Err(format!("API description version {named} is not read"));
    }
    let title = text(&root["info"], "title").into_owned();
    out.collection(&title);
    let at = if title.is_empty() {
        "API".to_owned()
    } else {
        title
    };
    let base = if swagger {
        swagger_base(&root)
    } else {
        server_base(&root, &at, out)
    };
    let mut examples = Examples::new(&root);
    if let Value::Object(paths) = &root["paths"] {
        for (path, item) in paths {
            let Some(item) = examples.resolve(item) else {
                continue;
            };
            for method in METHODS {
                if let Some(operation) = item.get(*method).filter(|op| op.is_object()) {
                    let context = Operation {
                        root: &root,
                        swagger,
                        base: &base,
                        path,
                        method,
                        shared: list(item, "parameters"),
                    };
                    operation_request(&context, operation, &mut examples, out);
                }
            }
        }
    }
    if let Value::Object(webhooks) = &root["webhooks"]
        && !webhooks.is_empty()
    {
        out.dropped(&at, "item", &format!("webhooks {}", webhooks.len()));
    }
    for reference in std::mem::take(&mut examples.external) {
        out.reported(&at, "body", &format!("external $ref {reference}"));
    }
    Ok(())
}

/// The URL every path follows, from the first server; its variables become `$shared` ones.
fn server_base(root: &Value, at: &str, out: &mut Collector) -> String {
    let servers = list(root, "servers");
    for other in servers.iter().skip(1) {
        out.reported(at, "server", &text(other, "url"));
    }
    let Some(first) = servers.first() else {
        return "{{baseUrl}}".to_owned();
    };
    let mut variables = Vec::new();
    if let Value::Object(map) = &first["variables"] {
        for (name, variable) in map {
            if is_name(name) {
                variables.push((
                    name.clone(),
                    as_text(&variable["default"]).into_owned(),
                    false,
                ));
            }
        }
    }
    if !variables.is_empty() {
        out.environment(rb_api_model::env::SHARED, &variables);
    }
    let url = common::brace_params(text(first, "url").trim_end_matches('/'));
    if url.contains("://") || url.starts_with("{{") {
        url
    } else {
        format!("{{{{baseUrl}}}}{url}")
    }
}

fn swagger_base(root: &Value) -> String {
    let base_path = text(root, "basePath").trim_end_matches('/').to_owned();
    let host = text(root, "host");
    if host.is_empty() {
        return format!("{{{{baseUrl}}}}{base_path}");
    }
    let schemes = list(root, "schemes");
    let scheme = if schemes.iter().any(|s| s == "https") || schemes.is_empty() {
        "https".to_owned()
    } else {
        as_text(&schemes[0]).into_owned()
    };
    format!("{scheme}://{host}{base_path}")
}

/// What an operation's request is built from beside the operation itself.
struct Operation<'a> {
    root: &'a Value,
    swagger: bool,
    base: &'a str,
    path: &'a str,
    method: &'a str,
    /// The path item's parameters.
    shared: &'a [Value],
}

fn operation_request<'a>(
    context: &Operation<'a>,
    operation: &'a Value,
    examples: &mut Examples<'a>,
    out: &mut Collector,
) {
    let method = context.method.to_ascii_uppercase();
    let name = [text(operation, "summary"), text(operation, "operationId")]
        .into_iter()
        .find(|n| !n.trim().is_empty())
        .map_or_else(|| format!("{method} {}", context.path), |n| n.into_owned());
    let folder: Vec<String> = list(operation, "tags")
        .first()
        .map(|tag| vec![as_text(tag).into_owned()])
        .unwrap_or_default();
    // The operation's parameters override the path item's of the same name and place.
    let mut parameters: Vec<&'a Value> = Vec::new();
    for raw in context.shared.iter().chain(list(operation, "parameters")) {
        let Some(parameter) = examples.resolve(raw) else {
            continue;
        };
        let key = (
            text(parameter, "name").into_owned(),
            text(parameter, "in").into_owned(),
        );
        parameters.retain(|known| {
            (
                text(known, "name").into_owned(),
                text(known, "in").into_owned(),
            ) != key
        });
        parameters.push(parameter);
    }
    let mut query = Vec::new();
    let mut headers = Vec::new();
    let mut form = Vec::new();
    let mut body_schema = None;
    for &parameter in &parameters {
        let pname = text(parameter, "name").into_owned();
        let required = parameter["required"] == Value::Bool(true);
        let given = given_example(parameter, examples);
        let value = given.clone().unwrap_or_else(|| format!("{{{{{pname}}}}}"));
        match &*text(parameter, "in") {
            "query" if required || given.is_some() => query.push((pname, value)),
            "header" if required || given.is_some() => headers.push(Entry::new(pname, value)),
            "cookie" => out.reported(&name, "setting", &format!("cookie {pname}")),
            "body" => body_schema = Some(&parameter["schema"]),
            "formData" => form.push((
                pname,
                given.unwrap_or_default(),
                text(parameter, "type") == "file",
            )),
            _ => {}
        }
    }
    let url = common::with_query(
        &format!("{}{}", context.base, common::brace_params(context.path)),
        &query,
    );
    let mut request = ApiRequest::new(&method, &url);
    request.name = name.clone();
    request.headers = headers;
    request.body = if context.swagger {
        swagger_body(
            operation,
            context.root,
            body_schema,
            form,
            examples,
            &mut request.headers,
            &name,
            out,
        )
    } else {
        openapi_body(operation, examples, &mut request.headers, &name, out)
    };
    request.auth =
        crate::openapi_auth::auth_of(context.root, operation, context.swagger, &name, out);
    if operation["deprecated"] == Value::Bool(true) {
        request.comments.push("Deprecated.".to_owned());
    }
    let description = text(operation, "description");
    request
        .comments
        .extend(description.lines().map(str::to_owned));
    if operation["callbacks"]
        .as_object()
        .is_some_and(|c| !c.is_empty())
    {
        out.reported(&name, "item", "callbacks");
    }
    out.request(&folder, request);
}

/// A parameter's `example`, first `examples` value, or `default`, as text.
fn given_example<'a>(parameter: &'a Value, examples: &mut Examples<'a>) -> Option<String> {
    let first = parameter["examples"]
        .as_object()
        .and_then(|map| map.values().next())
        .and_then(|example| examples.resolve(example))
        .map(|example| &example["value"]);
    [
        Some(&parameter["example"]),
        first,
        Some(&parameter["schema"]["example"]),
        Some(&parameter["default"]),
        Some(&parameter["schema"]["default"]),
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.is_null())
    .map(as_field)
}

fn openapi_body<'a>(
    operation: &'a Value,
    examples: &mut Examples<'a>,
    headers: &mut Vec<Entry>,
    at: &str,
    out: &mut Collector,
) -> Body {
    let Some(request_body) = examples.resolve(&operation["requestBody"]) else {
        return Body::None;
    };
    let Value::Object(content) = &request_body["content"] else {
        return Body::None;
    };
    let preferred = content
        .keys()
        .find(|media| media.contains("json"))
        .or_else(|| content.keys().next());
    let Some(media) = preferred else {
        return Body::None;
    };
    let entry = &content[media];
    let example = if entry["example"].is_null() {
        entry["examples"]
            .as_object()
            .and_then(|map| map.values().next())
            .and_then(|example| examples.resolve(example))
            .map(|example| &example["value"])
    } else {
        Some(&entry["example"])
    };
    body(examples, media, &entry["schema"], example, headers, at, out)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the parts of one Swagger body, each needed once"
)]
fn swagger_body<'a>(
    operation: &'a Value,
    root: &'a Value,
    schema: Option<&'a Value>,
    form: Vec<(String, String, bool)>,
    examples: &mut Examples<'a>,
    headers: &mut Vec<Entry>,
    at: &str,
    out: &mut Collector,
) -> Body {
    let consumes: Vec<String> = [list(operation, "consumes"), list(root, "consumes")]
        .into_iter()
        .find(|types| !types.is_empty())
        .map(|types| types.iter().map(|t| as_text(t).into_owned()).collect())
        .unwrap_or_default();
    if let Some(schema) = schema {
        let media = consumes
            .iter()
            .find(|t| t.contains("json"))
            .or_else(|| consumes.first())
            .cloned()
            .unwrap_or_else(|| "application/json".to_owned());
        return body(examples, &media, schema, None, headers, at, out);
    }
    if form.is_empty() {
        return Body::None;
    }
    let multipart = form.iter().any(|(_, _, file)| *file)
        || consumes.iter().any(|t| t == "multipart/form-data");
    if multipart {
        Body::Multipart {
            parts: form
                .into_iter()
                .map(|(name, value, file)| rb_api_model::Part {
                    file: file.then(|| common::local_file(&name, at, out)),
                    value: if file { String::new() } else { value },
                    name,
                    content_type: None,
                })
                .collect(),
        }
    } else {
        Body::Form {
            fields: form
                .into_iter()
                .map(|(name, value, _)| Entry::new(name, value))
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "openapi_tests.rs"]
mod tests;
