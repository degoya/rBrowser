//! The `{{...}}` of a template, read as `rb_api_model::vars` reads them, for what an export does
//! with them: [`fill`] puts in the plain values of an environment and keeps every secret, every
//! process variable and every unknown name as the variable it is; [`env_name`] names the
//! environment variable a snippet reads in its place; [`encode`] form-encodes the text around
//! the variables.

use rb_api_model::vars::{Lookup, MAX_DEPTH, is_name};
use rb_api_model::{ApiRequest, Auth, Body, Entry, Part};

/// The prefix of the process variables `{{$secret name}}` reads, as the core's
/// `api_client::resolve::secret_variable` builds them.
const SECRET_PREFIX: &str = "RB_SECRET_";

/// One piece of a template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Piece<'a> {
    Text(&'a str),
    /// `{{name}}`: the whole token and the name.
    Var {
        raw: &'a str,
        name: &'a str,
    },
    /// `{{$name args}}`: the whole token, the name with its `$`, the arguments.
    Dynamic {
        raw: &'a str,
        name: &'a str,
        args: &'a str,
    },
}

/// The pieces of `template`; what is no variable's form (`{{ 1 + 1 }}`, a lone `{{`) is text.
pub(crate) fn pieces(template: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open + 2..].find("}}") else {
            break;
        };
        let end = open + 2 + close + 2;
        let raw = &rest[open..end];
        let inner = rest[open + 2..open + 2 + close].trim();
        let piece = if let Some(dynamic) = inner.strip_prefix('$') {
            let (name, args) = dynamic
                .split_once(char::is_whitespace)
                .unwrap_or((dynamic, ""));
            is_name(name).then(|| Piece::Dynamic {
                raw,
                name: &inner[..name.len() + 1],
                args: args.trim(),
            })
        } else {
            is_name(inner).then_some(Piece::Var { raw, name: inner })
        };
        match piece {
            Some(piece) => {
                if open > 0 {
                    out.push(Piece::Text(&rest[..open]));
                }
                out.push(piece);
                rest = &rest[end..];
            }
            None => {
                out.push(Piece::Text(&rest[..open + 2]));
                rest = &rest[open + 2..];
            }
        }
    }
    if !rest.is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

/// `template` with the plain values of `lookup` put in, up to [`MAX_DEPTH`] deep; a secret, a
/// process variable (`$processEnv`, `$secret` - never read here) and an unknown name stay as
/// they are written.
#[must_use]
pub fn fill_text(template: &str, lookup: &dyn Lookup) -> String {
    let mut out = String::with_capacity(template.len());
    put(template, lookup, 0, &mut out);
    out
}

fn put(template: &str, lookup: &dyn Lookup, depth: usize, out: &mut String) {
    for piece in pieces(template) {
        let (raw, value) = match piece {
            Piece::Text(text) => {
                out.push_str(text);
                continue;
            }
            Piece::Var { raw, name } => (raw, lookup.variable(name)),
            Piece::Dynamic { raw, name, .. } if name == "$processEnv" || name == "$secret" => {
                (raw, None)
            }
            Piece::Dynamic { raw, name, args } => (raw, lookup.dynamic(name, args)),
        };
        match value {
            Some(value) if !value.secret => {
                if value.text.contains("{{") {
                    if depth + 1 < MAX_DEPTH {
                        put(&value.text, lookup, depth + 1, out);
                    } else {
                        out.push_str(raw);
                    }
                } else {
                    out.push_str(&value.text);
                }
            }
            _ => out.push_str(raw),
        }
    }
}

/// `request` with every template filled ([`fill_text`]).
#[must_use]
pub fn fill(request: &ApiRequest, lookup: &dyn Lookup) -> ApiRequest {
    map_templates(request, &|text| fill_text(text, lookup))
}

/// `request` with `f` applied to every template it holds: the URL, the headers, the body's texts,
/// fields, parts and paths, the auth's fields. The title, the method and the comments are no
/// templates.
#[must_use]
pub(crate) fn map_templates(request: &ApiRequest, f: &dyn Fn(&str) -> String) -> ApiRequest {
    let entry = |entry: &Entry| Entry {
        name: f(&entry.name),
        value: f(&entry.value),
        enabled: entry.enabled,
    };
    let body = match &request.body {
        Body::None => Body::None,
        Body::Json { text } => Body::Json { text: f(text) },
        Body::Raw { text } => Body::Raw { text: f(text) },
        Body::Form { fields } => Body::Form {
            fields: fields.iter().map(entry).collect(),
        },
        Body::Multipart { parts } => Body::Multipart {
            parts: parts
                .iter()
                .map(|part| Part {
                    name: f(&part.name),
                    value: f(&part.value),
                    file: part.file.as_deref().map(f),
                    content_type: part.content_type.as_deref().map(f),
                })
                .collect(),
        },
        Body::Binary { path } => Body::Binary { path: f(path) },
        Body::GraphQl { query, variables } => Body::GraphQl {
            query: f(query),
            variables: f(variables),
        },
    };
    let auth = match &request.auth {
        Auth::None => Auth::None,
        Auth::Basic { username, password } => Auth::Basic {
            username: f(username),
            password: f(password),
        },
        Auth::Digest { username, password } => Auth::Digest {
            username: f(username),
            password: f(password),
        },
        Auth::Bearer { token } => Auth::Bearer { token: f(token) },
        Auth::ApiKey { name, value, place } => Auth::ApiKey {
            name: f(name),
            value: f(value),
            place: *place,
        },
        Auth::OAuth2 {
            grant,
            token_url,
            auth_url,
            client_id,
            client_secret,
            scope,
            redirect_uri,
        } => Auth::OAuth2 {
            grant: *grant,
            token_url: f(token_url),
            auth_url: f(auth_url),
            client_id: f(client_id),
            client_secret: f(client_secret),
            scope: f(scope),
            redirect_uri: f(redirect_uri),
        },
    };
    ApiRequest {
        name: request.name.clone(),
        method: request.method.clone(),
        url: f(&request.url),
        headers: request.headers.iter().map(entry).collect(),
        body,
        auth,
        options: request.options,
        comments: request.comments.clone(),
        // A check's values are written as they are, never filled (RB-096-04).
        checks: request.checks.clone(),
    }
}

fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The environment variable a snippet reads for `piece`: a variable's name with `-` and `.` as
/// `_`, the variable `{{$processEnv NAME}}` names, `RB_SECRET_<NAME>` for `{{$secret name}}`;
/// `None` for text and for the other dynamic variables, which stay as they are written.
pub(crate) fn env_name(piece: &Piece<'_>) -> Option<String> {
    match piece {
        Piece::Text(_) => None,
        Piece::Var { name, .. } => Some(name.replace(['-', '.'], "_")),
        Piece::Dynamic { name, args, .. } => match *name {
            "$processEnv" => is_env_name(args).then(|| (*args).to_owned()),
            "$secret" => {
                let upper = args.to_ascii_uppercase().replace(['-', '.'], "_");
                is_env_name(&upper).then(|| format!("{SECRET_PREFIX}{upper}"))
            }
            _ => None,
        },
    }
}

/// `template` form-encoded (`application/x-www-form-urlencoded`, a space as `%20`), its
/// variables kept as they are.
pub(crate) fn encode(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    for piece in pieces(template) {
        match piece {
            Piece::Text(text) => {
                for byte in text.bytes() {
                    if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                        out.push(char::from(byte));
                    } else {
                        out.push_str(&format!("%{byte:02X}"));
                    }
                }
            }
            Piece::Var { raw, .. } | Piece::Dynamic { raw, .. } => out.push_str(raw),
        }
    }
    out
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
