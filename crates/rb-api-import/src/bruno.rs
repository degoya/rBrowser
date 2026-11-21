//! Bruno: a collection's folder - `bruno.json` (the name), `collection.bru` and each
//! `folder.bru` (headers, auth, scripts), one request per `.bru` file, `environments/*.bru`.
//! A `.bru` file is blocks: `name {` ... `}` of `key: value` lines (`~` for a disabled one) or of
//! text indented by two spaces, and `name [` ... `]` lists.
//!
//! Mapped: method, URL, query and path parameters, headers (the collection's and folders' before
//! the request's; disabled ones kept off), bodies JSON, text, XML, SPARQL, form, multipart
//! (`@file(...)` as `./<file name>`), GraphQL with its variables; auth basic, bearer, digest, API
//! key, OAuth 2 and `inherit` (the folder's, else the collection's); `docs` as comments; an
//! environment's `vars`, its `vars:secret` names as placeholders. Reported: scripts, tests and
//! assertions, request variables, other auth kinds; dropped: disabled parameters, a body file,
//! files that are no part of a collection (`.env` included).

use std::collections::HashMap;

use rb_api_model::{ApiRequest, Auth, Body, Entry, KeyPlace, OAuthGrant, Part};

use crate::Source;
pub(crate) use crate::bruno_file::{Block, blocks};
use crate::bruno_file::{METHODS, block, entries, value};
use crate::collect::Collector;
use crate::common;

/// The headers and the auth a folder or the collection hands down.
#[derive(Clone, Debug, Default)]
struct Level {
    headers: Vec<Entry>,
    /// The auth's mode and the blocks it reads from; `None` hands the outer level's down.
    auth: Option<(String, Vec<Block>)>,
}

fn level(blocks: Vec<Block>, at: &str, out: &mut Collector) -> Level {
    notes(&blocks, at, out);
    let mode = value(&blocks, "auth", "mode");
    Level {
        headers: entries(&blocks, "headers"),
        auth: if mode.is_empty() || mode == "inherit" {
            None
        } else {
            Some((mode, blocks))
        },
    }
}

/// Scripts, tests, assertions and request variables, reported.
fn notes(blocks: &[Block], at: &str, out: &mut Collector) {
    for block in blocks {
        let empty = block.text.trim().is_empty() && block.dict.is_empty();
        if empty {
            continue;
        }
        match block.name.as_str() {
            "tests" | "assert" => out.reported(at, "tests", &block.name),
            name if name.starts_with("script:") => {
                out.reported(at, "script", name.trim_start_matches("script:"));
            }
            name if name.starts_with("vars:") => out.reported(at, "variable", name),
            "settings" => out.reported(at, "setting", "settings"),
            _ => {}
        }
    }
}

fn normalised(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_owned()
}

/// The collection's folder: where `bruno.json` or `collection.bru` lies, else the folder every
/// file shares.
fn root_of(paths: &[String]) -> String {
    let marked = paths
        .iter()
        .filter_map(|path| {
            path.strip_suffix("bruno.json")
                .or_else(|| path.strip_suffix("collection.bru"))
                .filter(|dir| dir.is_empty() || dir.ends_with('/'))
        })
        .min_by_key(|dir| dir.len());
    if let Some(dir) = marked {
        return dir.to_owned();
    }
    let first: Vec<&str> = paths
        .first()
        .map_or_else(Vec::new, |p| p.split('/').collect());
    let mut shared = first.len().saturating_sub(1);
    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        let same = first
            .iter()
            .zip(&parts[..parts.len().saturating_sub(1)])
            .take_while(|(a, b)| a == b)
            .count();
        shared = shared.min(same);
    }
    first[..shared]
        .iter()
        .map(|part| format!("{part}/"))
        .collect()
}

/// Reads the files of one Bruno collection.
///
/// # Errors
///
/// None so far: a file that does not read is reported.
pub fn read(sources: &[Source], out: &mut Collector) -> Result<(), String> {
    let mut files: Vec<(String, &Source)> = sources
        .iter()
        .map(|source| (normalised(&source.name), source))
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let names: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
    let root = root_of(&names);
    let mut name = files
        .iter()
        .find(|(path, _)| path.strip_prefix(&root) == Some("bruno.json"))
        .and_then(|(_, source)| serde_json::from_str::<serde_json::Value>(&source.text).ok())
        .map(|value| crate::json::text(&value, "name").into_owned())
        .unwrap_or_default();
    if name.trim().is_empty() {
        name = root
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_owned();
    }
    out.collection(&name);
    let at = if name.trim().is_empty() {
        "collection"
    } else {
        name.as_str()
    };
    let mut collection = Level::default();
    let mut folders: HashMap<String, (String, Level)> = HashMap::new();
    let mut requests = Vec::new();
    let mut environments = Vec::new();
    for (path, source) in &files {
        let relative = path.strip_prefix(&root).unwrap_or(path).to_owned();
        let (dir, file) = relative.rsplit_once('/').unwrap_or(("", relative.as_str()));
        if relative == "bruno.json" {
            continue;
        } else if relative == "collection.bru" {
            collection = level(blocks(&source.text), at, out);
        } else if file == "folder.bru" {
            let blocks = blocks(&source.text);
            let shown = Some(value(&blocks, "meta", "name"))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| dir.rsplit('/').next().unwrap_or(dir).to_owned());
            let held = level(blocks, &shown, out);
            folders.insert(dir.to_owned(), (shown, held));
        } else if dir == "environments" && file.ends_with(".bru") {
            environments.push((file.trim_end_matches(".bru").to_owned(), *source));
        } else if file.ends_with(".bru") {
            requests.push((relative.clone(), *source));
        } else {
            out.dropped(&relative, "item", "not read");
        }
    }
    for (relative, source) in requests {
        request(&relative, &source.text, &collection, &folders, out);
    }
    for (env, source) in environments {
        environment(&env, &source.text, out);
    }
    Ok(())
}

fn environment(name: &str, text: &str, out: &mut Collector) {
    let blocks = blocks(text);
    let at = format!("environment {name}");
    let mut variables = Vec::new();
    if let Some(vars) = block(&blocks, "vars") {
        for (key, value, enabled) in &vars.dict {
            if *enabled {
                variables.push((key.clone(), value.clone(), false));
            } else {
                out.dropped(&at, "disabled", key);
            }
        }
    }
    if let Some(secret) = block(&blocks, "vars:secret") {
        for key in &secret.list {
            variables.push((key.clone(), String::new(), true));
        }
    }
    out.environment(name, &variables);
}

fn request(
    relative: &str,
    text: &str,
    collection: &Level,
    folders: &HashMap<String, (String, Level)>,
    out: &mut Collector,
) {
    let blocks = blocks(text);
    let stem = relative
        .rsplit('/')
        .next()
        .unwrap_or(relative)
        .trim_end_matches(".bru");
    let name = Some(value(&blocks, "meta", "name"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| stem.to_owned());
    let Some(method) = blocks.iter().find(|b| METHODS.contains(&b.name.as_str())) else {
        out.dropped(&name, "item", "no request");
        return;
    };
    // The folders from the outermost in, each with its level.
    let dirs: Vec<&str> = relative.split('/').collect();
    let mut path = Vec::new();
    let mut levels = vec![collection];
    for depth in 1..dirs.len() {
        let dir = dirs[..depth].join("/");
        match folders.get(&dir) {
            Some((shown, held)) => {
                path.push(shown.clone());
                levels.push(held);
            }
            None => path.push(dirs[depth - 1].to_owned()),
        }
    }
    let get = |key: &str| value(std::slice::from_ref(method), &method.name, key);
    let mut url = get("url");
    let mut query = Vec::new();
    if let Some(params) = block(&blocks, "params:query") {
        for (key, value, enabled) in &params.dict {
            if *enabled {
                query.push((key.clone(), value.clone()));
            } else {
                out.dropped(&name, "disabled", &format!("query {key}"));
            }
        }
    }
    if !url.contains('?') {
        url = common::with_query(&url, &query);
    }
    let path_values: Vec<(String, String)> = block(&blocks, "params:path")
        .map(|b| {
            b.dict
                .iter()
                .map(|(k, v, _)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let url = common::path_params(&url, &path_values);
    let mut request = ApiRequest::new(&method.name.to_ascii_uppercase(), &url);
    request.name = name.clone();
    let mut headers: Vec<Entry> = levels.iter().flat_map(|l| l.headers.clone()).collect();
    for own in entries(&blocks, "headers") {
        headers.retain(|h| !h.name.eq_ignore_ascii_case(&own.name));
        headers.push(own);
    }
    request.headers = headers;
    request.body = body(&get("body"), &blocks, &name, &mut request.headers, out);
    let mode = get("auth");
    request.auth = match mode.as_str() {
        "inherit" => match levels.iter().rev().find_map(|l| l.auth.as_ref()) {
            Some((mode, held)) => auth_of(mode, held, &name, out),
            None => Auth::None,
        },
        mode => auth_of(mode, &blocks, &name, out),
    };
    if let Some(docs) = block(&blocks, "docs") {
        request.comments = docs.text.lines().map(str::to_owned).collect();
    }
    notes(&blocks, &name, out);
    out.request(&path, request);
}

fn body(
    mode: &str,
    blocks: &[Block],
    at: &str,
    headers: &mut Vec<Entry>,
    out: &mut Collector,
) -> Body {
    let text = |name: &str| {
        block(blocks, name)
            .map(|b| b.text.clone())
            .unwrap_or_default()
    };
    match mode {
        "" | "none" => Body::None,
        "json" => common::text_body(text("body:json"), "json", headers),
        "text" => common::text_body(text("body:text"), "text", headers),
        "xml" => common::text_body(text("body:xml"), "xml", headers),
        "sparql" => {
            if common::content_type(headers).is_empty() {
                headers.push(Entry::new("Content-Type", "application/sparql-query"));
            }
            common::text_body(text("body:sparql"), "", headers)
        }
        "formUrlEncoded" => Body::Form {
            fields: entries(blocks, "body:form-urlencoded"),
        },
        "multipartForm" => {
            let mut parts = Vec::new();
            for field in entries(blocks, "body:multipart-form") {
                if !field.enabled {
                    out.dropped(at, "disabled", &format!("part {}", field.name));
                    continue;
                }
                let file = field
                    .value
                    .strip_prefix("@file(")
                    .and_then(|rest| rest.strip_suffix(')'))
                    .map(|files| {
                        common::local_file(files.split('|').next().unwrap_or(files), at, out)
                    });
                parts.push(Part {
                    name: field.name,
                    value: if file.is_some() {
                        String::new()
                    } else {
                        field.value
                    },
                    file,
                    content_type: None,
                });
            }
            Body::Multipart { parts }
        }
        "graphql" => Body::GraphQl {
            query: text("body:graphql"),
            variables: text("body:graphql:vars"),
        },
        other => {
            out.dropped(at, "body", other);
            Body::None
        }
    }
}

fn auth_of(mode: &str, blocks: &[Block], at: &str, out: &mut Collector) -> Auth {
    let section = format!("auth:{mode}");
    let get = |key: &str| value(blocks, &section, key);
    match mode {
        "" | "none" | "inherit" => Auth::None,
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
            name: get("key"),
            value: get("value"),
            place: if get("placement").eq_ignore_ascii_case("queryparams") {
                KeyPlace::Query
            } else {
                KeyPlace::Header
            },
        },
        "oauth2" => {
            let grant = match get("grant_type").as_str() {
                "" | "client_credentials" => OAuthGrant::ClientCredentials,
                "authorization_code" => OAuthGrant::AuthorizationCode,
                other => {
                    out.reported(at, "auth", &format!("oauth2 {other}"));
                    return Auth::None;
                }
            };
            Auth::OAuth2 {
                grant,
                token_url: get("access_token_url"),
                auth_url: get("authorization_url"),
                client_id: get("client_id"),
                client_secret: get("client_secret"),
                scope: get("scope"),
                redirect_uri: get("callback_url"),
            }
        }
        other => {
            out.reported(at, "auth", other);
            Auth::None
        }
    }
}

#[cfg(test)]
#[path = "bruno_tests.rs"]
mod tests;
